import { Env } from '@c9up/ream'

export default await Env.create(new URL('../', import.meta.url), {
  APP_NAME: Env.schema.string(),
  // Signs cookies and tokens. Required: without it the app refuses to start.
  APP_KEY: Env.schema.secret(),
  NODE_ENV: Env.schema.enum(['development', 'production', 'test'] as const),
  PORT: Env.schema.number(),
  // The instance's domain, the `domain` of every `name@domain` it hosts (ADR-002).
  APP_DOMAIN: Env.schema.string({ format: 'host' }),
  // PostgreSQL over mutual TLS only (scripts/stack.sh start writes these lines).
  DB_HOST: Env.schema.string({ format: 'host' }),
  DB_PORT: Env.schema.number(),
  DB_DATABASE: Env.schema.string(),
  DB_USER: Env.schema.string(),
  DB_PASSWORD: Env.schema.string.optional(),
  DB_SSL_ROOT_CERT: Env.schema.string(),
  DB_SSL_CERT: Env.schema.string(),
  DB_SSL_KEY: Env.schema.string(),
  // The schema owner: scripts/migrate.sh only, never the application.
  DB_OWNER_USER: Env.schema.string.optional(),
  DB_OWNER_PASSWORD: Env.schema.string.optional(),
  REDIS_URL: Env.schema.string(),
})
