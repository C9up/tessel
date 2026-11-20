import { Migration } from '@c9up/atlas'

/** Le rôle sous lequel l'application tourne. Il ne possède rien. */
const APP_ROLE = 'app'

/**
 * Ce que le rôle de l'application peut faire (repris de Klinivia). Les migrations
 * tournent sous le propriétaire du schéma ; l'application tourne sous `app`.
 *
 * Toute table que le propriétaire crée à partir d'ici est lisible et modifiable par
 * `app`. Une table qui doit en refuser davantage le reprend dans sa propre migration.
 */
export default class GrantApplicationRole extends Migration {
  async up() {
    this.schema.raw(`ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ${APP_ROLE}`)
    this.schema.raw(`ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO ${APP_ROLE}`)
  }

  async down() {
    this.schema.raw(`ALTER DEFAULT PRIVILEGES IN SCHEMA public REVOKE SELECT, INSERT, UPDATE, DELETE ON TABLES FROM ${APP_ROLE}`)
    this.schema.raw(`ALTER DEFAULT PRIVILEGES IN SCHEMA public REVOKE USAGE, SELECT ON SEQUENCES FROM ${APP_ROLE}`)
  }
}
