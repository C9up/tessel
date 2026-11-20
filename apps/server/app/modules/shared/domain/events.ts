import { DateTime } from '@c9up/chronos'

/**
 * The base of every domain event (nadeo model): a stable `eventId`, the
 * idempotency key, and the instant it occurred. Pure: `emit()` hands it to the
 * publisher that the infrastructure plugs in at startup
 * (`shared/infrastructure/events.ts`), which keeps it in the outbox before the
 * listeners hear it.
 *
 * An event NEVER carries message content: the server has none.
 * What it does carry is entered in the metadata register (story 15-02).
 */
export abstract class DomainEvent {
  /** Stable identifier of the instance: the deduplication key. */
  readonly eventId: string = globalThis.crypto.randomUUID()
  /** ISO-8601 instant at which the event was constructed. */
  readonly occurredAt: string = DateTime.now().toISO()

  /** Published: kept durably first, then delivered to the process's listeners. */
  emit(): Promise<void> {
    return publish(this)
  }
}

let publish: (event: DomainEvent) => Promise<void> = async () => {}

/** Plugged in once at startup by the infrastructure, which owns the outbox and the emitter. */
export function setEventPublisher(fn: (event: DomainEvent) => Promise<void>): void {
  publish = fn
}
