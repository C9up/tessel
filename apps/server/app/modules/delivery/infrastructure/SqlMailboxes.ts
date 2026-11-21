import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { EnvelopeHeader, EnvelopeKind, Placed, StoredEnvelope } from '../domain/envelope.js'
import type { Mailboxes } from '../domain/ports.js'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')
const bytes = (text: string) => new Uint8Array(Buffer.from(text, 'hex'))

/** How each kind is kept: a small number, the MLS content type's order. */
const KINDS: readonly EnvelopeKind[] = ['application', 'proposal', 'commit', 'welcome']
const kindOf = (code: number): EnvelopeKind => KINDS[code - 1] ?? 'application'

interface EnvelopeRow {
  id: string
  seq: string | null
  kind: number
  group_id: string | null
  epoch: string | null
  body: string
}

const placedOf = (row: { id: string; seq: string | null }): Placed => ({ id: row.id, seq: row.seq === null ? null : Number(row.seq) })

/**
 * The mailboxes, in `envelopes` (each envelope once), `deliveries` (who it is for) and
 * `delivery_groups` (each group's epoch and next sequence number). What each column
 * says is in the metadata register (story 15-02).
 */
export class SqlMailboxes implements Mailboxes {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  store(sender: Uint8Array, sendId: Uint8Array, header: EnvelopeHeader, body: Uint8Array, recipients: readonly Uint8Array[]): Promise<Placed | null> {
    return header.group === undefined
      ? this.#storeWelcome(sender, sendId, header, body, recipients)
      : this.#storeInGroup(sender, sendId, header, header.group, body, recipients)
  }

