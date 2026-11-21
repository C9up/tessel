import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { Scheduler } from '@c9up/ream'
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import relay from '@c9up/relay/services/main'
import { Method } from '@tessel/protocol'
import delivery from '#config/delivery.js'
import { Mailbox } from '#modules/delivery/application/Mailbox.js'
import { deliveryMethods } from '#modules/delivery/http/deliveryMethods.js'
import { purgeTask } from '#modules/delivery/http/purge.js'
import { canListen, DEVICE_CHANNEL, streamGate } from '#modules/delivery/http/streams.js'
import { coreEnvelopes } from '#modules/delivery/infrastructure/coreEnvelopes.js'
import { relayDoorbell } from '#modules/delivery/infrastructure/relayDoorbell.js'
import { SqlMailboxes } from '#modules/delivery/infrastructure/SqlMailboxes.js'
import { sessionGuard } from '#modules/identity/http/identityMethods.js'
import { identityDeps } from './identity.js'

/** The mailboxes (04-01, 04-02) and their doorbell (04-03), composed once, on first use. */
let composed: Promise<Mailbox> | null = null
function mailbox(): Promise<Mailbox> {
  composed ??= app.container.make<AsyncDatabaseConnection>('db').then((db) => new Mailbox(coreEnvelopes, new SqlMailboxes(db), relayDoorbell))
  return composed
}

const guard = sessionGuard(identityDeps)

/** The delivery module's RPC points, behind the identity module's sessions. */
export function registerDeliveryMethods(rpc: RpcRouter): void {
  const points = deliveryMethods({ mailbox, guard })
  rpc.method(Method.send, points.send)
  rpc.method(Method.fetch, points.fetch)
  rpc.method(Method.ack, points.ack)
}

/** Each device's stream and channel (04-03), behind the same sessions. */
export function registerDeliveryStreams(): void {
  relay.authorize(DEVICE_CHANNEL, canListen)
  relay.registerRoutes((route) => route.use(streamGate(guard)))
}

/** The purge (04-04), on ream's scheduler: one instance at a time, every hour. */
export function registerDeliverySchedules(scheduler: Scheduler): void {
  scheduler.register('delivery.purge', delivery.purgeSchedule, purgeTask(mailbox, delivery.retention.as('days')))
}
