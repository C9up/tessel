import { ThrottleException } from '@c9up/blackhole'
import limiter from '@c9up/blackhole/services/limiter'
import type { HttpContext } from '@c9up/ream'
import { type Acknowledged, type EnvelopeSent, type Envelopes, fromBase64Url, toBase64Url } from '@tessel/protocol'
import { RpcFailure } from '#modules/shared/http/rpc.js'
import type { Mailbox } from '../application/Mailbox.js'
import { DeliveryError, ENVELOPE_LIMITS, MAX_ENVELOPE, MAX_RECIPIENTS, type OutgoingEnvelope } from '../domain/envelope.js'

type Handler = (ctx: HttpContext, params: unknown) => Promise<unknown>

/**
 * Envelopes one device posts, per minute (story 04-06): five a second, sustained, is
 * more than a person types, and enough for a device catching up on its queue.
 */
export const SENDS = { requests: 300, duration: '1 minute' } as const

/** Envelopes one account posts, per minute, all its devices together. */
export const ACCOUNT_SENDS = { requests: 600, duration: '1 minute' } as const

/** Fetches (and acknowledgements) by one device, per minute. */
export const FETCHES = { requests: 600, duration: '1 minute' } as const

/** The calling device, as the identity module's session says (story 03-02). */
export interface CallingDevice {
  readonly deviceId: Uint8Array
  /** Its account's opaque identifier: what the per-account limits count under. */
  readonly accountId: string
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')

/** The bytes of a base64url string of at most `max` bytes; `undefined` otherwise. */
function decoded(value: unknown, max: number): Uint8Array | undefined {
  if (typeof value !== 'string' || value.length > Math.ceil((max * 4) / 3)) return undefined
  return fromBase64Url(value)
}

/** The envelope of `delivery.send`, decoded; `undefined` when it is not one. */
function outgoing(params: unknown): OutgoingEnvelope | undefined {
  if (!isRecord(params) || !Array.isArray(params.recipients) || params.recipients.length > MAX_RECIPIENTS) return undefined
  // Too large is refused by name, before anything is decoded.
  if (typeof params.envelope === 'string' && params.envelope.length > Math.ceil((MAX_ENVELOPE * 4) / 3)) throw new DeliveryError('tooLarge')
  const sendId = decoded(params.sendId, 16)
  const body = decoded(params.envelope, MAX_ENVELOPE)
  const recipients: Uint8Array[] = []
  for (const entry of params.recipients) {
    const device = decoded(entry, 16)
    if (device?.length !== 16) return undefined
    recipients.push(device)
  }
  if (sendId?.length !== 16 || body === undefined) return undefined
  return { sendId, body, recipients }
}

function refused(error: unknown): never {
  if (error instanceof DeliveryError) {
    if (error.code === 'noRecipient') throw new RpcFailure('E_DELIVERY_NO_RECIPIENT')
    if (error.code === 'staleEpoch') throw new RpcFailure('E_DELIVERY_STALE_EPOCH')
    if (error.code === 'tooLarge') throw new RpcFailure('E_DELIVERY_TOO_LARGE', { limits: ENVELOPE_LIMITS })
    throw new RpcFailure('E_DELIVERY_INVALID_ENVELOPE')
  }
  if (error instanceof ThrottleException) throw new RpcFailure('E_TOO_MANY_REQUESTS', { retryIn: error.response.availableIn })
  throw error
}

/** The delivery module's points: both need a session. */
export function deliveryMethods(deps: {
  mailbox: () => Promise<Mailbox>
  guard: (ctx: HttpContext) => Promise<CallingDevice>
}): Record<'send' | 'fetch' | 'ack', Handler> {
  return {
    send: async (ctx, params) => {
      const device = await deps.guard(ctx)
      try {
        // Per device, and per account: an account cannot get around the limit by
        // linking devices.
        await limiter.use(SENDS).consume(`delivery_send_${hex(device.deviceId)}`)
        await limiter.use(ACCOUNT_SENDS).consume(`delivery_account_send_${device.accountId}`)
        const envelope = outgoing(params)
        if (envelope === undefined) throw new RpcFailure('E_INVALID_PARAMS')
        const placed = await (await deps.mailbox()).send(device.deviceId, envelope)
        return (placed.seq === null ? { envelope: placed.id } : { envelope: placed.id, seq: placed.seq }) satisfies EnvelopeSent
      } catch (error) {
        refused(error)
      }
    },

    fetch: async (ctx, params) => {
      const device = await deps.guard(ctx)
      try {
        await limiter.use(FETCHES).consume(`delivery_fetch_${hex(device.deviceId)}`)
        const after = isRecord(params) ? params.after : undefined
        if (after !== undefined && (typeof after !== 'string' || !/^\d{1,19}$/.test(after))) throw new RpcFailure('E_INVALID_PARAMS')
        const envelopes = await (await deps.mailbox()).fetch(device.deviceId, after ?? null)
        return {
          envelopes: envelopes.map((envelope) =>
            envelope.seq === null
              ? { id: envelope.id, envelope: toBase64Url(envelope.body) }
              : { id: envelope.id, seq: envelope.seq, envelope: toBase64Url(envelope.body) },
          ),
        } satisfies Envelopes
      } catch (error) {
        refused(error)
      }
    },

    ack: async (ctx, params) => {
      const device = await deps.guard(ctx)
      try {
        // As often as fetches: the inbox acknowledges after each batch it fetched.
        await limiter.use(FETCHES).consume(`delivery_ack_${hex(device.deviceId)}`)
        const through = isRecord(params) ? params.through : undefined
        if (typeof through !== 'string' || !/^\d{1,19}$/.test(through)) throw new RpcFailure('E_INVALID_PARAMS')
        return { acknowledged: await (await deps.mailbox()).ack(device.deviceId, through) } satisfies Acknowledged
      } catch (error) {
        refused(error)
      }
    },
  }
}
