import { DomainEvent } from '#modules/shared/domain/events.js'

/**
 * `identity.account.created`: an account exists. It names only the account's opaque
 * identifier, never its name or its keys (ADR-006).
 */
export class AccountCreated extends DomainEvent {
  readonly type = 'identity.account.created'
  readonly accountId: string

  constructor(accountId: string) {
    super()
    this.accountId = accountId
  }
}
