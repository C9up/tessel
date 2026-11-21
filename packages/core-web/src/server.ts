/**
 * What the server takes from the core (story 03-01): checking a device certificate's
 * signature, in Node, with the same WASM module as the browser. The server never holds
 * a private key; it only verifies.
 *
 * Self-contained on purpose: the server type-checks this file without the browser's
 * types (no DOM, no IndexedDB) that the rest of the package needs.
 */

import { readFileSync } from 'node:fs'
import {
  Core,
  generateStoreKey,
  initSync,
  PendingLink,
  selfCheck,
  verifyCertificate as verify,
  verifyKeyPackage as verifyPackage,
  verifyRevocation,
  verifySignIn,
} from '../generated/tessel_wasm.js'

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

/**
 * Whether `revocation` is the signature, by `accountKey` (the one the server keeps for
 * the account), revoking `deviceId` of `account` (story 03-05).
 */
export function checkRevocation(account: string, deviceId: Uint8Array, accountKey: Uint8Array, revocation: Uint8Array): boolean {
  loadServerCore()
  try {
    verifyRevocation(account, deviceId, accountKey, revocation)
    return true
  } catch {
    return false
  }
}

/** A *KeyPackage*, as its signatures, suite and device certificate check out (story 03-03). */
export interface CheckedKeyPackage {
  /** The device that made it. */
  device: CertifiedDevice
  /** Its `KeyPackageRef`. */
  reference: Uint8Array<ArrayBuffer>
  lastResort: boolean
  /** The end of its validity, in Unix seconds. */
  notAfter: number
}

/** The *KeyPackage* checked by the core; `undefined` when it is not a valid one. */
export function checkKeyPackage(keyPackage: Uint8Array): CheckedKeyPackage | undefined {
  loadServerCore()
  try {
    return verifyPackage(keyPackage)
  } catch {
    return undefined
  }
}

/** What a device sends the directory after renewing its pool: `TesselCore.refreshKeyPackages`. */
export interface KeyPackageBatch {
  publish: Uint8Array<ArrayBuffer>[]
  lastResort?: Uint8Array<ArrayBuffer>
  withdraw: Uint8Array<ArrayBuffer>[]
}

/** For tests only: a throwaway account's first device, as an app holds it. */
export interface ThrowawayDevice {
  readonly certificate: Uint8Array<ArrayBuffer>
  readonly deviceId: Uint8Array<ArrayBuffer>
  /** The proof `TesselCore.signIn` makes. */
  signIn(challenge: Uint8Array): Uint8Array
  /** Its *KeyPackage* pool renewed at `now` (Unix seconds), as `TesselCore.refreshKeyPackages` gives it. */
  refreshKeyPackages(now: number): KeyPackageBatch
  /** `TesselCore.grantLink`: on an account's first device, it certifies a scanned device. */
  grantLink(qr: Uint8Array): { link: Uint8Array<ArrayBuffer>; certificate: Uint8Array<ArrayBuffer>; grant: Uint8Array<ArrayBuffer> }
  /** `TesselCore.revokeDevice`: on an account's first device, it revokes another one. */
  revokeDevice(deviceId: Uint8Array): Uint8Array
  /** Drops its keys. */
  free(): void
}

function throwaway(core: Core): ThrowawayDevice {
  const device: CertifiedDevice = core.device()
  return {
    certificate: device.certificate,
    deviceId: device.deviceId,
    signIn: (challenge) => core.signIn(challenge),
    refreshKeyPackages: (now) => core.refreshKeyPackages(now),
    grantLink: (qr) => core.grantLink(qr),
    revokeDevice: (deviceId) => core.revokeDevice(deviceId),
    free: () => core.free(),
  }
}

/** For tests only: a throwaway account's first device. Its keys live until `free`. */
export function throwawayDevice(account: string): ThrowawayDevice {
  loadServerCore()
  const key = generateStoreKey()
  const core = Core.createAccount(account, key)
  key.fill(0)
  return throwaway(core)
}

/** For tests only: a new device showing its link QR code (`TesselCore.requestLink`). */
export interface ThrowawayLink {
  readonly qr: Uint8Array
  readonly link: Uint8Array
  /** `TesselCore.completeLink`: the linked device. */
  accept(grant: Uint8Array): ThrowawayDevice
}

/** For tests only: a new device waiting to be linked. */
export function throwawayLink(): ThrowawayLink {
  loadServerCore()
  const pending = new PendingLink()
  return {
    qr: pending.qr(),
    link: pending.link(),
    accept: (grant) => {
      const key = generateStoreKey()
      try {
        return throwaway(pending.accept(grant, key))
      } finally {
        key.fill(0)
        pending.free()
      }
    },
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
