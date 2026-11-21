import { DateTime } from '@c9up/chronos'
import { accountName, IdentityError } from '../domain/account.js'
import { KeyPackagesLow } from '../domain/events.js'
import {
  type ClaimedKeyPackage,
  crossesLowStock,
  type KeyPackageUpload,
  LOW_STOCK,
  MAX_BATCH,
  MAX_STOCK,
  type StoredKeyPackage,
} from '../domain/keyPackages.js'
import type { Devices, KeyPackages, KeyPackageVerifier } from '../domain/ports.js'
import type { SignedInDevice } from '../domain/session.js'

const hex = (bytes: Uint8Array) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i])
}

/**
 * The KeyPackage directory (story 03-03): a signed-in device leaves its KeyPackages,
 * another signed-in device takes one per device of the account it wants to add to a
 * group.
 */
export class KeyPackageDirectory {
  readonly #verifier: KeyPackageVerifier
  readonly #devices: Devices
  readonly #keyPackages: KeyPackages
  readonly #domain: string

  constructor(verifier: KeyPackageVerifier, devices: Devices, keyPackages: KeyPackages, domain: string) {
    this.#verifier = verifier
    this.#devices = devices
    this.#keyPackages = keyPackages
    this.#domain = domain
  }

  /**
   * Keeps what the device uploads, all or nothing. Each KeyPackage must be valid, still
   * valid now, of the right kind, and carry the very certificate the server keeps for
   * this device: a device leaves KeyPackages for itself only. Answers the stock after.
   */
  async publish(device: SignedInDevice, upload: KeyPackageUpload): Promise<number> {
    if (upload.publish.length + (upload.lastResort === undefined ? 0 : 1) + upload.withdraw.length > MAX_BATCH) throw new IdentityError('invalidKeyPackage')
    const known = await this.#devices.find(device.deviceId)
    if (known === null || known.revoked) throw new IdentityError('unauthenticated')
    const now = DateTime.now().toSeconds()

    const check = (keyPackage: Uint8Array, lastResort: boolean): StoredKeyPackage => {
      const verified = this.#verifier.verify(keyPackage)
      if (verified === null || !sameBytes(verified.certificate, known.certificate) || verified.lastResort !== lastResort || verified.notAfter <= now)
        throw new IdentityError('invalidKeyPackage')
      return { reference: verified.reference, keyPackage, lastResort, notAfter: verified.notAfter }
    }
    const publish = upload.publish.map((keyPackage) => check(keyPackage, false))
    const lastResort = upload.lastResort === undefined ? undefined : check(upload.lastResort, true)

    if ((await this.#keyPackages.stock(device.deviceId)) + publish.length > MAX_STOCK) throw new IdentityError('stockFull')
    return this.#keyPackages.store(device.deviceId, { publish, lastResort, withdraw: upload.withdraw })
  }

  /** The device's stock of single-use KeyPackages, and whether it should renew its pool. */
  async stock(device: SignedInDevice): Promise<{ stock: number; low: boolean }> {
    const stock = await this.#keyPackages.stock(device.deviceId)
    return { stock, low: stock < LOW_STOCK }
  }

  /**
   * A KeyPackage for each device of `account` (one of this instance), the caller's own
   * device left out. The devices whose stock a claim runs low are told, once per crossing.
   */
  async claim(caller: SignedInDevice, account: string): Promise<ClaimedKeyPackage[]> {
    let name: string
    try {
      name = accountName(account, this.#domain)
    } catch (error) {
      // A name no account can have: no device to add. Another instance's stays an error.
      if (error instanceof IdentityError && error.code === 'invalidCertificate') return []
      throw error
    }
    const claimed = await this.#keyPackages.claim(name, caller.deviceId)
    for (const entry of claimed) if (crossesLowStock(entry)) await new KeyPackagesLow(hex(entry.deviceId), entry.stockLeft).emit()
    return claimed
  }
}
