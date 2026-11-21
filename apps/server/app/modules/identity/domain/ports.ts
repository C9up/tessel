import type { CertifiedDevice, NewAccount } from './account.js'

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
