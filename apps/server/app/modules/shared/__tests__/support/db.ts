import { type AsyncDatabaseConnection, createNapiConnection } from '@c9up/atlas'
import databaseConfig from '#config/database.js'
import { databaseUrl } from '#modules/shared/infrastructure/databaseUrl.js'
import env from '#start/env.js'

/**
 * A test connection: the application's own, as `app`, through BlackGate over mutual
 * TLS (scripts/stack.sh start). A single session, so that a test's writes and reads
 * share it (taken from Klinivia).
 */
export function connectTestDb(): Promise<AsyncDatabaseConnection> {
  const url = databaseConfig.url
  if (url === undefined) throw new Error('config/database.ts gives no url')
  return createNapiConnection(url, 1, 1)
}

/**
 * The owner, on PostgreSQL itself, around BlackGate: what the database really
 * keeps. Tests only, to check that an encrypted column holds ciphertext.
 */
export function connectDirectDb(): Promise<AsyncDatabaseConnection> {
  const port = env.get('DB_DIRECT_PORT')
  const user = env.get('DB_OWNER_USER')
  if (port === undefined || user === undefined) throw new Error('DB_DIRECT_PORT / DB_OWNER_USER missing from .env (scripts/stack.sh start)')
  return createNapiConnection(
    databaseUrl({
      host: env.get('DB_HOST'),
      port,
      database: env.get('DB_DATABASE'),
      user,
      password: env.get('DB_OWNER_PASSWORD'),
      rootCert: env.get('DB_SSL_ROOT_CERT'),
      cert: env.get('DB_SSL_CERT'),
      key: env.get('DB_SSL_KEY'),
    }),
    1,
    1,
  )
}
