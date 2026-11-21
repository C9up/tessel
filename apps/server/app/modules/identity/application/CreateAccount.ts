import { accountName, IdentityError } from '../domain/account.js'
import { AccountCreated } from '../domain/events.js'
import type { Accounts, CertificateVerifier } from '../domain/ports.js'

/**
 * Creating an account (story 03-01): the device made its account key and its own key,
 * and sends the certificate the first signs for the second. The server checks the
 * signature, that the account is of this instance, and that the name is free; it asks
 * for nothing else.
 */
export class CreateAccount {
  readonly #verifier: CertificateVerifier
  readonly #accounts: Accounts
  readonly #domain: string

  constructor(verifier: CertificateVerifier, accounts: Accounts, domain: string) {
    this.#verifier = verifier
    this.#accounts = accounts
    this.#domain = domain
  }

  /** The account `name@domain`, created; `IdentityError` otherwise. */
  async run(certificate: Uint8Array): Promise<string> {
    const device = this.#verifier.verify(certificate)
    if (device === null) throw new IdentityError('invalidCertificate')
    const name = accountName(device.account, this.#domain)
    const created = await this.#accounts.create({ name, device })
    if (created === null) throw new IdentityError('nameTaken')
    await new AccountCreated(created.id).emit()
    return device.account
  }
}
