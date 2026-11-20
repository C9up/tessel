import { getDb } from '@c9up/atlas/services/db'
import { deliverOutboxRow } from '#modules/shared/infrastructure/events.js'
import { claimOutboxBatch, deleteOutbox, type OutboxDb, type OutboxRow, rescheduleOutbox } from '#modules/shared/infrastructure/outbox.js'

/**
 * Le relais de l'outbox (repris de nadeo) : une boucle de fond réserve les lignes dues,
 * les redélivre aux abonnés, puis les efface en cas de succès ou les repousse avec un
 * délai croissant. Au moins une fois ; le garde de déduplication rend la reprise sûre.
 * Démarré par `bin/server.ts`, jamais par le harnais de test.
 */
const POLL_MS = 10_000
const BATCH = 50
const LEASE_SEC = 5 * 60
const BASE_BACKOFF_SEC = 30
const MAX_BACKOFF_SEC = 60 * 60
const MAX_ATTEMPTS = 20
/** Lettre morte : la ligne reste lisible pour l'exploitation, mais n'est plus rejouée. */
const PARK_SEC = 100 * 365 * 24 * 60 * 60

function backoffSeconds(attempt: number): number {
  return Math.min(BASE_BACKOFF_SEC * 2 ** attempt, MAX_BACKOFF_SEC)
}

/** Dépendances injectables, pour tester `tick()` sans base ni émetteur. */
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

  /** Une passe : réserver, redélivrer, effacer ou repousser. Rend le nombre de lignes traitées. */
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

/** Le relais du processus, démarré par `bin/server.ts` et arrêté par `AppProvider`. */
export const outboxRelay = new OutboxRelay()
