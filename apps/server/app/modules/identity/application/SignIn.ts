import { IdentityError } from '../domain/account.js'
import type { Challenges, Devices, Sessions, SignInVerifier } from '../domain/ports.js'
import type { SignInAttempt } from '../domain/session.js'

/**
 * Signing in (story 03-02). The challenge is spent first, whatever comes next: a proof
 * is tried once. Every refusal is the same one, so a caller learns nothing about which
 * devices exist.
 */
export class SignIn {
  readonly #challenges: Challenges
  readonly #devices: Devices
  readonly #verifier: SignInVerifier
  readonly #sessions: Sessions

  constructor(challenges: Challenges, devices: Devices, verifier: SignInVerifier, sessions: Sessions) {
    this.#challenges = challenges
    this.#devices = devices
    this.#verifier = verifier
    this.#sessions = sessions
  }

  /** A fresh challenge for a device to sign. */
  challenge(): Promise<Uint8Array> {
    return this.#challenges.issue()
  }

  /** The new session's token; `IdentityError('signInRefused')` otherwise. */
  async run(attempt: SignInAttempt): Promise<string> {
    if (!(await this.#challenges.take(attempt.challenge))) throw new IdentityError('signInRefused')
    const device = await this.#devices.find(attempt.deviceId)
    if (device === null || device.revoked) throw new IdentityError('signInRefused')
    if (!this.#verifier.verify(device.certificate, attempt.challenge, attempt.proof)) throw new IdentityError('signInRefused')
    return this.#sessions.open(device.deviceId)
  }
}
