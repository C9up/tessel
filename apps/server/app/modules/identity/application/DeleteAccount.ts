import { IdentityError } from '../domain/account.js'
import { AccountDeleted } from '../domain/events.js'
import type { Accounts, DeletionVerifier, Devices } from '../domain/ports.js'
import type { SignedInDevice } from '../domain/session.js'

/** Days a deleted account's name stays reserved, so that nobody takes it to pass for it. */
export const NAME_RESERVATION_DAYS = 30

/**
 * Deleting an account (story 03-07). It takes the account key's signature, like revoking
 * another device: a stolen secondary device cannot delete the account. The devices leave
 * their groups first (MLS); the server then erases everything it kept, and keeps only
 * the name, reserved for a while.
 */
export class DeleteAccount {
  readonly #devices: Devices
  readonly #accounts: Accounts
  readonly #verifier: DeletionVerifier

  constructor(devices: Devices, accounts: Accounts, verifier: DeletionVerifier) {
    this.#devices = devices
    this.#accounts = accounts
    this.#verifier = verifier
  }

  async run(caller: SignedInDevice, deletion: Uint8Array): Promise<void> {
    const device = await this.#devices.find(caller.deviceId)
    if (device === null || device.revoked) throw new IdentityError('unauthenticated')
    if (!this.#verifier.verify(caller.account, device.accountKey, deletion)) throw new IdentityError('invalidDeletion')
    if (!(await this.#accounts.delete(caller.accountId, device.name, NAME_RESERVATION_DAYS))) throw new IdentityError('unauthenticated')
    await new AccountDeleted(caller.accountId).emit()
  }
}
