import { Migration } from '@c9up/atlas'

/**
 * Accounts and their devices (story 03-01). The server keeps a name, public keys and the
 * day things happened: no e-mail, no number, no display name (ADR-002, ADR-004). Each
 * column is in the metadata register (story 15-02).
 */
export default class CreateAccounts extends Migration {
  async up() {
    this.schema.raw(`
      CREATE TABLE accounts (
        id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
        -- The name of name@domain: the domain is the instance's (APP_DOMAIN).
        name text NOT NULL UNIQUE,
        account_key bytea NOT NULL UNIQUE,
        -- The day, not the hour: what Signal answers a subpoena with, and no more.
        created_on date NOT NULL DEFAULT current_date
      )
    `)
    this.schema.raw(`
      CREATE TABLE devices (
        -- The 128-bit random identifier the device drew itself.
        id bytea PRIMARY KEY CHECK (octet_length(id) = 16),
        account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
        device_key bytea NOT NULL UNIQUE,
        -- Signed by the account key: what any contact checks the device against.
        certificate bytea NOT NULL,
        created_on date NOT NULL DEFAULT current_date
      )
    `)
    this.schema.raw('CREATE INDEX devices_account_idx ON devices (account_id)')
  }

  async down() {
    this.schema.raw('DROP TABLE devices')
    this.schema.raw('DROP TABLE accounts')
  }
}
