/** Where the database is, and the files that authenticate both ends of the TLS session (taken from Klinivia). */
export interface DatabaseTarget {
  readonly host: string
  readonly port: number
  readonly database: string
  readonly user: string
  readonly password?: string
  /** The CA the server's certificate must chain to. */
  readonly rootCert: string
  /** The client certificate and its key (unencrypted: atlas refuses an encrypted key). */
  readonly cert: string
  readonly key: string
}

/**
 * The PostgreSQL URL of a connection, always `sslmode=verify-full` with a
 * client certificate: the database is reached over mutual TLS only.
 *
 * Nothing in the configuration can lower it: atlas connects in plaintext,
 * without a word, when a URL carries no `sslmode`, and `require` or
 * `verify-ca` do not check the server's name. The mode is therefore not a
 * setting; the three files are required, and atlas refuses to connect when one
 * of them cannot be read.
 */
export function databaseUrl(target: DatabaseTarget): string {
  const url = new URL('postgres://localhost')
  url.hostname = target.host
  url.port = String(target.port)
  url.pathname = `/${encodeURIComponent(target.database)}`
  url.username = encodeURIComponent(target.user)
  if (target.password !== undefined) url.password = encodeURIComponent(target.password)
  url.searchParams.set('sslmode', 'verify-full')
  url.searchParams.set('sslrootcert', target.rootCert)
  url.searchParams.set('sslcert', target.cert)
  url.searchParams.set('sslkey', target.key)
  return url.toString()
}
