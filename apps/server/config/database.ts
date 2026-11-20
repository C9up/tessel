import { defineConfig } from '@c9up/atlas'
import app from '@c9up/ream/services/app'
import { databaseUrl } from '#modules/shared/infrastructure/databaseUrl.js'
import env from '#start/env.js'

// À travers BlackGate seulement, en TLS mutuel. Les migrations ne se jouent pas au
// démarrage : elles tournent sous le propriétaire du schéma (scripts/migrate.sh),
// jamais sous le rôle de l'application.
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
    // Paresseux : `app.migrationsPath()` lève tant que la racine de l'app n'est pas posée.
    get paths() {
      return [app.migrationsPath()]
    },
  },
})
