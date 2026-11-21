import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { NewAccount } from '../domain/account.js'
import type { Accounts } from '../domain/ports.js'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')

/**
 * The accounts and their devices, in `accounts` and `devices`. What each column says,
 * and why the server keeps it, is in the metadata register (story 15-02): the name, the
 * public keys, the certificate, the day of creation. Nothing else about a person.
 */
export class SqlAccounts implements Accounts {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async create(account: NewAccount): Promise<{ id: string } | null> {
    // One statement, so one transaction: no account without its device. A name or an
    // account key already there inserts nothing, and nothing comes back.
    const [row] = await this.#db.query<{ id: string }>(
      `WITH expired AS (
         DELETE FROM reserved_names WHERE reserved_until <= current_date
       ), account AS (
         INSERT INTO accounts (name, account_key)
         SELECT $1, decode($2, 'hex')
          WHERE NOT EXISTS (SELECT 1 FROM reserved_names WHERE name = $1 AND reserved_until > current_date)
         ON CONFLICT DO NOTHING
         RETURNING id
       ), device AS (
         INSERT INTO devices (id, account_id, device_key, certificate)
         SELECT decode($3, 'hex'), id, decode($4, 'hex'), decode($5, 'hex') FROM account
         RETURNING account_id
       )
       SELECT account_id::text AS id FROM device`,
      [account.name, hex(account.device.accountKey), hex(account.device.deviceId), hex(account.device.deviceKey), hex(account.device.certificate)],
    )
    return row ?? null
  }

  async delete(accountId: string, name: string, reservedDays: number): Promise<boolean> {
    // One statement, so one transaction: the account goes with everything that hangs
    // from it (devices, sessions, KeyPackages, profile: ON DELETE CASCADE), its name is
    // reserved, and reservations past their day are cleared on the way.
    const [row] = await this.#db.query<{ deleted: string }>(
      `WITH expired AS (
         DELETE FROM reserved_names WHERE reserved_until <= current_date
       ), deleted AS (
         DELETE FROM accounts WHERE id = $1::uuid AND name = $2 RETURNING name
       ), reserved AS (
         INSERT INTO reserved_names (name, reserved_until)
         SELECT name, current_date + $3::int FROM deleted
         ON CONFLICT (name) DO UPDATE SET reserved_until = EXCLUDED.reserved_until
       )
       SELECT count(*)::text AS deleted FROM deleted`,
      [accountId, name, reservedDays],
    )
    return row?.deleted === '1'
  }
}
