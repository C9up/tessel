import relay from '@c9up/relay/services/main'
import { deviceChannel, type Ring } from '@tessel/protocol/realtime'
import type { Doorbell } from '../domain/ports.js'

/**
 * Rings each device's channel on `relay` (story 04-03). The ring carries the envelope's
 * identifier only: no body goes through the SSE streams, nor through the bus between
 * instances (story 04-05).
 */
export const relayDoorbell: Doorbell = {
  ring(devices, envelope) {
    const ring: Ring = { envelope }
    for (const device of devices) relay.broadcast(deviceChannel(device), ring)
  },
}
