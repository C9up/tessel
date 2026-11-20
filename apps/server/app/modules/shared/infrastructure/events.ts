import { getDb } from '@c9up/atlas/services/db'
import emitter from '@c9up/ream/events/services/main'
import { type DomainEvent, setEventPublisher } from '#modules/shared/domain/events.js'
import { enqueueOutbox, type OutboxRow } from '#modules/shared/infrastructure/outbox.js'
import { redis } from '#modules/shared/infrastructure/redis.js'

// Tout événement du domaine est publié ici : le `emit()` du domaine le remet.
setEventPublisher(async (event) => {
  await enqueueForDurability(event)
  await emitter.dispatchEvent(event)
})

/**
 * Garde « au plus une fois » par `(abonné, événement)`, en trois temps pour qu'une
 * livraison ÉCHOUÉE reste rejouable (repris de nadeo, bug-899) :
 * - `claim` prend un bail court ; `false` si un pair le tient ou si c'est déjà fait ;
 * - `commit` le change en fenêtre de déduplication, seulement après un succès ;
 * - `release` le rend quand l'écouteur lève, pour que la reprise puisse tourner.
 */
export interface ProcessedGuard {
  claim(key: string): Promise<boolean>
  commit(key: string): Promise<void>
  release(key: string): Promise<void>
}

/** Bail d'une livraison en cours, bien au-dessus de la durée réelle d'un écouteur. */
const LEASE_SEC = 5 * 60
/** Fenêtre pendant laquelle une redélivrance du même événement est ignorée. */
const DEDUP_SEC = 24 * 60 * 60

/**
 * Garde par défaut, sur KeyDB. OUVERTE en cas de panne : si KeyDB ne répond pas, on
 * journalise et l'écouteur tourne quand même (un effet de bord ne se perd pas en silence).
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
 * L'écouteur idempotent d'un événement : `name` est l'identité de l'abonné et préfixe la
 * clé de déduplication, pour que deux abonnés d'un même événement tournent chacun une fois.
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
 * Abonner un écouteur à une classe d'événement, sur l'émetteur du cœur (chemin rapide)
 * ET au registre de l'outbox (redélivrance), avec le même écouteur des deux côtés.
 * Le câblage entre modules vit dans `start/events.ts`, nulle part ailleurs.
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

/** Les abonnés par type d'événement (nom de classe), que le relais de l'outbox rejoue. */
const outboxSubscribers = new Map<string, Array<(event: DomainEvent) => Promise<void>>>()

/** Inscrire un abonné à la redélivrance, sans passer par l'émetteur (testable seul). */
export function registerOutboxSubscriber<E extends DomainEvent>(eventName: string, listener: (event: E) => Promise<void>): void {
  const list = outboxSubscribers.get(eventName) ?? []
  // Seule variance assumée : un écouteur de `E` n'est jamais appelé qu'avec un `E`.
  list.push(listener as (event: DomainEvent) => Promise<void>)
  outboxSubscribers.set(eventName, list)
}

/** Délai laissé au chemin rapide avant que le relais ne prenne la ligne. */
const OUTBOX_GRACE_SEC = 30

/**
 * L'outbox n'écrit que quand le relais tourne : `bin/server.ts` l'active, le harnais de
 * test ne passe jamais par là. Écrire sans relais n'empilerait que des lignes mortes.
 */
let outboxActive = false
export function setOutboxActive(active: boolean): void {
  outboxActive = active
}

/** Garder l'événement pour le relais. OUVERT : un échec est journalisé, l'émission continue. */
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
 * Redélivrer une ligne réservée à TOUS ses abonnés (le garde saute ceux qui ont abouti).
 * `ok` si aucun n'a levé ; un type sans abonné est `ok` pour que sa ligne soit nettoyée.
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
