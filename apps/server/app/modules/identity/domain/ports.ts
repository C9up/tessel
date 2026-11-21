import type { CertifiedDevice, NewAccount } from './account.js'
import type { ClaimedKeyPackage, StoredKeyPackage, VerifiedKeyPackage } from './keyPackages.js'
import type { DeviceEntry, DeviceKind, KnownDevice } from './session.js'

/** Checks a device certificate's signature: the core does, never this server's code. */
export interface CertificateVerifier {
  /** The device it names; `null` when it does not decode or its signature does not verify. */
  verify(certificate: Uint8Array): CertifiedDevice | null
}

/** The accounts and their devices. */
export interface Accounts {
  /** Keeps the account and its first device; `null` when the name or the key is already taken. */
  create(account: NewAccount): Promise<{ id: string } | null>
  /**
   * Deletes the account and all that hangs from it, and reserves its name for
   * `reservedDays`. Whether it was there.
   */
  delete(accountId: string, name: string, reservedDays: number): Promise<boolean>
}

/** Checks an account deletion: the core does, with the account key the server keeps. */
export interface DeletionVerifier {
  verify(account: string, accountKey: Uint8Array, deletion: Uint8Array): boolean
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
  /** How many live (unrevoked) devices the account has. */
  live(accountId: string): Promise<number>
  /**
   * Registers a device of the account: only if the certificate's account key is the
   * account's, and its identifier and key are new. Whether it was added.
   */
  add(accountId: string, device: CertifiedDevice): Promise<boolean>
  /** Records a sign-in: the kind the device declares, and today as its last active day. */
  signedIn(deviceId: Uint8Array, kind: DeviceKind): Promise<void>
  /** The account's live devices, oldest first. */
  list(accountId: string): Promise<DeviceEntry[]>
}

/** The sealed profiles: one blob per account. */
export interface Profiles {
  put(accountId: string, blob: Uint8Array): Promise<void>
  /** The blob of the account named `name`; `null` when it has none. */
  get(name: string): Promise<Uint8Array | null>
}

/** Checks a device revocation: the core does, with the account key the server keeps. */
export interface RevocationVerifier {
  verify(account: string, deviceId: Uint8Array, accountKey: Uint8Array, revocation: Uint8Array): boolean
}

/** The mailboxes of device links: one sealed grant per link, for a few minutes. */
export interface LinkMailbox {
  /** Leaves the grant; `false` when the link already holds one. */
  post(link: Uint8Array, grant: Uint8Array): Promise<boolean>
  /** Takes the grant out; `null` when there is none. */
  take(link: Uint8Array): Promise<Uint8Array | null>
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

/** Checks a KeyPackage: the core does (signatures, suite, device certificate). */
export interface KeyPackageVerifier {
  /** `null` when it is not a valid KeyPackage of Tessel's suite. */
  verify(keyPackage: Uint8Array): VerifiedKeyPackage | null
}

/** The KeyPackage directory. */
export interface KeyPackages {
  /**
   * In one transaction: withdraws `withdraw`, replaces the last-resort KeyPackage when a
   * new one comes, adds `publish`. Answers the single-use stock after.
   */
  store(deviceId: Uint8Array, upload: { publish: StoredKeyPackage[]; lastResort?: StoredKeyPackage; withdraw: readonly Uint8Array[] }): Promise<number>
  /** The single-use KeyPackages the device has left, still valid. */
  stock(deviceId: Uint8Array): Promise<number>
  /**
   * One KeyPackage for each live device of account `name` but `except`: a single-use one,
   * deleted as it is handed out (never twice, even to concurrent claims), else the
   * last-resort one. A device with neither is left out.
   */
  claim(name: string, except: Uint8Array): Promise<ClaimedKeyPackage[]>
}
