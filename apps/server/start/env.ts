import { Env } from '@c9up/ream'

export default await Env.create(new URL('../', import.meta.url), {
  APP_NAME: Env.schema.string(),
  // Signe cookies et jetons. Obligatoire : sans elle l'app refuse de démarrer.
  APP_KEY: Env.schema.secret(),
  NODE_ENV: Env.schema.enum(['development', 'production', 'test'] as const),
  PORT: Env.schema.number(),
  // PostgreSQL à travers BlackGate seulement, en TLS mutuel (scripts/stack.sh start écrit ces lignes).
  DB_HOST: Env.schema.string({ format: 'host' }),
  DB_PORT: Env.schema.number(),
  DB_DATABASE: Env.schema.string(),
  DB_USER: Env.schema.string(),
  DB_PASSWORD: Env.schema.string.optional(),
  DB_SSL_ROOT_CERT: Env.schema.string(),
  DB_SSL_CERT: Env.schema.string(),
  DB_SSL_KEY: Env.schema.string(),
  // Le propriétaire du schéma (scripts/migrate.sh) et PostgreSQL en direct : tests seulement.
  DB_OWNER_USER: Env.schema.string.optional(),
  DB_OWNER_PASSWORD: Env.schema.string.optional(),
  DB_DIRECT_PORT: Env.schema.number().optional(),
  REDIS_URL: Env.schema.string(),
})
