/**
 * The durable side of the outbox (taken from nadeo, without tenant).
 *
 * Functions over a minimal `OutboxDb` handle, which the real atlas connection
 * satisfies by its shape: tests pass an in-memory double without a cast.
 *
 * All time is the DATABASE's (`now()`): neither the client's clock nor any skew
 * between client and database comes into play. Each parameter carries an explicit
 * cast (`::uuid`, `::double precision`), since the driver binds a JS string as `text`.
 *
 * `event_type`, `payload` and `last_error` are encrypted by BlackGate: the database
 * sees only ciphertext there, the application reads them in clear.
 */
export interface OutboxRow {
  id: string
  event_type: string
  /** The event's JSON, as text (the column holds BlackGate's ciphertext). */
  payload: unknown
  attempts: number
}

/** The database surface the outbox needs: a subset of the atlas connection. */
export interface OutboxDb {
  query(sql: string, params?: unknown[]): Promise<OutboxRow[]>
  execute(sql: string, params?: unknown[]): Promise<{ rowsAffected: number }>
}

export interface OutboxEntry {
  id: string
  eventType: string
  payload: unknown
  /** Seconds before the relay may take the row (time left to the fast path). */
  delaySeconds: number
}

/** Keep an event pending, which the relay may take after `delaySeconds`. */
export async function enqueueOutbox(db: OutboxDb, entry: OutboxEntry): Promise<void> {
  await db.execute(
    `INSERT INTO outbox (id, event_type, payload, created_at, available_at, attempts)
     VALUES ($1::uuid, $2, $3, now(), now() + make_interval(secs => $4::double precision), 0)`,
    [entry.id, entry.eventType, JSON.stringify(entry.payload), entry.delaySeconds],
  )
}

/**
 * CLAIM at most `limit` due rows: `FOR UPDATE SKIP LOCKED` takes those no other relay
 * holds, and the same query pushes their `available_at` back by `leaseSeconds`. A relay
 * that dies mid-delivery lets the lease expire: the row becomes due again.
 */
export function claimOutboxBatch(db: OutboxDb, opts: { limit: number; leaseSeconds: number }): Promise<OutboxRow[]> {
  return db.query(
    `UPDATE outbox SET available_at = now() + make_interval(secs => $2::double precision)
     WHERE id IN (
       SELECT id FROM outbox
       WHERE available_at <= now()
       ORDER BY created_at
       LIMIT $1
       FOR UPDATE SKIP LOCKED
     )
     RETURNING id, event_type, payload, attempts`,
    [opts.limit, opts.leaseSeconds],
  )
}

/** Remove a delivered row: the outbox keeps only work in progress. */
export async function deleteOutbox(db: OutboxDb, id: string): Promise<void> {
  await db.execute('DELETE FROM outbox WHERE id = $1::uuid', [id])
}

/** Count a failure, record the error, and push the next attempt back by `delaySeconds`. */
export async function rescheduleOutbox(db: OutboxDb, id: string, opts: { delaySeconds: number; error: string }): Promise<void> {
  await db.execute(
    `UPDATE outbox SET attempts = attempts + 1,
        available_at = now() + make_interval(secs => $2::double precision),
        last_error = $3
     WHERE id = $1::uuid`,
    [id, opts.delaySeconds, opts.error.slice(0, 2000)],
  )
}
