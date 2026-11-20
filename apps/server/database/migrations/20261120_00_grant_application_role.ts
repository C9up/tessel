import { Migration } from '@c9up/atlas'

/** The role the application runs as. It owns nothing. */
const APP_ROLE = 'app'

/**
 * What the application's role may do (taken from Klinivia). Migrations run as the
 * schema owner; the application runs as `app`.
 *
 * Every table the owner creates from here on is readable and writable by `app`. A
 * table that must refuse more takes it back in its own migration.
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
