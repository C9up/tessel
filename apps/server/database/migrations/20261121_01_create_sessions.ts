import { Migration } from '@c9up/atlas'

/**
 * Sessions and device revocation (story 03-02). A session is bound to one device and
 * keeps the hash of its secret, never the token. Dates to the day only (ADR-006); each
 * column is in the metadata register (story 15-02).
 */
export default class CreateSessions extends Migration {
  async up() {
    this.schema.raw('ALTER TABLE devices ADD COLUMN revoked_on date')
    this.schema.raw(`
      CREATE TABLE sessions (
        -- warden's 128-bit random identifier, hex: the public half of the token.
        id text PRIMARY KEY CHECK (id ~ '^[0-9a-f]{32}$'),
        device_id bytea NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
        -- sha256 of the token's secret, hex: a copy of the table opens no session.
        hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
        created_on date NOT NULL DEFAULT current_date,
        last_used_on date,
        expires_on date NOT NULL
      )
    `)
    this.schema.raw('CREATE INDEX sessions_device_idx ON sessions (device_id)')
  }

  async down() {
    this.schema.raw('DROP TABLE sessions')
    this.schema.raw('ALTER TABLE devices DROP COLUMN revoked_on')
  }
}
