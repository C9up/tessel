/**
 * An account (story 03-01): `name@domain`, its account key, and its first device, as
 * the device's certificate names them (ADR-002). The server knows the name and the
 * public keys, nothing else: no e-mail, no number, no display name.
 */

/** ADR-002: 3 to 32 of `a-z 0-9 . _ -`, a letter first. No Unicode: no homoglyph. */
const NAME = /^[a-z][a-z0-9._-]{2,31}$/

/** A device, as its certificate names it once the account key's signature is checked. */
export interface CertifiedDevice {
  /** `name@domain`. */
  readonly account: string
  readonly deviceId: Uint8Array
  readonly accountKey: Uint8Array
  readonly deviceKey: Uint8Array
  /** The certificate itself, to hand to the account's contacts (story 03-03). */
  readonly certificate: Uint8Array
}

/** An account to create: its name on this instance, and its first device. */
export interface NewAccount {
  readonly name: string
  readonly device: CertifiedDevice
}

export type IdentityErrorCode = 'invalidCertificate' | 'wrongDomain' | 'nameTaken' | 'signInRefused' | 'unauthenticated'

export class IdentityError extends Error {
  readonly code: IdentityErrorCode

  constructor(code: IdentityErrorCode) {
    super(code)
    this.name = 'IdentityError'
    this.code = code
  }
}

/** The name of an account of `domain`; the refusal otherwise. */
export function accountName(account: string, domain: string): string {
  const at = account.lastIndexOf('@')
  const name = account.slice(0, at)
  if (at < 0 || !NAME.test(name)) throw new IdentityError('invalidCertificate')
  if (account.slice(at + 1) !== domain) throw new IdentityError('wrongDomain')
  return name
}
