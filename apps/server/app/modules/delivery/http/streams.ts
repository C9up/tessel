import type { HttpContext } from '@c9up/ream'
import type { CallingDevice } from './deliveryMethods.js'

/** The channel pattern of a device's rings: `devices/<id hex>`. */
export const DEVICE_CHANNEL = 'devices/:id'

const hex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')

/**
 * The relay routes' gate (story 04-03): the stream, and each subscription, carry the
 * device's session token (`Authorization: Bearer`), as every RPC call does. A valid one
 * makes the device the request's user, which is what relay checks a stream's owner
 * against; anything else is turned away before relay sees the request.
 */
export function streamGate(guard: (ctx: HttpContext) => Promise<CallingDevice>) {
  return async (ctx: HttpContext, next: () => Promise<void>): Promise<void> => {
    let device: CallingDevice
    try {
      device = await guard(ctx)
    } catch {
      ctx.response.status(401).json({ error: { code: 'E_UNAUTHENTICATED', message: 'E_UNAUTHENTICATED' } })
      return
    }
    ctx.auth = { isAuthenticated: true, user: { id: hex(device.deviceId) } }
    await next()
  }
}

/** A device hears its own channel, and no other. */
export function canListen(ctx: { auth?: { user?: { id: string } } }, params: Record<string, string>): boolean {
  const own = ctx.auth?.user?.id
  return own !== undefined && own === params.id
}
