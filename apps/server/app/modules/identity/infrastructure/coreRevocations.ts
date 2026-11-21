import { checkRevocation } from '@tessel/core-web/server'
import type { RevocationVerifier } from '../domain/ports.js'

/** The revocation check of `tessel-core`, through its WASM module: the server writes no crypto of its own. */
export const coreRevocations: RevocationVerifier = {
  verify: (account, deviceId, accountKey, revocation) => checkRevocation(account, deviceId, accountKey, revocation),
}
