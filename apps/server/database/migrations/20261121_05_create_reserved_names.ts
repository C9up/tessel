import { Migration } from '@c9up/atlas'

/**
 * Deleted accounts (story 03-07): nothing of them stays but their name, reserved for a
 * while so that nobody takes it to pass for them, then freed. Both columns are in the
 * metadata register (story 15-02).
 */
export default class CreateReservedNames extends Migration {
  async up() {
    this.schema.raw(`
      CREATE TABLE reserved_names (
        name text PRIMARY KEY,
        -- The first day the name is free again.
        reserved_until date NOT NULL
      )
    `)
  }

  async down() {
    this.schema.raw('DROP TABLE reserved_names')
  }
}
