import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import { Method } from '@tessel/protocol'
import { Mailbox } from '#modules/delivery/application/Mailbox.js'
import { deliveryMethods } from '#modules/delivery/http/deliveryMethods.js'
import { coreEnvelopes } from '#modules/delivery/infrastructure/coreEnvelopes.js'
import { SqlMailboxes } from '#modules/delivery/infrastructure/SqlMailboxes.js'
import { sessionGuard } from '#modules/identity/http/identityMethods.js'
import { identityDeps } from './identity.js'

/** The mailboxes (04-01), composed once, on first use. */
let composed: Promise<Mailbox> | null = null
function mailbox(): Promise<Mailbox> {
  composed ??= app.container.make<AsyncDatabaseConnection>('db').then((db) => new Mailbox(coreEnvelopes, new SqlMailboxes(db)))
  return composed
}

/** The delivery module's RPC points, behind the identity module's sessions. */
export function registerDeliveryMethods(rpc: RpcRouter): void {
  const points = deliveryMethods({ mailbox, guard: sessionGuard(identityDeps) })
  rpc.method(Method.send, points.send)
  rpc.method(Method.fetch, points.fetch)
}
