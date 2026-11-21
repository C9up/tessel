import { accountName, IdentityError } from '../domain/account.js'
import { ProfileUpdated } from '../domain/events.js'
import type { Profiles } from '../domain/ports.js'
import { isProfileBlob } from '../domain/profile.js'
import type { SignedInDevice } from '../domain/session.js'

/**
 * Encrypted profiles (story 03-06): a device of the account leaves the sealed blob,
 * contacts fetch it and open it with the key the account shared with them over MLS.
 */
export class AccountProfiles {
  readonly #profiles: Profiles
  readonly #domain: string

  constructor(profiles: Profiles, domain: string) {
    this.#profiles = profiles
    this.#domain = domain
  }

  /** Keeps the caller's account's new sealed profile. */
  async set(caller: SignedInDevice, blob: Uint8Array): Promise<void> {
    if (!isProfileBlob(blob)) throw new IdentityError('invalidProfile')
    await this.#profiles.put(caller.accountId, blob)
    await new ProfileUpdated(caller.accountId).emit()
  }

  /** The sealed profile of `account` (of this instance); `null` when it has none. */
  async get(account: string): Promise<Uint8Array | null> {
    let name: string
    try {
      name = accountName(account, this.#domain)
    } catch (error) {
      // A name no account can have has no profile. Another instance's stays an error.
      if (error instanceof IdentityError && error.code === 'invalidCertificate') return null
      throw error
    }
    return this.#profiles.get(name)
  }
}
