import { Migration } from '@c9up/atlas'

/**
 * Encrypted profiles (story 03-06): one sealed blob per account, which only the
 * account's devices and the contacts they shared its key with can open. The server keeps
 * the blob and the day it last changed; it never sees a name or a picture. Both columns
 * are in the metadata register (story 15-02).
 */
export default class CreateProfiles extends Migration {
  async up() {
    this.schema.raw(`
      CREATE TABLE profiles (
        account_id uuid PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
        -- Sealed by the devices under the account's profile key (XChaCha20-Poly1305).
        blob bytea NOT NULL,
        updated_on date NOT NULL DEFAULT current_date
      )
    `)
  }

  async down() {
    this.schema.raw('DROP TABLE profiles')
  }
}
