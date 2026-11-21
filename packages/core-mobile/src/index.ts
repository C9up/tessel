/**
 * tessel-core on the phone (story 02-13): the same `TesselCore` as on the web, over the
 * native module `NativeTessel` (JSI, then the C ABI of tessel-mobile, ../rust). Every
 * call goes through `RemoteCore`, the very class the web page uses to reach its
 * worker: the two apps share one contract by construction, not by copy.
 *
 * Arguments and answers travel in MessagePack, which keeps bytes as bytes; the store
 * key never crosses (the core seals it with the Android Keystore itself).
 */

// First: msgpack builds a TextDecoder when it loads, which Hermes lacks.
import './textDecoder.ts'
import { decode, encode } from '@msgpack/msgpack'
import type { TesselCore } from '@tessel/core-web/api'
import { type Delivered, type Endpoint, RemoteCore } from '@tessel/core-web/rpc'

/** The native module (specs/NativeTessel.ts): arguments in, an ArrayBuffer out. */
export interface NativeTesselModule {
  call(method: string, args: Uint8Array): unknown
}

type Listener = (event: Delivered) => void

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null

/**
 * What the core's answer means in JavaScript: MessagePack's nil is `undefined` (an
 * absent optional), byte strings are `Uint8Array`s already.
 */
function plain(value: unknown): unknown {
  if (value === null) return undefined
  if (value instanceof Uint8Array) return value
  if (Array.isArray(value)) return value.map(plain)
  if (isRecord(value)) {
    const out: Record<string, unknown> = {}
    for (const [key, field] of Object.entries(value)) {
      const converted = plain(field)
      if (converted !== undefined) out[key] = converted
    }
    return out
  }
  return value
}

/** A native error, "<code>\n<message>", as the code and message of a TesselError. */
function failure(error: unknown): { code: string; message: string } {
  const text = error instanceof Error ? error.message : String(error)
  const newline = text.indexOf('\n')
  return newline < 0 ? { code: 'host:native', message: text } : { code: text.slice(0, newline), message: text.slice(newline + 1) }
}

/** The native module, seen as the far end of a message channel: what `RemoteCore` speaks to. */
export class NativeEndpoint implements Endpoint {
  readonly #module: NativeTesselModule
  readonly #listeners = new Set<Listener>()

  constructor(module: NativeTesselModule) {
    this.#module = module
  }

  postMessage(message: unknown): void {
    if (!isRecord(message) || typeof message.id !== 'number' || typeof message.method !== 'string' || !Array.isArray(message.args)) return
    const { id, method, args } = message
    let answer: unknown
    try {
      const bytes = this.#module.call(method, encode(args))
      answer = { id, ok: true, value: plain(decode(new Uint8Array(bytes instanceof ArrayBuffer ? bytes : new ArrayBuffer(0)))) }
    } catch (error) {
      answer = { id, ok: false, ...failure(error) }
    }
    // Answered on a later turn, as a worker would: the caller has registered its promise.
    queueMicrotask(() => {
      for (const listener of this.#listeners) listener({ data: answer })
    })
  }

  addEventListener(_type: 'message', listener: Listener): void {
    this.#listeners.add(listener)
  }

  removeEventListener(_type: 'message', listener: Listener): void {
    this.#listeners.delete(listener)
  }
}

/** The core on this phone. */
export function mobileCore(module: NativeTesselModule): TesselCore {
  return new RemoteCore(new NativeEndpoint(module))
}
