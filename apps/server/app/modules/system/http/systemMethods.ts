import type { RpcRouter } from '@c9up/ream/rpc/router'
import { Method, type Pong, PROTOCOL_VERSION } from '@tessel/protocol'

/** Les points RPC du module `system`, enregistrés par `start/rpc.ts`. */
export function registerSystemMethods(rpc: RpcRouter): void {
  // Public : un client vérifie que le serveur répond et parle son protocole.
  rpc.method(Method.ping, async (): Promise<Pong> => ({ pong: true, protocol: PROTOCOL_VERSION }))
}
