import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { Devices } from '../domain/ports.js'
import type { KnownDevice } from '../domain/session.js'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')

/** The devices, in `devices` (story 03-01), with their revocation (story 03-02). */
export class SqlDevices implements Devices {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async find(deviceId: Uint8Array): Promise<KnownDevice | null> {
    const [row] = await this.#db.query<{ account_id: string; name: string; certificate: string; revoked: boolean }>(
      `SELECT d.account_id::text AS account_id, a.name, encode(d.certificate, 'hex') AS certificate, d.revoked_on IS NOT NULL AS revoked
         FROM devices d JOIN accounts a ON a.id = d.account_id
        WHERE d.id = decode($1, 'hex')`,
      [hex(deviceId)],
    )
    if (row === undefined) return null
    return {
      deviceId,
      accountId: row.account_id,
      name: row.name,
      certificate: new Uint8Array(Buffer.from(row.certificate, 'hex')),
      revoked: row.revoked,
    }
  }

  async revoke(deviceId: Uint8Array): Promise<boolean> {
    // One statement, so one transaction: no revoked device keeps a session, nor
    // KeyPackages for anyone to add it to a group with (story 03-03).
    const [row] = await this.#db.query<{ revoked: string }>(
      `WITH revoked AS (
         UPDATE devices SET revoked_on = current_date
          WHERE id = decode($1, 'hex') AND revoked_on IS NULL
         RETURNING id
       ), closed AS (
         DELETE FROM sessions WHERE device_id IN (SELECT id FROM revoked)
       ), unlisted AS (
         DELETE FROM key_packages WHERE device_id IN (SELECT id FROM revoked)
       )
       SELECT count(*)::text AS revoked FROM revoked`,
      [hex(deviceId)],
    )
    return row?.revoked === '1'
  }
}
