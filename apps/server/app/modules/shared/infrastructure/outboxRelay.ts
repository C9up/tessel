import { getDb } from '@c9up/atlas/services/db'
import { deliverOutboxRow } from '#modules/shared/infrastructure/events.js'
import { claimOutboxBatch, deleteOutbox, type OutboxDb, type OutboxRow, rescheduleOutbox } from '#modules/shared/infrastructure/outbox.js'

/**
 * The outbox relay (taken from nadeo): a background loop claims the due rows,
 * redelivers them to the subscribers, then deletes them on success or pushes them back
 * with a growing delay. At least once; the deduplication guard makes the retry safe.
 * Started by `bin/server.ts`, never by the test harness.
 */
const POLL_MS = 10_000
const BATCH = 50
const LEASE_SEC = 5 * 60
const BASE_BACKOFF_SEC = 30
const MAX_BACKOFF_SEC = 60 * 60
const MAX_ATTEMPTS = 20
/** Dead letter: the row stays readable for operations, but is no longer replayed. */
const PARK_SEC = 100 * 365 * 24 * 60 * 60

function backoffSeconds(attempt: number): number {
  return Math.min(BASE_BACKOFF_SEC * 2 ** attempt, MAX_BACKOFF_SEC)
}

/** Injectable dependencies, to test `tick()` without a database or an emitter. */
export interface OutboxRelayDeps {
  db: () => Promise<OutboxDb>
  claim: typeof claimOutboxBatch
  del: typeof deleteOutbox
  reschedule: typeof rescheduleOutbox
  deliver: typeof deliverOutboxRow
}

const defaultDeps: OutboxRelayDeps = {
  db: async () => {
    const conn = getDb()
    if (!conn) throw new Error('[outbox] relay started before AtlasProvider.boot()')
    return conn
  },
  claim: claimOutboxBatch,
  del: deleteOutbox,
  reschedule: rescheduleOutbox,
  deliver: deliverOutboxRow,
}

export class OutboxRelay {
  #timer: ReturnType<typeof setTimeout> | null = null
  #running = false
  #stopped = false
  readonly #deps: OutboxRelayDeps

  constructor(deps: OutboxRelayDeps = defaultDeps) {
    this.#deps = deps
  }

  start(): void {
    if (this.#timer || this.#stopped) return
    this.#schedule(0)
  }

  stop(): void {
    this.#stopped = true
    if (this.#timer) {
      clearTimeout(this.#timer)
      this.#timer = null
    }
  }

  #schedule(delayMs: number): void {
    this.#timer = setTimeout(() => {
      void this.#loop()
    }, delayMs)
    this.#timer.unref?.()
  }

  async #loop(): Promise<void> {
    if (this.#stopped || this.#running) return
    this.#running = true
    try {
      await this.tick()
    } catch (err) {
      console.error('[outbox] relay tick failed:', err)
    } finally {
      this.#running = false
      if (!this.#stopped) this.#schedule(POLL_MS)
    }
  }

  /** One pass: claim, redeliver, delete or push back. Returns the number of rows handled. */
  async tick(): Promise<number> {
    const db = await this.#deps.db()
    const rows = await this.#deps.claim(db, {
      limit: BATCH,
      leaseSeconds: LEASE_SEC,
    })
    for (const row of rows) {
      try {
        const result = await this.#deps.deliver(row)
        if (result.ok) await this.#deps.del(db, row.id)
        else await this.#fail(db, row, result.error ?? 'delivery failed')
      } catch (err) {
        await this.#fail(db, row, err instanceof Error ? err.message : String(err))
      }
    }
    return rows.length
  }

  async #fail(db: OutboxDb, row: OutboxRow, error: string): Promise<void> {
    const nextAttempt = (Number(row.attempts) || 0) + 1
    if (nextAttempt >= MAX_ATTEMPTS) {
      await this.#deps.reschedule(db, row.id, {
        delaySeconds: PARK_SEC,
        error: `DEAD-LETTER after ${nextAttempt} attempts: ${error}`,
      })
      console.error(`[outbox] DEAD-LETTER ${row.event_type} id=${row.id} after ${nextAttempt} attempts: ${error}`)
    } else {
      await this.#deps.reschedule(db, row.id, {
        delaySeconds: backoffSeconds(nextAttempt),
        error,
      })
    }
  }
}

/** The process's relay, started by `bin/server.ts` and stopped by `AppProvider`. */
export const outboxRelay = new OutboxRelay()
