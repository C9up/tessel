import type { RpcRouter } from '@c9up/ream/rpc/router'
import { Method, type Pong, PROTOCOL_VERSION } from '@tessel/protocol'

/** The RPC points of the `system` module, registered by `start/rpc.ts`. */
export function registerSystemMethods(rpc: RpcRouter): void {
  // Public: a client checks that the server answers and speaks its protocol.
  rpc.method(Method.ping, async (): Promise<Pong> => ({ pong: true, protocol: PROTOCOL_VERSION }))
}
