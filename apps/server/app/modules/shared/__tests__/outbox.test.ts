import { describe, expect, test } from '@c9up/helix'
import { DomainEvent } from '#modules/shared/domain/events.js'
import { deliverOutboxRow, makeListener, type ProcessedGuard, registerOutboxSubscriber } from '#modules/shared/infrastructure/events.js'
import type { OutboxDb, OutboxEntry, OutboxRow } from '#modules/shared/infrastructure/outbox.js'
import { OutboxRelay } from '#modules/shared/infrastructure/outboxRelay.js'

/** A sample event, specific to this test. */
class SampleHappened extends DomainEvent {
  constructor(readonly note: string) {
    super()
  }
}

/** An in-memory outbox: the same operations as the SQL functions, without a database. */
function memoryOutbox() {
  const rows = new Map<string, OutboxRow & { due: boolean; lastError?: string }>()
  const db = {} as OutboxDb
  return {
    rows,
    db,
    enqueue(entry: OutboxEntry) {
      rows.set(entry.id, {
        id: entry.id,
        event_type: entry.eventType,
        payload: JSON.stringify(entry.payload),
        attempts: 0,
        due: true,
      })
    },
    deps: {
      db: async () => db,
      claim: async () => [...rows.values()].filter((r) => r.due).map((r) => ({ ...r })),
      del: async (_db: OutboxDb, id: string) => {
        rows.delete(id)
      },
      reschedule: async (_db: OutboxDb, id: string, opts: { delaySeconds: number; error: string }) => {
        const row = rows.get(id)
        if (row)
          Object.assign(row, {
            attempts: row.attempts + 1,
            due: false,
            lastError: opts.error,
          })
      },
      deliver: deliverOutboxRow,
    },
  }
}

/** An in-memory guard, in place of KeyDB. */
function memoryGuard(): ProcessedGuard {
  const keys = new Map<string, string>()
  return {
    async claim(key) {
      if (keys.has(key)) return false
      keys.set(key, 'in-flight')
      return true
    },
    async commit(key) {
      keys.set(key, 'done')
    },
    async release(key) {
      keys.delete(key)
    },
  }
}

describe('an event goes through the outbox', () => {
  test('the relay delivers a kept event to its subscriber, then forgets it', async () => {
    const outbox = memoryOutbox()
    const heard: SampleHappened[] = []
    registerOutboxSubscriber(
      'SampleHappened',
      makeListener<SampleHappened>('test.sample', (e) => void heard.push(e), memoryGuard()),
    )
    const event = new SampleHappened('hello')
    outbox.enqueue({
      id: crypto.randomUUID(),
      eventType: event.constructor.name,
      payload: JSON.parse(JSON.stringify(event)),
      delaySeconds: 0,
    })

    expect(await new OutboxRelay(outbox.deps).tick()).toBe(1)
    expect(heard.map((e) => [e.eventId, e.note])).toEqual([[event.eventId, 'hello']])
    expect(outbox.rows.size).toBe(0)
  })

  test('a subscriber that throws leaves the row to be retried', async () => {
    const outbox = memoryOutbox()
    registerOutboxSubscriber(
      'FailingHappened',
      makeListener(
        'test.failing',
        () => {
          throw new Error('boom')
        },
        memoryGuard(),
      ),
    )
    const id = crypto.randomUUID()
    outbox.enqueue({
      id,
      eventType: 'FailingHappened',
      payload: {},
      delaySeconds: 0,
    })

    await new OutboxRelay(outbox.deps).tick()
    expect(outbox.rows.get(id)?.attempts).toBe(1)
    expect(outbox.rows.get(id)?.lastError).toBe('boom')
  })

  test('an event type nobody listens to is cleaned up', async () => {
    expect(
      await deliverOutboxRow({
        id: crypto.randomUUID(),
        event_type: 'NobodyCares',
        payload: {},
        attempts: 0,
      }),
    ).toEqual({ ok: true })
  })
})
