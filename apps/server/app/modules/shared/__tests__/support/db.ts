import { type AsyncDatabaseConnection, createNapiConnection } from '@c9up/atlas'
import databaseConfig from '#config/database.js'
import { databaseUrl } from '#modules/shared/infrastructure/databaseUrl.js'
import env from '#start/env.js'

/**
 * Une connexion de test : celle de l'application, sous `app`, à travers BlackGate en
 * TLS mutuel (scripts/stack.sh start). Une seule session, pour que les écritures et
 * les lectures d'un test la partagent (repris de Klinivia).
 */
export function connectTestDb(): Promise<AsyncDatabaseConnection> {
  const url = databaseConfig.url
  if (url === undefined) throw new Error('config/database.ts gives no url')
  return createNapiConnection(url, 1, 1)
}

/**
 * Le propriétaire, sur PostgreSQL lui-même, autour de BlackGate : ce que la base
 * garde vraiment. Tests seulement, pour vérifier qu'une colonne chiffrée porte du chiffré.
 */
export function connectDirectDb(): Promise<AsyncDatabaseConnection> {
  const port = env.get('DB_DIRECT_PORT')
  const user = env.get('DB_OWNER_USER')
  if (port === undefined || user === undefined) throw new Error('DB_DIRECT_PORT / DB_OWNER_USER absent de .env (scripts/stack.sh start)')
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
