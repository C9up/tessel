import { type AsyncDatabaseConnection, createNapiConnection } from '@c9up/atlas'
import databaseConfig from '#config/database.js'
import env from '#start/env.js'

/**
 * A test connection: the application's own, as `app`, over mutual TLS
 * (scripts/stack.sh start). A single session, so that a test's writes and reads
 * share it (taken from Klinivia).
 */
export function connectTestDb(): Promise<AsyncDatabaseConnection> {
  const url = databaseConfig.url
  if (url === undefined) throw new Error('config/database.ts gives no url')
  return createNapiConnection(url, 1, 1)
}

/**
 * The application's URL with its TLS settings replaced: to prove that the database
 * refuses anything weaker than mutual TLS.
 */
export function weakenedUrl(tls: Record<string, string>): string {
  const url = new URL(databaseConfig.url ?? '')
  for (const key of ['sslmode', 'sslrootcert', 'sslcert', 'sslkey']) url.searchParams.delete(key)
  for (const [key, value] of Object.entries(tls)) url.searchParams.set(key, value)
  url.password = encodeURIComponent(env.get('DB_PASSWORD') ?? '')
  return url.toString()
}
