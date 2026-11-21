import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { ClaimedKeyPackage, StoredKeyPackage } from '../domain/keyPackages.js'
import type { KeyPackages } from '../domain/ports.js'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')
const bytes = (text: string) => new Uint8Array(Buffer.from(text, 'hex'))

/** A KeyPackage still good to hand out: its last valid day is not over. */
const VALID = 'not_after > current_date'

/**
 * The directory, in `key_packages`. Each KeyPackage is kept with its device, its kind
 * and the day its validity ends; nothing else (metadata register, story 15-02).
 */
export class SqlKeyPackages implements KeyPackages {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async store(deviceId: Uint8Array, upload: { publish: StoredKeyPackage[]; lastResort?: StoredKeyPackage; withdraw: readonly Uint8Array[] }): Promise<number> {
    const added = [...upload.publish, ...(upload.lastResort === undefined ? [] : [upload.lastResort])].map((entry) => ({
      reference: hex(entry.reference),
      keyPackage: hex(entry.keyPackage),
      lastResort: entry.lastResort,
      notAfter: entry.notAfter,
    }))
    // One statement, so one transaction. Its parts see the table as it was before: the
    // stock is counted from that, minus what goes, plus what comes.
    const [row] = await this.#db.query<{ stock: string }>(
      `WITH withdrawn AS (
         DELETE FROM key_packages
          WHERE device_id = decode($1, 'hex')
            AND (reference IN (SELECT decode(r, 'hex') FROM jsonb_array_elements_text($2::jsonb) r)
                 OR (last_resort AND $3::boolean))
         RETURNING last_resort, ${VALID} AS valid
       ), added AS (
         INSERT INTO key_packages (reference, device_id, key_package, last_resort, not_after)
         SELECT decode(e->>'reference', 'hex'), decode($1, 'hex'), decode(e->>'keyPackage', 'hex'),
                (e->>'lastResort')::boolean, (to_timestamp((e->>'notAfter')::float8) AT TIME ZONE 'UTC')::date
           FROM jsonb_array_elements($4::jsonb) e
         ON CONFLICT (reference) DO NOTHING
         RETURNING last_resort, ${VALID} AS valid
       )
       SELECT ((SELECT count(*) FROM key_packages WHERE device_id = decode($1, 'hex') AND NOT last_resort AND ${VALID})
             - (SELECT count(*) FROM withdrawn WHERE NOT last_resort AND valid)
             + (SELECT count(*) FROM added WHERE NOT last_resort AND valid))::text AS stock`,
      [hex(deviceId), JSON.stringify(upload.withdraw.map(hex)), upload.lastResort !== undefined, JSON.stringify(added)],
    )
    return Number(row?.stock ?? 0)
  }

  async stock(deviceId: Uint8Array): Promise<number> {
    const [row] = await this.#db.query<{ stock: string }>(
      `SELECT count(*)::text AS stock FROM key_packages WHERE device_id = decode($1, 'hex') AND NOT last_resort AND ${VALID}`,
      [hex(deviceId)],
    )
    return Number(row?.stock ?? 0)
  }

  async claim(name: string, except: Uint8Array): Promise<ClaimedKeyPackage[]> {
    // For each device, the single-use KeyPackage closest to its end is locked and deleted
    // (SKIP LOCKED: a concurrent claim takes the next one, never the same); a device with
    // none left gets its last-resort KeyPackage, which stays.
    const rows = await this.#db.query<{ device: string; key_package: string; last_resort: boolean; stock_left: string }>(
      `WITH targets AS (
         SELECT d.id FROM devices d JOIN accounts a ON a.id = d.account_id
          WHERE a.name = $1 AND d.revoked_on IS NULL AND d.id <> decode($2, 'hex')
       ), taken AS (
         DELETE FROM key_packages
          WHERE reference IN (
            SELECT (SELECT k.reference FROM key_packages k
                     WHERE k.device_id = t.id AND NOT k.last_resort AND k.${VALID}
                     ORDER BY k.not_after, k.reference LIMIT 1 FOR UPDATE SKIP LOCKED)
              FROM targets t)
         RETURNING device_id, key_package
       ), fallback AS (
         SELECT DISTINCT ON (k.device_id) k.device_id, k.key_package
           FROM key_packages k JOIN targets t ON t.id = k.device_id
          WHERE k.last_resort AND k.${VALID} AND k.device_id NOT IN (SELECT device_id FROM taken)
          ORDER BY k.device_id, k.not_after DESC
       )
       SELECT encode(t.device_id, 'hex') AS device, encode(t.key_package, 'hex') AS key_package, false AS last_resort,
              ((SELECT count(*) FROM key_packages k WHERE k.device_id = t.device_id AND NOT k.last_resort AND k.${VALID}) - 1)::text AS stock_left
         FROM taken t
       UNION ALL
       SELECT encode(f.device_id, 'hex'), encode(f.key_package, 'hex'), true, '0' FROM fallback f`,
      [name, hex(except)],
    )
    return rows.map((row) => ({
      deviceId: bytes(row.device),
      keyPackage: bytes(row.key_package),
      lastResort: row.last_resort,
      stockLeft: Number(row.stock_left),
    }))
  }
}
