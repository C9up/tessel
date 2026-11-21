import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { Profiles } from '../domain/ports.js'

/** The sealed profiles, in `profiles`: a blob per account and the day it changed. */
export class SqlProfiles implements Profiles {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async put(accountId: string, blob: Uint8Array): Promise<void> {
    await this.#db.execute(
      `INSERT INTO profiles (account_id, blob) VALUES ($1::uuid, decode($2, 'hex'))
       ON CONFLICT (account_id) DO UPDATE SET blob = EXCLUDED.blob, updated_on = current_date`,
      [accountId, Buffer.from(blob).toString('hex')],
    )
  }

  async get(name: string): Promise<Uint8Array | null> {
    const [row] = await this.#db.query<{ blob: string }>(
      `SELECT encode(p.blob, 'hex') AS blob FROM profiles p JOIN accounts a ON a.id = p.account_id WHERE a.name = $1`,
      [name],
    )
    return row === undefined ? null : new Uint8Array(Buffer.from(row.blob, 'hex'))
  }
}
