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

/**
 * `identity.keyPackages.low`: a device's pool of single-use KeyPackages runs low, or is
 * empty. Emitted when a claim crosses the threshold, not on every claim: the realtime
 * channel (epic 04) forwards it to the device, which renews its pool.
 */
export class KeyPackagesLow extends DomainEvent {
  readonly type = 'identity.keyPackages.low'
  /** The device, hex. */
  readonly deviceId: string
  readonly stockLeft: number

  constructor(deviceId: string, stockLeft: number) {
    super()
    this.deviceId = deviceId
    this.stockLeft = stockLeft
  }
}
