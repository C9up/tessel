import { Migration } from '@c9up/atlas'

/**
 * The domain events outbox (nadeo model, without tenant).
 *
 * Each `DomainEvent.emit()` writes a row here before the fast delivery; a background
 * relay replays the pending rows, so that a listener that throws or an abrupt stop
 * does not lose the side effect. The table keeps only work in progress.
 */
export default class CreateOutbox extends Migration {
  async up() {
    this.schema.createTable('outbox', (t) => {
      t.uuid('id').primary()
      t.text('event_type').notNullable()
      t.text('payload').notNullable()
      t.timestamptz('created_at').notNullable().defaultTo(this.now())
      t.timestamptz('available_at').notNullable().defaultTo(this.now())
      t.integer('attempts').notNullable().defaultTo(0)
      t.text('last_error').nullable()
      t.index(['available_at'], 'outbox_available_idx')
    })
  }

  async down() {
    this.schema.dropTable('outbox')
  }
}
