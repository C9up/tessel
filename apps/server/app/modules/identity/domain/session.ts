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
  /** Its account's key, as the server keeps it: what a revocation is checked against. */
  readonly accountKey: Uint8Array
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

/** What a sign-in carries: the device, the challenge it answers, its signature of it, its kind. */
export interface SignInAttempt {
  readonly deviceId: Uint8Array
  readonly challenge: Uint8Array
  readonly proof: Uint8Array
  readonly kind: DeviceKind
}

/** What a device says it is when it signs in. */
export const DEVICE_KINDS = ['web', 'android', 'ios'] as const
export type DeviceKind = (typeof DEVICE_KINDS)[number]

export const isDeviceKind = (value: unknown): value is DeviceKind => DEVICE_KINDS.some((kind) => kind === value)

/** A live device of the account, as its list shows it (story 03-05). */
export interface DeviceEntry {
  readonly deviceId: Uint8Array
  /** `null` until it signs in once. */
  readonly kind: DeviceKind | null
  /** `YYYY-MM-DD`. */
  readonly createdOn: string
  /** `YYYY-MM-DD`; `null` until it signs in once. */
  readonly lastActiveOn: string | null
}
