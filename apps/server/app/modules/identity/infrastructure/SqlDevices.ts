import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { CertifiedDevice } from '../domain/account.js'
import type { Devices } from '../domain/ports.js'
import { type DeviceEntry, type DeviceKind, isDeviceKind, type KnownDevice } from '../domain/session.js'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')

/** The devices, in `devices` (story 03-01), with their revocation (story 03-02). */
export class SqlDevices implements Devices {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async find(deviceId: Uint8Array): Promise<KnownDevice | null> {
    const [row] = await this.#db.query<{ account_id: string; name: string; certificate: string; account_key: string; revoked: boolean }>(
      `SELECT d.account_id::text AS account_id, a.name, encode(d.certificate, 'hex') AS certificate,
              encode(a.account_key, 'hex') AS account_key, d.revoked_on IS NOT NULL AS revoked
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
      accountKey: new Uint8Array(Buffer.from(row.account_key, 'hex')),
      revoked: row.revoked,
    }
  }

  async signedIn(deviceId: Uint8Array, kind: DeviceKind): Promise<void> {
    await this.#db.execute(`UPDATE devices SET kind = $2, last_active_on = current_date WHERE id = decode($1, 'hex')`, [hex(deviceId), kind])
  }

  async list(accountId: string): Promise<DeviceEntry[]> {
    const rows = await this.#db.query<{ id: string; kind: string | null; created_on: string; last_active_on: string | null }>(
      `SELECT encode(id, 'hex') AS id, kind, to_char(created_on, 'YYYY-MM-DD') AS created_on,
              to_char(last_active_on, 'YYYY-MM-DD') AS last_active_on
         FROM devices WHERE account_id = $1::uuid AND revoked_on IS NULL
        ORDER BY created_on, id`,
      [accountId],
    )
    return rows.map((row) => ({
      deviceId: new Uint8Array(Buffer.from(row.id, 'hex')),
      kind: isDeviceKind(row.kind) ? row.kind : null,
      createdOn: row.created_on,
      lastActiveOn: row.last_active_on,
    }))
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

  async live(accountId: string): Promise<number> {
    const [row] = await this.#db.query<{ live: string }>('SELECT count(*)::text AS live FROM devices WHERE account_id = $1::uuid AND revoked_on IS NULL', [
      accountId,
    ])
    return Number(row?.live ?? 0)
  }

  async add(accountId: string, device: CertifiedDevice): Promise<boolean> {
    // Only into the account whose key signed the certificate; a known identifier or key
    // inserts nothing.
    const rows = await this.#db.query<{ id: string }>(
      `INSERT INTO devices (id, account_id, device_key, certificate)
       SELECT decode($2, 'hex'), a.id, decode($3, 'hex'), decode($4, 'hex')
         FROM accounts a WHERE a.id = $1::uuid AND a.account_key = decode($5, 'hex')
       ON CONFLICT DO NOTHING
       RETURNING encode(id, 'hex') AS id`,
      [accountId, hex(device.deviceId), hex(device.deviceKey), hex(device.certificate), hex(device.accountKey)],
    )
    return rows.length > 0
  }
}
