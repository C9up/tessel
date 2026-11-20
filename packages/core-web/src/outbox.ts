/**
 * Sends the core's queue (story 02-07). Every message is queued by the core, online or
 * not (`Client::queue_message`); this sender empties the queue whenever it can: at
 * start, when the app writes, when the browser says the network is back, and after a
 * failure, with growing delays.
 *
 * Order holds per group: a group's next message is only asked for once the server has
 * acknowledged the previous one. Groups are independent: one stuck group does not hold
 * the others back. Nothing here decides anything about the content: the core encrypts,
 * the relay carries bytes.
 */

import type { Bytes } from './store.ts'

/** The head of a group's queue, encrypted by the core. */
export interface OutboundMessage {
  group: Bytes
  /** What the server deduplicates on: a retry is stored once. */
  sendId: Bytes
  messageId: Bytes
  encrypted: Bytes
}

/** The core's send queue, as the web binding exposes it (story 02-10). */
export interface Outbox {
  /** The groups whose queue holds messages. */
  groups(): Promise<Bytes[]>
  /** The head of the group's queue, encrypted and persisted; `undefined` when empty. */
  next(group: Bytes): Promise<OutboundMessage | undefined>
  /** The server acknowledged the message: it leaves the queue (persisted). */
  sent(message: OutboundMessage): Promise<void>
}

/** The way to the server (story 04-03). Resolves once the server acknowledged. */
export interface Relay {
  send(message: OutboundMessage): Promise<void>
}

/** Delays between retries after a failure, in milliseconds; the last one repeats. */
export const RETRY_DELAYS: readonly number[] = [1_000, 2_000, 5_000, 15_000, 30_000, 60_000]

export interface OutboxSenderOptions {
  /** Where `online` events come from: the window or the worker's global scope. */
  connectivity?: EventTarget
  retryDelays?: readonly number[]
  setTimer?: (callback: () => void, delay: number) => unknown
  clearTimer?: (handle: unknown) => void
}

function isEventTarget(value: unknown): value is EventTarget {
  return typeof value === 'object' && value !== null && 'addEventListener' in value && typeof value.addEventListener === 'function'
}

export class OutboxSender {
  readonly #outbox: Outbox
  readonly #relay: Relay
  readonly #connectivity: EventTarget | undefined
  readonly #retryDelays: readonly number[]
  readonly #setTimer: (callback: () => void, delay: number) => unknown
  readonly #clearTimer: (handle: unknown) => void
  #running: Promise<void> | undefined
  #rerun: Promise<void> | undefined
  #failures = 0
  #timer: unknown
  readonly #onOnline = () => {
    this.#failures = 0
    this.#cancelRetry()
    void this.flush()
  }

  constructor(outbox: Outbox, relay: Relay, options: OutboxSenderOptions = {}) {
    this.#outbox = outbox
    this.#relay = relay
    this.#connectivity = options.connectivity ?? (isEventTarget(globalThis) ? globalThis : undefined)
    this.#retryDelays = options.retryDelays ?? RETRY_DELAYS
    this.#setTimer = options.setTimer ?? ((callback, delay) => setTimeout(callback, delay))
    this.#clearTimer = options.clearTimer ?? ((handle) => clearTimeout(handle as ReturnType<typeof setTimeout>))
  }

  /** Listens for the network coming back, and sends what is already queued. */
  start(): void {
    this.#connectivity?.addEventListener('online', this.#onOnline)
    void this.flush()
  }

  stop(): void {
    this.#connectivity?.removeEventListener('online', this.#onOnline)
    this.#cancelRetry()
  }

  /**
   * Sends everything queued. Resolves once the queues are empty, or once a failure has
   * scheduled a retry; never rejects. Called while a flush runs, it runs once more
   * afterwards, so a message queued meanwhile is not left behind.
   */
  flush(): Promise<void> {
    if (this.#running === undefined) {
      this.#running = this.#drain().finally(() => {
        this.#running = undefined
      })
      return this.#running
    }
    this.#rerun ??= this.#running.then(() => {
      this.#rerun = undefined
      return this.flush()
    })
    return this.#rerun
  }

  async #drain(): Promise<void> {
    let failed: boolean
    try {
      const groups = await this.#outbox.groups()
      const results = await Promise.allSettled(groups.map((group) => this.#drainGroup(group)))
      failed = results.some((result) => result.status === 'rejected')
    } catch {
      failed = true
    }
    if (failed) {
      this.#scheduleRetry()
    } else {
      this.#failures = 0
    }
  }

  async #drainGroup(group: Bytes): Promise<void> {
    for (;;) {
      const message = await this.#outbox.next(group)
      if (message === undefined) return
      await this.#relay.send(message)
      await this.#outbox.sent(message)
    }
  }

  #scheduleRetry(): void {
    if (this.#timer !== undefined) return
    const delay = this.#retryDelays[Math.min(this.#failures, this.#retryDelays.length - 1)] ?? 0
    this.#failures += 1
    this.#timer = this.#setTimer(() => {
      this.#timer = undefined
      void this.flush()
    }, delay)
  }

  #cancelRetry(): void {
    if (this.#timer === undefined) return
    this.#clearTimer(this.#timer)
    this.#timer = undefined
  }
}
