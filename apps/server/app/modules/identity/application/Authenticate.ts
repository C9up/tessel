import { IdentityError } from '../domain/account.js'
import type { Devices, Sessions } from '../domain/ports.js'
import type { SignedInDevice } from '../domain/session.js'

/**
 * The device behind a session token, on every call that needs one (story 03-02). The
 * device is read again each time: once it is revoked, its sessions are refused even
 * before they are deleted.
 */
export class Authenticate {
  readonly #sessions: Sessions
  readonly #devices: Devices
  readonly #domain: string

  constructor(sessions: Sessions, devices: Devices, domain: string) {
    this.#sessions = sessions
    this.#devices = devices
    this.#domain = domain
  }

  /** The signed-in device; `IdentityError('unauthenticated')` otherwise. */
  async run(token: string): Promise<SignedInDevice> {
    const session = token === '' ? null : await this.#sessions.find(token)
    const device = session === null ? null : await this.#devices.find(session.deviceId)
    if (device === null || device.revoked) throw new IdentityError('unauthenticated')
    return { accountId: device.accountId, account: `${device.name}@${this.#domain}`, deviceId: device.deviceId }
  }

  /** Closes the session behind `token`; `IdentityError('unauthenticated')` when there is none. */
  async signOut(token: string): Promise<void> {
    await this.run(token)
    if (!(await this.#sessions.close(token))) throw new IdentityError('unauthenticated')
  }
}
