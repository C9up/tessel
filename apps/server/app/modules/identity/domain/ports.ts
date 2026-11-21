import type { CertifiedDevice, NewAccount } from './account.js'
import type { KnownDevice } from './session.js'

/** Checks a device certificate's signature: the core does, never this server's code. */
export interface CertificateVerifier {
  /** The device it names; `null` when it does not decode or its signature does not verify. */
  verify(certificate: Uint8Array): CertifiedDevice | null
}

/** The accounts and their devices. */
export interface Accounts {
  /** Keeps the account and its first device; `null` when the name or the key is already taken. */
  create(account: NewAccount): Promise<{ id: string } | null>
}

/** Checks a sign-in proof: the core does, with the key the certificate certifies. */
export interface SignInVerifier {
  verify(certificate: Uint8Array, challenge: Uint8Array, proof: Uint8Array): boolean
}

/** Sign-in challenges: random, short-lived, good for one attempt. */
export interface Challenges {
  issue(): Promise<Uint8Array>
  /** Whether `challenge` was issued and is still fresh; it is spent either way. */
  take(challenge: Uint8Array): Promise<boolean>
}

/** The devices the server knows. */
export interface Devices {
  find(deviceId: Uint8Array): Promise<KnownDevice | null>
  /** Marks the device revoked and deletes its sessions; whether it was known and live. */
  revoke(deviceId: Uint8Array): Promise<boolean>
}

/** The sessions, each bound to one device. */
export interface Sessions {
  /** Opens a session for the device; its token, to hand to the device only. */
  open(deviceId: Uint8Array): Promise<string>
  /** The device of a valid (known, unexpired) session token; `null` otherwise. */
  find(token: string): Promise<{ deviceId: Uint8Array } | null>
  /** Closes the session behind `token`; whether there was one. */
  close(token: string): Promise<boolean>
}
