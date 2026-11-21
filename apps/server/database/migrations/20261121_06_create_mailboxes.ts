import { Migration } from '@c9up/atlas'

/**
 * The mailboxes (story 04-01). An envelope is kept once, with its public MLS header
 * (kind, group, epoch) and its encrypted body; `deliveries` says which devices it is
 * for. Each column is in the metadata register (story 15-02).
 *
 * No foreign key to `devices`: that table is the identity module's. An envelope for a
 * device that does not exist is never fetched, and goes with the purge (story 04-04).
 */
export default class CreateMailboxes extends Migration {
  async up() {
    this.schema.raw(`
      CREATE TABLE envelopes (
        id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
        -- The deduplication key (story 02-07): the sending device and its send identifier.
        sender_device bytea NOT NULL CHECK (octet_length(sender_device) = 16),
        send_id bytea NOT NULL CHECK (octet_length(send_id) = 16),
        -- 1 application, 2 proposal, 3 commit, 4 welcome.
        kind smallint NOT NULL CHECK (kind BETWEEN 1 AND 4),
        -- The MLS group identifier (random) and epoch; none for a Welcome.
        group_id bytea,
        epoch bigint,
        -- The MLS message: encrypted by the devices, opaque to the server.
        body bytea NOT NULL,
        created_on date NOT NULL DEFAULT current_date,
        UNIQUE (sender_device, send_id)
      )
    `)
    this.schema.raw(`
      CREATE TABLE deliveries (
        device_id bytea NOT NULL CHECK (octet_length(device_id) = 16),
        envelope_id bigint NOT NULL REFERENCES envelopes (id) ON DELETE CASCADE,
        PRIMARY KEY (device_id, envelope_id)
      )
    `)
    this.schema.raw('CREATE INDEX deliveries_envelope_idx ON deliveries (envelope_id)')
  }

  async down() {
    this.schema.raw('DROP TABLE deliveries')
    this.schema.raw('DROP TABLE envelopes')
  }
}
