import { Migration } from '@c9up/atlas'

/**
 * L'outbox des événements du domaine (modèle nadeo, sans tenant).
 *
 * Chaque `DomainEvent.emit()` écrit une ligne ici avant la livraison rapide ; un relais
 * de fond rejoue les lignes en attente, pour qu'un écouteur qui lève ou un arrêt brutal
 * ne perde pas l'effet de bord. La table ne garde que le travail en cours.
 */
export default class CreateOutbox extends Migration {
  async up() {
    this.schema.createTable('outbox', (t) => {
      t.uuid('id').primary()
      // Chiffrées par BlackGate (deploy/blackgate/columns.yaml) : la colonne porte le chiffré.
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
