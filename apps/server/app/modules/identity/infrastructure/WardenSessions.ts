import type { AsyncDatabaseConnection } from '@c9up/atlas'
import { type AccessTokenStore, AccessTokensProvider, Secret, type StoredAccessToken } from '@c9up/warden'
import type { Sessions } from '../domain/ports.js'

/** How long a session lasts before the device signs in again. */
const SESSION_LIFETIME = '30 days'

/** The token's prefix, for secret scanners to recognise a leaked one. */
const TOKEN_PREFIX = 'tsn_'

/** The one kind of token `sessions` holds. */
const SESSION_TYPE = 'tessel_session'

/** Epoch milliseconds of a `date`, read by PostgreSQL: midnight UTC of that day. */
const EPOCH_MS = (column: string) => `(extract(epoch FROM ${column}) * 1000)::float8`

interface SessionRow {
  id: string
  device: string
  hash: string
  created_at: number
  last_used_at: number | null
  expires_at: number
}

/**
 * warden's token table, over `sessions`. The owner of a token is a device (its
 * identifier, hex), never an account. Dates are kept to the day, as everywhere else
 * (ADR-006): a session expires at the start of the day its lifetime ends, up to a day
 * early, and its last use is known to the day only.
 */
class SqlSessionStore implements AccessTokenStore {
  readonly #db: AsyncDatabaseConnection

  constructor(db: AsyncDatabaseConnection) {
    this.#db = db
  }

  async create(token: StoredAccessToken): Promise<void> {
    if (token.expiresAt === null) throw new Error('a session always expires')
    await this.#db.execute(
      `INSERT INTO sessions (id, device_id, hash, expires_on)
       VALUES ($1, decode($2, 'hex'), $3, (to_timestamp($4::float8 / 1000) AT TIME ZONE 'UTC')::date)`,
      [token.identifier, String(token.tokenableId), token.hash, token.expiresAt],
    )
  }

  async find(identifier: string): Promise<StoredAccessToken | null> {
    const [row] = await this.#select('id = $1', identifier)
    return row ?? null
  }

  async touch(identifier: string): Promise<void> {
    // Once a day at most: the day is all the server keeps.
    await this.#db.execute('UPDATE sessions SET last_used_on = current_date WHERE id = $1 AND last_used_on IS DISTINCT FROM current_date', [identifier])
  }

  async delete(identifier: string): Promise<boolean> {
    const rows = await this.#db.query<{ id: string }>('DELETE FROM sessions WHERE id = $1 RETURNING id', [identifier])
    return rows.length > 0
  }

  listForUser(deviceHex: string | number): Promise<StoredAccessToken[]> {
    return this.#select(`device_id = decode($1, 'hex')`, String(deviceHex))
  }

  async #select(where: string, value: string): Promise<StoredAccessToken[]> {
    const rows = await this.#db.query<SessionRow>(
      `SELECT id, encode(device_id, 'hex') AS device, hash,
              ${EPOCH_MS('created_on')} AS created_at,
              ${EPOCH_MS('last_used_on')} AS last_used_at,
              ${EPOCH_MS('expires_on')} AS expires_at
         FROM sessions WHERE ${where}`,
      [value],
    )
    return rows.map((row) => ({
      identifier: row.id,
      tokenableId: row.device,
      type: SESSION_TYPE,
      name: null,
      hash: row.hash,
      abilities: ['*'],
      createdAt: row.created_at,
      updatedAt: row.created_at,
      lastUsedAt: row.last_used_at,
      expiresAt: row.expires_at,
    }))
  }
}

/**
 * The sessions, as warden's opaque access tokens: `tsn_<identifier>.<secret>`, of which
 * the server keeps only `sha256(secret)`. A stolen copy of the table opens no session.
 */
export class WardenSessions implements Sessions {
  readonly #tokens: AccessTokensProvider

  constructor(db: AsyncDatabaseConnection) {
    this.#tokens = new AccessTokensProvider({ store: new SqlSessionStore(db), prefix: TOKEN_PREFIX, type: SESSION_TYPE, expiresIn: SESSION_LIFETIME })
  }

  async open(deviceId: Uint8Array): Promise<string> {
    const token = await this.#tokens.create({ id: Buffer.from(deviceId).toString('hex') })
    const value = token.value
    if (value === undefined) throw new Error('a token just minted has a value')
    return value.release()
  }

  async find(token: string): Promise<{ deviceId: Uint8Array } | null> {
    const found = await this.#tokens.verify(new Secret(token))
    return found === null ? null : { deviceId: new Uint8Array(Buffer.from(String(found.tokenableId), 'hex')) }
  }

  close(token: string): Promise<boolean> {
    return this.#tokens.invalidate(new Secret(token))
  }
}