  /** A *Welcome* belongs to no group yet: kept as it comes. */
  async #storeWelcome(sender: Uint8Array, sendId: Uint8Array, header: EnvelopeHeader, body: Uint8Array, recipients: readonly Uint8Array[]): Promise<Placed> {
    // One statement, so one transaction: an envelope never without its recipients. The
    // same (device, send identifier) inserts nothing, and the first envelope comes back.
    const [row] = await this.#db.query<{ id: string; seq: string | null }>(
      `WITH stored AS (
         INSERT INTO envelopes (sender_device, send_id, kind, body)
         VALUES (decode($1, 'hex'), decode($2, 'hex'), $3::smallint, decode($4, 'hex'))
         ON CONFLICT (sender_device, send_id) DO NOTHING
         RETURNING id, seq
       ), addressed AS (
         INSERT INTO deliveries (envelope_id, device_id)
         SELECT s.id, decode(r, 'hex') FROM stored s, jsonb_array_elements_text($5::jsonb) r
       )
       SELECT id::text AS id, seq::text AS seq FROM stored
       UNION ALL
       SELECT id::text, seq::text FROM envelopes
        WHERE sender_device = decode($1, 'hex') AND send_id = decode($2, 'hex') AND NOT EXISTS (SELECT 1 FROM stored)`,
      [hex(sender), hex(sendId), KINDS.indexOf(header.kind) + 1, hex(body), JSON.stringify(recipients.map(hex))],
    )
    if (row === undefined) throw new Error('an envelope stored or found has an identifier')
    return placedOf(row)
  }

  /**
   * A group's envelope, in one statement: the group's row is locked and checked first
   * (its epoch must be the envelope's), a commit moves it to the next epoch, the
   * envelope takes the next sequence number, then is kept with its recipients. The row
   * lock serializes a group's envelopes across every instance of the server, and the
   * envelope's identifier is drawn after it: identifiers follow sequence numbers.
   */
  async #storeInGroup(
    sender: Uint8Array,
    sendId: Uint8Array,
    header: EnvelopeHeader,
    group: Uint8Array,
    body: Uint8Array,
    recipients: readonly Uint8Array[],
  ): Promise<Placed | null> {
    const [row] = await this.#db.query<{ id: string; seq: string | null }>(
      `WITH existing AS (
         SELECT id, seq FROM envelopes WHERE sender_device = decode($1, 'hex') AND send_id = decode($2, 'hex')
       ), placed AS (
         INSERT INTO delivery_groups AS g (group_id, epoch, next_seq)
         SELECT decode($4, 'hex'), $5::bigint + $8::int, 2 WHERE NOT EXISTS (SELECT 1 FROM existing)
         ON CONFLICT (group_id) DO UPDATE
            SET epoch = g.epoch + $8::int, next_seq = g.next_seq + 1
          WHERE g.epoch = $5::bigint
         RETURNING g.next_seq - 1 AS seq
       ), stored AS (
         INSERT INTO envelopes (sender_device, send_id, kind, group_id, epoch, body, seq)
         SELECT decode($1, 'hex'), decode($2, 'hex'), $3::smallint, decode($4, 'hex'), $5::bigint, decode($6, 'hex'), p.seq FROM placed p
         ON CONFLICT (sender_device, send_id) DO NOTHING
         RETURNING id, seq
       ), addressed AS (
         INSERT INTO deliveries (envelope_id, device_id)
         SELECT s.id, decode(r, 'hex') FROM stored s, jsonb_array_elements_text($7::jsonb) r
       )
       SELECT id::text AS id, seq::text AS seq FROM stored
       UNION ALL
       SELECT id::text, seq::text FROM existing`,
      [
        hex(sender),
        hex(sendId),
        KINDS.indexOf(header.kind) + 1,
        hex(group),
        header.epoch ?? 0,
        hex(body),
        JSON.stringify(recipients.map(hex)),
        header.kind === 'commit' ? 1 : 0,
      ],
    )
    return row === undefined ? null : placedOf(row)
  }

  async ack(device: Uint8Array, through: string): Promise<number> {
    // One statement: the device's deliveries up to `through` go; the envelopes among
    // them that nobody else waits for keep only their deduplication key and place.
    const [row] = await this.#db.query<{ acknowledged: string }>(
      `WITH acked AS (
         DELETE FROM deliveries WHERE device_id = decode($1, 'hex') AND envelope_id <= $2::bigint
         RETURNING envelope_id
       ), emptied AS (
         UPDATE envelopes e SET body = NULL, kind = NULL, group_id = NULL, epoch = NULL
          WHERE e.id IN (SELECT envelope_id FROM acked)
            AND NOT EXISTS (
              SELECT 1 FROM deliveries d
               WHERE d.envelope_id = e.id AND d.device_id <> decode($1, 'hex'))
       )
       SELECT count(*)::text AS acknowledged FROM acked`,
      [hex(device), through],
    )
    return Number(row?.acknowledged ?? 0)
  }

  async purge(retentionDays: number): Promise<number> {
    // Their deliveries go with them (ON DELETE CASCADE).
    const [row] = await this.#db.query<{ purged: string }>(
      `WITH gone AS (DELETE FROM envelopes WHERE created_on < current_date - $1::int RETURNING 1)
       SELECT count(*)::text AS purged FROM gone`,
      [retentionDays],
    )
    return Number(row?.purged ?? 0)
  }

  async fetch(device: Uint8Array, after: string | null, limit: number): Promise<StoredEnvelope[]> {
    const rows = await this.#db.query<EnvelopeRow>(
      `SELECT e.id::text AS id, e.seq::text AS seq, e.kind, encode(e.group_id, 'hex') AS group_id, e.epoch::text AS epoch, encode(e.body, 'hex') AS body
         FROM deliveries d JOIN envelopes e ON e.id = d.envelope_id
        WHERE d.device_id = decode($1, 'hex') AND e.id > COALESCE($2::bigint, 0)
        ORDER BY e.id
        LIMIT $3`,
      [hex(device), after, limit],
    )
    return rows.map((row) => ({
      ...placedOf(row),
      header: {
        kind: kindOf(row.kind),
        ...(row.group_id === null ? {} : { group: bytes(row.group_id) }),
        ...(row.epoch === null ? {} : { epoch: Number(row.epoch) }),
      },
      body: bytes(row.body),
    }))
  }
}
