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

/**
 * `identity.device.added`: a device was linked to an account by QR code (story 03-04).
 * Opaque identifiers only: the account's, and the device's (hex).
 */
export class DeviceAdded extends DomainEvent {
  readonly type = 'identity.device.added'
  readonly accountId: string
  readonly deviceId: string

  constructor(accountId: string, deviceId: string) {
    super()
    this.accountId = accountId
    this.deviceId = deviceId
  }
}

/**
 * `identity.device.revoked`: a device was revoked (story 03-05); its sessions and its
 * KeyPackages are gone. Opaque identifiers only.
 */
export class DeviceRevoked extends DomainEvent {
  readonly type = 'identity.device.revoked'
  readonly accountId: string
  readonly deviceId: string

  constructor(accountId: string, deviceId: string) {
    super()
    this.accountId = accountId
    this.deviceId = deviceId
  }
}

/**
 * `identity.profile.updated`: an account sealed a new profile (story 03-06). Its devices
 * tell their contacts in their groups (MLS); this event names the account's opaque
 * identifier only.
 */
export class ProfileUpdated extends DomainEvent {
  readonly type = 'identity.profile.updated'
  readonly accountId: string

  constructor(accountId: string) {
    super()
    this.accountId = accountId
  }
}

/**
 * `identity.account.deleted`: an account and everything the server kept of it are gone
 * (story 03-07). The modules that keep something of an account (backups, pending media:
 * epics 05 and 07) listen to it and erase theirs. Its opaque identifier only.
 */
export class AccountDeleted extends DomainEvent {
  readonly type = 'identity.account.deleted'
  readonly accountId: string

  constructor(accountId: string) {
    super()
    this.accountId = accountId
  }
}
