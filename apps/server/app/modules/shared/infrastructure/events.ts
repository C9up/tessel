import { getDb } from '@c9up/atlas/services/db'
import emitter from '@c9up/ream/events/services/main'
import { type DomainEvent, setEventPublisher } from '#modules/shared/domain/events.js'
import { enqueueOutbox, type OutboxRow } from '#modules/shared/infrastructure/outbox.js'
import { redis } from '#modules/shared/infrastructure/redis.js'

// Every domain event is published here: the domain's `emit()` hands it over.
setEventPublisher(async (event) => {
  await enqueueForDurability(event)
  await emitter.dispatchEvent(event)
})

/**
 * "At most once" guard per `(subscriber, event)`, in three steps so that a FAILED
 * delivery stays replayable (taken from nadeo, bug-899):
 * - `claim` takes a short lease; `false` if a peer holds it or it is already done;
 * - `commit` turns it into a deduplication window, only after a success;
 * - `release` gives it back when the listener throws, so that the retry can run.
 */
export interface ProcessedGuard {
  claim(key: string): Promise<boolean>
  commit(key: string): Promise<void>
  release(key: string): Promise<void>
}

/** Lease of a delivery in progress, well above a listener's real duration. */
const LEASE_SEC = 5 * 60
/** Window during which a redelivery of the same event is ignored. */
const DEDUP_SEC = 24 * 60 * 60

/**
 * Default guard, on KeyDB. OPEN on failure: if KeyDB does not answer, we log it
 * and the listener runs anyway (a side effect is not lost silently).
 */
export const redisGuard: ProcessedGuard = {
  async claim(key) {
    try {
      return (await redis.set(key, 'in-flight', 'EX', LEASE_SEC, 'NX')) === 'OK'
    } catch (err) {
      console.error('[event] dedup guard unavailable, processing anyway:', err)
      return true
    }
  },
  async commit(key) {
    try {
      await redis.set(key, 'done', 'EX', DEDUP_SEC)
    } catch (err) {
      console.error('[event] dedup guard could not commit, delivery stands:', err)
    }
  },
  async release(key) {
    try {
      await redis.del(key)
    } catch (err) {
      console.error('[event] dedup guard could not release, lease will expire:', err)
    }
  },
}

/**
 * The idempotent listener of an event: `name` is the subscriber's identity and prefixes
 * the deduplication key, so that two subscribers of the same event each run once.
 */
export function makeListener<E extends DomainEvent>(
  name: string,
  handler: (event: E) => Promise<void> | void,
  guard: ProcessedGuard = redisGuard,
): (event: E) => Promise<void> {
  return async (event: E): Promise<void> => {
    const key = `evt:dedup:${name}:${event.eventId}`
    if (!(await guard.claim(key))) return
    try {
      await handler(event)
    } catch (err) {
      await guard.release(key)
      throw err
    }
    await guard.commit(key)
  }
}

/**
 * Subscribe a listener to an event class, on the core emitter (fast path) AND in the
 * outbox registry (redelivery), with the same listener on both sides.
 * The wiring between modules lives in `start/events.ts`, nowhere else.
 */
export function onDomainEvent<E extends DomainEvent>(
  event: new (...args: never[]) => E,
  name: string,
  handler: (event: E) => Promise<void> | void,
  guard?: ProcessedGuard,
): void {
  const listener = makeListener(name, handler, guard)
  emitter.on(event, listener)
  registerOutboxSubscriber(event.name, listener)
}

/** The subscribers by event type (class name), which the outbox relay replays. */
const outboxSubscribers = new Map<string, Array<(event: DomainEvent) => Promise<void>>>()

/** Register a subscriber for redelivery, without going through the emitter (testable on its own). */
export function registerOutboxSubscriber<E extends DomainEvent>(eventName: string, listener: (event: E) => Promise<void>): void {
  const list = outboxSubscribers.get(eventName) ?? []
  // The only variance we accept: a listener of `E` is only ever called with an `E`.
  list.push(listener as (event: DomainEvent) => Promise<void>)
  outboxSubscribers.set(eventName, list)
}

/** Time left to the fast path before the relay takes the row. */
const OUTBOX_GRACE_SEC = 30

/**
 * The outbox writes only while the relay runs: `bin/server.ts` turns it on, the test
 * harness never goes through there. Writing without a relay would only pile up dead rows.
 */
let outboxActive = false
export function setOutboxActive(active: boolean): void {
  outboxActive = active
}

/** Keep the event for the relay. OPEN: a failure is logged, emission carries on. */
async function enqueueForDurability(event: DomainEvent): Promise<void> {
  try {
    if (!outboxActive) return
    const db = getDb()
    if (!db) return
    await enqueueOutbox(db, {
      id: globalThis.crypto.randomUUID(),
      eventType: event.constructor.name,
      payload: JSON.parse(JSON.stringify(event)),
      delaySeconds: OUTBOX_GRACE_SEC,
    })
  } catch (err) {
    console.error('[outbox] enqueue failed; event delivered fast-path only:', err)
  }
}

/**
 * Redeliver a claimed row to ALL its subscribers (the guard skips those that succeeded).
 * `ok` if none threw; a type with no subscriber is `ok` so that its row gets cleaned up.
 */
export async function deliverOutboxRow(row: OutboxRow): Promise<{ ok: boolean; error?: string }> {
  const subs = outboxSubscribers.get(row.event_type) ?? []
  if (subs.length === 0) return { ok: true }
  const payload = (typeof row.payload === 'string' ? JSON.parse(row.payload) : row.payload) as DomainEvent
  let firstError: string | undefined
  for (const run of subs) {
    try {
      await run(payload)
    } catch (err) {
      firstError ??= err instanceof Error ? err.message : String(err)
    }
  }
  return firstError ? { ok: false, error: firstError } : { ok: true }
}
