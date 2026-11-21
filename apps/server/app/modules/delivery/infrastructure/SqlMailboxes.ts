import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { EnvelopeHeader, EnvelopeKind, StoredEnvelope } from '../domain/envelope.js'
import type { Mailboxes } from '../domain/ports.js'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')
const bytes = (text: string) => new Uint8Array(Buffer.from(text, 'hex'))

/** How each kind is kept: a small number, the MLS content type's order. */
const KINDS: readonly EnvelopeKind[] = ['application', 'proposal', 'commit', 'welcome']
const kindOf = (code: number): EnvelopeKind => KINDS[code - 1] ?? 'application'

interface EnvelopeRow {
  id: string
  kind: number
  group_id: string | null
  epoch: string | null
  body: string
}

/**
 * The mailboxes, in `envelopes` (each envelope once) and `deliveries` (who it is for).
 * What each column says is in the metadata register (story 15-02).
 */
export class SqlMailboxes implements Mailboxes {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async store(sender: Uint8Array, sendId: Uint8Array, header: EnvelopeHeader, body: Uint8Array, recipients: readonly Uint8Array[]): Promise<string> {
    // One statement, so one transaction: an envelope never without its recipients. The
    // same (device, send identifier) inserts nothing, and the first envelope's
    // identifier comes back.
    const [row] = await this.#db.query<{ id: string }>(
      `WITH stored AS (
         INSERT INTO envelopes (sender_device, send_id, kind, group_id, epoch, body)
         VALUES (decode($1, 'hex'), decode($2, 'hex'), $3::smallint, decode($4, 'hex'), $5::bigint, decode($6, 'hex'))
         ON CONFLICT (sender_device, send_id) DO NOTHING
         RETURNING id
       ), addressed AS (
         INSERT INTO deliveries (envelope_id, device_id)
         SELECT s.id, decode(r, 'hex') FROM stored s, jsonb_array_elements_text($7::jsonb) r
       )
       SELECT id::text AS id FROM stored
       UNION ALL
       SELECT id::text FROM envelopes
        WHERE sender_device = decode($1, 'hex') AND send_id = decode($2, 'hex') AND NOT EXISTS (SELECT 1 FROM stored)`,
      [
        hex(sender),
        hex(sendId),
        KINDS.indexOf(header.kind) + 1,
        header.group === undefined ? null : hex(header.group),
        header.epoch ?? null,
        hex(body),
        JSON.stringify(recipients.map(hex)),
      ],
    )
    if (row === undefined) throw new Error('an envelope stored or found has an identifier')
    return row.id
  }

  async fetch(device: Uint8Array, after: string | null, limit: number): Promise<StoredEnvelope[]> {
    const rows = await this.#db.query<EnvelopeRow>(
      `SELECT e.id::text AS id, e.kind, encode(e.group_id, 'hex') AS group_id, e.epoch::text AS epoch, encode(e.body, 'hex') AS body
         FROM deliveries d JOIN envelopes e ON e.id = d.envelope_id
        WHERE d.device_id = decode($1, 'hex') AND e.id > COALESCE($2::bigint, 0)
        ORDER BY e.id
        LIMIT $3`,
      [hex(device), after, limit],
    )
    return rows.map((row) => ({
      id: row.id,
      header: {
        kind: kindOf(row.kind),
        ...(row.group_id === null ? {} : { group: bytes(row.group_id) }),
        ...(row.epoch === null ? {} : { epoch: Number(row.epoch) }),
      },
      body: bytes(row.body),
    }))
  }
}
