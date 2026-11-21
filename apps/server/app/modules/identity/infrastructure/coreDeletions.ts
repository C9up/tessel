import { checkAccountDeletion } from '@tessel/core-web/server'
import type { DeletionVerifier } from '../domain/ports.js'

/** The account deletion check of `tessel-core`, through its WASM module: the server writes no crypto of its own. */
export const coreDeletions: DeletionVerifier = {
  verify: (account, accountKey, deletion) => checkAccountDeletion(account, accountKey, deletion),
}
