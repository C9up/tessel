import { checkKeyPackage } from '@tessel/core-web/server'
import type { VerifiedKeyPackage } from '../domain/keyPackages.js'
import type { KeyPackageVerifier } from '../domain/ports.js'

/** The KeyPackage check of `tessel-core`, through its WASM module: the server writes no crypto of its own. */
export const coreKeyPackages: KeyPackageVerifier = {
  verify(keyPackage: Uint8Array): VerifiedKeyPackage | null {
    const checked = checkKeyPackage(keyPackage)
    if (checked === undefined) return null
    return {
      deviceId: checked.device.deviceId,
      certificate: checked.device.certificate,
      reference: checked.reference,
      lastResort: checked.lastResort,
      notAfter: checked.notAfter,
    }
  },
}
