/**
 * The KeyPackage directory (story 03-03). Each device leaves a pool of single-use
 * KeyPackages, plus one last-resort KeyPackage. Whoever adds the device to a group takes
 * one: a single-use one is handed out once and deleted; when none is left, the last-resort
 * one serves, as many times as needed, until the device renews its pool.
 *
 * The server checks what it is handed (signatures, suite, the device's certificate) but
 * cannot read it: a KeyPackage holds only public keys.
 */

/** A device keeps at most this many single-use KeyPackages here: its pool is 100. */
export const MAX_STOCK = 200

/** Below this many single-use KeyPackages left, the device is told to renew its pool. */
export const LOW_STOCK = 25

/** At most this many KeyPackages in one upload. */
export const MAX_BATCH = 128

/** A KeyPackage as the core checked it. */
export interface VerifiedKeyPackage {
  readonly deviceId: Uint8Array
  /** The device certificate inside it. */
  readonly certificate: Uint8Array
  readonly reference: Uint8Array
  readonly lastResort: boolean
  /** Unix seconds. */
  readonly notAfter: number
}

/** A KeyPackage to keep. */
export interface StoredKeyPackage {
  readonly reference: Uint8Array
  readonly keyPackage: Uint8Array
  readonly lastResort: boolean
  /** Unix seconds. */
  readonly notAfter: number
}

/** What a device uploads: `TesselCore.refreshKeyPackages`, as bytes. */
export interface KeyPackageUpload {
  readonly publish: readonly Uint8Array[]
  readonly lastResort?: Uint8Array
  /** References of KeyPackages to withdraw. */
  readonly withdraw: readonly Uint8Array[]
}

/** A KeyPackage handed out for one device of the account. */
export interface ClaimedKeyPackage {
  readonly deviceId: Uint8Array
  readonly keyPackage: Uint8Array
  /** Single-use KeyPackages the device has left; 0 when the last-resort one served. */
  readonly stockLeft: number
  readonly lastResort: boolean
}

/** Whether a claim that left `stockLeft` crosses a threshold the device must hear about. */
export function crossesLowStock(claimed: ClaimedKeyPackage): boolean {
  return !claimed.lastResort && (claimed.stockLeft === LOW_STOCK - 1 || claimed.stockLeft === 0)
}
