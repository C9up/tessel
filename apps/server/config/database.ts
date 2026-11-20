import { defineConfig } from '@c9up/atlas'
import app from '@c9up/ream/services/app'
import { databaseUrl } from '#modules/shared/infrastructure/databaseUrl.js'
import env from '#start/env.js'

// Over mutual TLS only (sslmode=verify-full and a client certificate). Migrations are not run at startup: they
// run as the schema owner (scripts/migrate.sh), never as the application's role.
export default defineConfig({
  url: databaseUrl({
    host: env.get('DB_HOST'),
    port: env.get('DB_PORT'),
    database: env.get('DB_DATABASE'),
    user: env.get('DB_USER'),
    password: env.get('DB_PASSWORD'),
    rootCert: env.get('DB_SSL_ROOT_CERT'),
    cert: env.get('DB_SSL_CERT'),
    key: env.get('DB_SSL_KEY'),
  }),
  migrations: {
    // Lazy: `app.migrationsPath()` throws until the app root is set.
    get paths() {
      return [app.migrationsPath()]
    },
  },
})
