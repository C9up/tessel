import { IdentityError } from '../domain/account.js'
import { DeviceRevoked } from '../domain/events.js'
import type { Devices, RevocationVerifier } from '../domain/ports.js'
import type { DeviceEntry, SignedInDevice } from '../domain/session.js'

const hex = (bytes: Uint8Array) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i])
}

/**
 * An account's devices (story 03-05): the list, and revocation. A device revokes itself
 * on its session alone; revoking another one takes the account key's signature, so a
 * stolen secondary device cannot lock the person out of the others.
 */
export class AccountDevices {
  readonly #devices: Devices
  readonly #verifier: RevocationVerifier

  constructor(devices: Devices, verifier: RevocationVerifier) {
    this.#devices = devices
    this.#verifier = verifier
  }

  /** The caller's account's live devices. */
  list(caller: SignedInDevice): Promise<DeviceEntry[]> {
    return this.#devices.list(caller.accountId)
  }

  /**
   * Revokes `deviceId`, a live device of the caller's account: its sessions and its
   * KeyPackages go, it signs in no more. Removing it from the groups is the devices' job
   * (MLS commits), not the server's.
   */
  async revoke(caller: SignedInDevice, deviceId: Uint8Array, revocation: Uint8Array | undefined): Promise<void> {
    const target = await this.#devices.find(deviceId)
    if (target === null || target.revoked || target.accountId !== caller.accountId) throw new IdentityError('unknownDevice')
    if (!sameBytes(deviceId, caller.deviceId)) {
      if (revocation === undefined || !this.#verifier.verify(caller.account, deviceId, target.accountKey, revocation))
        throw new IdentityError('invalidRevocation')
    }
    if (!(await this.#devices.revoke(deviceId))) throw new IdentityError('unknownDevice')
    await new DeviceRevoked(caller.accountId, hex(deviceId)).emit()
  }
}
