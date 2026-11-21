import { Migration } from '@c9up/atlas'

/**
 * Acknowledgements and purge (story 04-04). Once every recipient has acknowledged an
 * envelope, its body and header go: what stays is its deduplication key (sending
 * device, send identifier) and its place in its group, so that the same envelope sent
 * again is answered and not stored twice. The purge removes the rest after the
 * retention period.
 */
export default class EnvelopeTombstones extends Migration {
  async up() {
    this.schema.raw('ALTER TABLE envelopes ALTER COLUMN body DROP NOT NULL')
    this.schema.raw('ALTER TABLE envelopes ALTER COLUMN kind DROP NOT NULL')
    this.schema.raw('CREATE INDEX envelopes_created_idx ON envelopes (created_on)')
  }

  async down() {
    this.schema.raw('DROP INDEX envelopes_created_idx')
    this.schema.raw('DELETE FROM envelopes WHERE body IS NULL')
    this.schema.raw('ALTER TABLE envelopes ALTER COLUMN kind SET NOT NULL')
    this.schema.raw('ALTER TABLE envelopes ALTER COLUMN body SET NOT NULL')
  }
}
