import { checkSignIn } from '@tessel/core-web/server'
import type { SignInVerifier } from '../domain/ports.js'

/** The sign-in check of `tessel-core`, through its WASM module: the server writes no crypto of its own. */
export const coreSignIns: SignInVerifier = {
  verify: (certificate, challenge, proof) => checkSignIn(certificate, challenge, proof),
}
