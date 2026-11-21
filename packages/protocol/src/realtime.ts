/**
 * Real-time delivery (story 04-03), the client side shared by web and mobile.
 *
 * The server rings a device's channel (`devices/<id>`, over `relay`'s SSE stream) when
 * an envelope arrives for it; the ring carries the envelope's identifier, never its
 * body. The device then fetches what came after its cursor (`delivery.fetch`): the
 * mailbox stays the one source of truth, so nothing is lost when a ring is (a cut, a
 * reconnection), nothing is handed twice, and envelopes come in the server's order.
 */

import type { ReceivedEnvelope, TesselClient } from './index.js'

/** The channel a device listens on: its identifier, hex. */
export function deviceChannel(device: Uint8Array): string {
  return `devices/${Array.from(device, (byte) => byte.toString(16).padStart(2, '0')).join('')}`
}

/** What a ring carries: the identifier of the envelope that arrived. */
export interface Ring {
  readonly envelope: string
}

export function parseRing(data: unknown): Ring | undefined {
  return typeof data === 'object' && data !== null && 'envelope' in data && typeof data.envelope === 'string' && /^\d+$/.test(data.envelope)
    ? { envelope: data.envelope }
    : undefined
}

/**
 * A device's inbox: it hands each envelope to `deliver` once, in order, and moves its
 * cursor only once `deliver` resolved. The app persists the cursor (`cursor`) and
 * gives it back on the next start.
 */
export class Inbox {
  readonly #client: Pick<TesselClient, 'fetch'>
  readonly #deliver: (envelope: ReceivedEnvelope) => Promise<void>
  #cursor: string | undefined
  #draining: Promise<void> | null = null
  #again = false

  constructor(client: Pick<TesselClient, 'fetch'>, deliver: (envelope: ReceivedEnvelope) => Promise<void>, cursor?: string) {
    this.#client = client
    this.#deliver = deliver
    this.#cursor = cursor
  }

  /** The last envelope handed to `deliver`. */
  get cursor(): string | undefined {
    return this.#cursor
  }

  /**
   * Fetches and delivers everything after the cursor. One drain at a time: a call
   * while one runs makes it go round once more, so a ring is never missed.
   */
  drain(): Promise<void> {
    if (this.#draining !== null) {
      this.#again = true
      return this.#draining
    }
    this.#draining = this.#run().finally(() => {
      this.#draining = null
    })
    return this.#draining
  }

  /** A ring: the envelope is fetched with whatever else came, unless already delivered. */
  ring(ring: Ring): Promise<void> {
    if (this.#cursor !== undefined && BigInt(ring.envelope) <= BigInt(this.#cursor)) return Promise.resolve()
    return this.drain()
  }

  async #run(): Promise<void> {
    do {
      this.#again = false
      for (;;) {
        const { envelopes } = await this.#client.fetch(this.#cursor)
        for (const envelope of envelopes) {
          await this.#deliver(envelope)
          this.#cursor = envelope.id
        }
        if (envelopes.length === 0) break
      }
    } while (this.#again)
  }
}

/** One Server-Sent Event. */
export interface SseEvent {
  readonly event: string
  readonly data: string
}

/** The events of an SSE body, as they arrive (comments and keep-alives included). */
export async function* readSse(body: ReadableStream<Uint8Array>): AsyncGenerator<SseEvent> {
  const decoder = new TextDecoder()
  let buffer = ''
  let event = 'message'
  let data: string[] = []
  for await (const chunk of body) {
    buffer += decoder.decode(chunk, { stream: true })
    let end = buffer.indexOf('\n')
    while (end >= 0) {
      const line = buffer.slice(0, end).replace(/\r$/, '')
      buffer = buffer.slice(end + 1)
      if (line === '') {
        if (data.length > 0) yield { event, data: data.join('\n') }
        event = 'message'
        data = []
      } else if (line.startsWith('event:')) {
        event = line.slice(6).trim()
      } else if (line.startsWith('data:')) {
        data.push(line.slice(5).replace(/^ /, ''))
      }
      end = buffer.indexOf('\n')
    }
  }
}

/** The stream identifier of relay's `connected` event. */
function uidOf(data: unknown): string | undefined {
  return typeof data === 'object' && data !== null && 'uid' in data && typeof data.uid === 'string' ? data.uid : undefined
}

/** What {@link listen} needs. */
export interface ListenOptions {
  /** The server, as `fetchTransport` has it. */
  readonly baseUrl: string
  /** The session token (story 03-02). */
  readonly token: string
  /** This device's identifier. */
  readonly device: Uint8Array
  readonly inbox: Inbox
  readonly fetch?: typeof fetch
  readonly signal?: AbortSignal
}

/**
 * Opens the device's stream, subscribes to its channel, catches up from the inbox's
 * cursor, then fetches on each ring. Resolves when the stream ends: the app calls it
 * again, and the inbox resumes from its cursor. (A `fetch` stream rather than
 * `EventSource`: it carries the session in `Authorization`.)
 */
export async function listen(options: ListenOptions): Promise<void> {
  const fetchImpl = options.fetch ?? fetch
  const authorization = `Bearer ${options.token}`
  const response = await fetchImpl(new URL('/__relay/events', options.baseUrl), {
    headers: { authorization, accept: 'text/event-stream' },
    signal: options.signal,
  })
  if (!response.ok || response.body === null) throw new Error(`E_REALTIME_REFUSED: ${response.status}`)
  const channel = deviceChannel(options.device)
  for await (const frame of readSse(response.body)) {
    if (frame.event === 'connected') {
      const uid = uidOf(JSON.parse(frame.data))
      if (uid === undefined) throw new Error('E_REALTIME_REFUSED: no stream identifier')
      const subscribed = await fetchImpl(new URL('/__relay/subscribe', options.baseUrl), {
        method: 'POST',
        headers: { authorization, 'content-type': 'application/json' },
        body: JSON.stringify({ uid, channel }),
        signal: options.signal,
      })
      if (!subscribed.ok) throw new Error(`E_REALTIME_REFUSED: ${subscribed.status}`)
      // Whatever came while the device was away, before the first ring.
      await options.inbox.drain()
    } else if (frame.event === channel) {
      const ring = parseRing(JSON.parse(frame.data))
      if (ring !== undefined) await options.inbox.ring(ring)
    }
  }
}
