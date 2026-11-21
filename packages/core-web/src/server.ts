/**
 * What the server takes from the core (story 03-01): checking a device certificate's
 * signature, in Node, with the same WASM module as the browser. The server never holds
 * a private key; it only verifies.
 *
 * Self-contained on purpose: the server type-checks this file without the browser's
 * types (no DOM, no IndexedDB) that the rest of the package needs.
 */

import { readFileSync } from 'node:fs'
import { Core, generateStoreKey, initSync, selfCheck, verifyCertificate as verify, verifySignIn } from '../generated/tessel_wasm.js'

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
 * Whether `proof` is the signature of `challenge` by the device `certificate` certifies
 * (story 03-02). The certificate is the one the server keeps for the device.
 */
export function checkSignIn(certificate: Uint8Array, challenge: Uint8Array, proof: Uint8Array): boolean {
  loadServerCore()
  try {
    verifySignIn(certificate, challenge, proof)
    return true
  } catch {
    return false
  }
}

/** For tests only: a throwaway account's first device, as an app holds it. */
export interface ThrowawayDevice {
  readonly certificate: Uint8Array<ArrayBuffer>
  readonly deviceId: Uint8Array<ArrayBuffer>
  /** The proof `TesselCore.signIn` makes. */
  signIn(challenge: Uint8Array): Uint8Array
  /** Drops its keys. */
  free(): void
}

/** For tests only: a throwaway account's first device. Its keys live until `free`. */
export function throwawayDevice(account: string): ThrowawayDevice {
  loadServerCore()
  const key = generateStoreKey()
  const core = Core.createAccount(account, key)
  key.fill(0)
  const device: CertifiedDevice = core.device()
  return {
    certificate: device.certificate,
    deviceId: device.deviceId,
    signIn: (challenge) => core.signIn(challenge),
    free: () => core.free(),
  }
}

/**
 * For tests only: the certificate of a throwaway account's first device, as an app
 * would send it. Its keys are dropped at once.
 */
export function throwawayCertificate(account: string): Uint8Array<ArrayBuffer> {
  const device = throwawayDevice(account)
  device.free()
  return device.certificate
}
