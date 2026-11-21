/**
 * Signing in with the device key (story 03-02): the server draws a challenge, the
 * device signs it, the server checks the signature with the key the device's certificate
 * certifies, and opens a session bound to that device. No password: nothing to steal
 * from the server, nothing to phish from the person.
 */

/** A device, as the server keeps it. */
export interface KnownDevice {
  readonly deviceId: Uint8Array
  readonly accountId: string
  /** The name of `name@domain`. */
  readonly name: string
  /** What its sign-in proofs are checked against. */
  readonly certificate: Uint8Array
  /** A revoked device signs in no more, and its sessions are gone. */
  readonly revoked: boolean
}

/** The device behind a valid session. */
export interface SignedInDevice {
  readonly accountId: string
  /** `name@domain`. */
  readonly account: string
  readonly deviceId: Uint8Array
}

/** What a sign-in carries: the device, the challenge it answers, its signature of it. */
export interface SignInAttempt {
  readonly deviceId: Uint8Array
  readonly challenge: Uint8Array
  readonly proof: Uint8Array
}
