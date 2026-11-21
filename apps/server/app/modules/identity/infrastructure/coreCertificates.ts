import { verifyCertificate } from '@tessel/core-web/server'
import type { CertifiedDevice } from '../domain/account.js'
import type { CertificateVerifier } from '../domain/ports.js'

/** The signature check of `tessel-core`, through its WASM module: the server writes no crypto of its own. */
export const coreCertificates: CertificateVerifier = {
  verify(certificate: Uint8Array): CertifiedDevice | null {
    return verifyCertificate(certificate) ?? null
  },
}
