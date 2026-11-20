import assert from 'node:assert/strict'
import { test } from 'node:test'
import { type OutboundMessage, type Outbox, OutboxSender, type Relay } from './outbox.ts'
import type { Bytes } from './store.ts'

const bytes = (text: string): Bytes => Uint8Array.from(new TextEncoder().encode(text))
const name = (value: Bytes) => new TextDecoder().decode(value)

/** The core's queues, in memory: the head comes back until acknowledged. */
class FakeOutbox implements Outbox {
  readonly queues = new Map<string, OutboundMessage[]>()
  #count = 0

  write(group: string, text: string): void {
    const queue = this.queues.get(group) ?? []
    queue.push({ group: bytes(group), sendId: bytes(`send-${this.#count++}`), messageId: bytes(text), encrypted: bytes(text) })
    this.queues.set(group, queue)
  }

  async groups(): Promise<Bytes[]> {
    return [...this.queues.keys()].map(bytes)
  }

  async next(group: Bytes): Promise<OutboundMessage | undefined> {
    return this.queues.get(name(group))?.[0]
  }

  async sent(message: OutboundMessage): Promise<void> {
    const key = name(message.group)
    const queue = (this.queues.get(key) ?? []).filter((m) => name(m.sendId) !== name(message.sendId))
    if (queue.length === 0) this.queues.delete(key)
    else this.queues.set(key, queue)
  }
}

/** The server: stores what it receives, unless it is unreachable. */
class FakeRelay implements Relay {
  readonly received: string[] = []
  down = new Set<string>()
  offline = false

  async send(message: OutboundMessage): Promise<void> {
    if (this.offline || this.down.has(name(message.group))) throw new Error('unreachable')
    this.received.push(`${name(message.group)}:${name(message.encrypted)}`)
  }
}

/** Timers fired by hand. */
class FakeTimers {
  readonly scheduled: { callback: () => void; delay: number }[] = []
  readonly setTimer = (callback: () => void, delay: number) => {
    const timer = { callback, delay }
    this.scheduled.push(timer)
    return timer
  }
  readonly clearTimer = (handle: unknown) => {
    const index = this.scheduled.findIndex((timer) => timer === handle)
    if (index >= 0) this.scheduled.splice(index, 1)
  }

  fire(): void {
    const timer = this.scheduled.shift()
    assert.ok(timer, 'a retry is scheduled')
    timer.callback()
  }
}

function setup() {
  const outbox = new FakeOutbox()
  const relay = new FakeRelay()
  const timers = new FakeTimers()
  const network = new EventTarget()
  const sender = new OutboxSender(outbox, relay, { connectivity: network, setTimer: timers.setTimer, clearTimer: timers.clearTimer })
  return { outbox, relay, timers, network, sender }
}

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve))

test('what is queued leaves at start, in order per group', async () => {
  const { outbox, relay, sender } = setup()
  outbox.write('family', 'one')
  outbox.write('work', 'meeting')
  outbox.write('family', 'two')
  outbox.write('family', 'three')
  sender.start()
  await sender.flush()
  assert.deepEqual(
    relay.received.filter((m) => m.startsWith('family:')),
    ['family:one', 'family:two', 'family:three'],
  )
  assert.ok(relay.received.includes('work:meeting'))
  assert.equal(outbox.queues.size, 0)
  sender.stop()
})

test('offline, retries come with growing delays, then everything leaves once', async () => {
  const { outbox, relay, timers, sender } = setup()
  relay.offline = true
  outbox.write('family', 'written in the tunnel')
  outbox.write('family', 'still in the tunnel')
  await sender.flush()
  timers.fire()
  await sender.flush()
  timers.fire()
  await sender.flush()
  assert.deepEqual(
    timers.scheduled.map((t) => t.delay),
    [5_000],
  )
  assert.deepEqual(relay.received, [])

  relay.offline = false
  timers.fire()
  await settle()
  await sender.flush()
  assert.deepEqual(relay.received, ['family:written in the tunnel', 'family:still in the tunnel'])
  assert.equal(timers.scheduled.length, 0)
})

test('the network coming back sends at once and cancels the pending retry', async () => {
  const { outbox, relay, timers, network, sender } = setup()
  sender.start()
  relay.offline = true
  outbox.write('family', 'back soon')
  await sender.flush()
  assert.equal(timers.scheduled.length, 1)

  relay.offline = false
  network.dispatchEvent(new Event('online'))
  await settle()
  await sender.flush()
  assert.deepEqual(relay.received, ['family:back soon'])
  assert.equal(timers.scheduled.length, 0)
  sender.stop()
})

test('a stuck group does not hold the others back', async () => {
  const { outbox, relay, timers, sender } = setup()
  relay.down.add('stuck')
  outbox.write('stuck', 'waiting')
  outbox.write('fine', 'hello')
  outbox.write('fine', 'again')
  await sender.flush()
  assert.deepEqual(relay.received, ['fine:hello', 'fine:again'])
  assert.deepEqual([...outbox.queues.keys()], ['stuck'])
  assert.equal(timers.scheduled.length, 1)
})

test('a message written during a flush leaves too', async () => {
  const { outbox, relay, sender } = setup()
  outbox.write('family', 'first')
  const running = sender.flush()
  outbox.write('family', 'written meanwhile')
  outbox.write('friends', 'new group meanwhile')
  await Promise.all([running, sender.flush()])
  assert.deepEqual(relay.received.sort(), ['family:first', 'family:written meanwhile', 'friends:new group meanwhile'])
  assert.equal(outbox.queues.size, 0)
})
