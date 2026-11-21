import { Migration } from '@c9up/atlas'

/**
 * The order of each group (story 04-02): its current epoch, as the server accepted its
 * commits, and the next sequence number. One commit per epoch: the row's lock serializes
 * the envelopes of a group, on every instance of the server. Each column is in the
 * metadata register (story 15-02).
 */
export default class OrderGroupEnvelopes extends Migration {
  async up() {
    this.schema.raw(`
      CREATE TABLE delivery_groups (
        -- The MLS group identifier (random), as envelope headers carry it.
        group_id bytea PRIMARY KEY,
        -- The epoch the next commit must be made in.
        epoch bigint NOT NULL,
        next_seq bigint NOT NULL
      )
    `)
    // The envelope's place in its group; none for a Welcome.
    this.schema.raw('ALTER TABLE envelopes ADD COLUMN seq bigint')
  }

  async down() {
    this.schema.raw('ALTER TABLE envelopes DROP COLUMN seq')
    this.schema.raw('DROP TABLE delivery_groups')
  }
}
