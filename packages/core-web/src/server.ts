/**
 * What the server takes from the core (story 03-01): checking a device certificate's
 * signature, in Node, with the same WASM module as the browser. The server never holds
 * a private key; it only verifies.
 *
 * Self-contained on purpose: the server type-checks this file without the browser's
 * types (no DOM, no IndexedDB) that the rest of the package needs.
 */

import { readFileSync } from 'node:fs'
import { Core, generateStoreKey, initSync, selfCheck, verifyCertificate as verify } from '../generated/tessel_wasm.js'

/** A device, as its verified certificate names it. */
export interface CertifiedDevice {
  /** `name@domain`. */
  account: string
  deviceId: Uint8Array<ArrayBuffer>
  accountKey: Uint8Array<ArrayBuffer>
  deviceKey: Uint8Array<ArrayBuffer>
  certificate: Uint8Array<ArrayBuffer>
}

let loaded = false

/** Loads the WASM module from this package's files, once. */
export function loadServerCore(): void {
  if (loaded) return
  initSync({ module: readFileSync(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url)) })
  selfCheck()
  loaded = true
}

/** The device a certificate names, once its account key's signature is checked; `undefined` otherwise. */
export function verifyCertificate(certificate: Uint8Array): CertifiedDevice | undefined {
  loadServerCore()
  try {
    return verify(certificate)
  } catch {
    // The core refuses: it does not decode, or its signature does not verify.
    return undefined
  }
}

/**
 * For tests only: the certificate of a throwaway account's first device, as an app
 * would send it. Its keys are dropped at once.
 */
export function throwawayCertificate(account: string): Uint8Array<ArrayBuffer> {
  loadServerCore()
  const key = generateStoreKey()
  const core = Core.createAccount(account, key)
  key.fill(0)
  try {
    const device: CertifiedDevice = core.device()
    return device.certificate
  } finally {
    core.free()
  }
}
