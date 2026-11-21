/**
 * Conversations (epic 05), shared by web and mobile: what an app does with the core
 * (encryption, the MLS state, the local history) and the server (the directory, the
 * mailboxes). Nothing here encrypts or decides who may read: the core does. This module
 * only orders the calls: claim the KeyPackages, make the group, hand the Welcomes, send
 * what the core queued, hand what arrives back to it.
 */

import { fromBase64Url, type ReceivedEnvelope, ServerError, type TesselClient } from '@tessel/protocol'

type MaybePromise<T> = T | Promise<T>
type Bytes = Uint8Array

/** A device, as the core describes one: its certificate names its account and its account key. */
export interface CoreDevice {
  readonly account: string
  readonly deviceId: Bytes
  readonly certificate: Bytes
}

/** The safety number between this account and a contact's (story 02-09): the same on both screens. */
export interface SafetyNumber {
  /** 60 digits: {@link safetyGroups} cuts them for the screen. */
  readonly digits: string
  /** What this device shows as a QR, for the contact to scan. */
  readonly qr: Bytes
}

/**
 * What this device's verification says about a contact (story 05-03): never verified,
 * verified, verified but with a device added since, or verified under another account
 * key than the one it shows now (the safety number changed: verify again).
 */
export type Trust = 'unverified' | 'verified' | 'new-device' | 'key-changed'

/**
 * What the user must be told about a verified contact, as it happens: its safety number
 * changed, it has new devices, or some of its verified devices left the group.
 */
export type Alert =
  | { readonly type: 'key-changed'; readonly group: Bytes; readonly account: string }
  | { readonly type: 'new-device'; readonly group: Bytes; readonly account: string; readonly devices: readonly Bytes[] }
  | { readonly type: 'device-removed'; readonly group: Bytes; readonly account: string; readonly devices: readonly Bytes[] }

/** A safety number in twelve groups of five digits, as the screen shows it. */
export function safetyGroups(digits: string): string[] {
  return digits.match(/\d{1,5}/g) ?? []
}

/** A commit the core prepared, and the Welcome for the devices it adds. */
export interface CoreCommit {
  readonly commit: Bytes
  readonly welcome?: Bytes
}

/** A group's settings (story 05-02), as the core holds them: encrypted, in the group. */
export interface GroupSettings {
  readonly name: string
  readonly admins: readonly string[]
  readonly restricted: boolean
}

/** A change to a group's settings. */
export type GroupChange = { type: 'rename'; name: string } | { type: 'admin'; account: string; admin: boolean } | { type: 'restrict'; restricted: boolean }

/** How many devices one commit adds at most: its KeyPackages stay well within the server's limit (ADR-008). */
export const INVITE_BATCH = 100

/**
 * What the messenger asks of the core: the web binding's `TesselCore` (story 02-10)
 * answers it, and so does the mobile binding (story 02-13).
 */
export interface MessengerCore {
  device(): MaybePromise<CoreDevice>
  verifyKeyPackage(keyPackage: Bytes): MaybePromise<{ readonly device: CoreDevice }>
  createGroup(): MaybePromise<Bytes>
  addDevices(group: Bytes, keyPackages: Bytes[]): MaybePromise<CoreCommit>
  removeDevices(group: Bytes, removals: { account: string; deviceId?: Bytes }[]): MaybePromise<CoreCommit>
  configureGroup(group: Bytes, change: GroupChange): MaybePromise<CoreCommit>
  groupSettings(group: Bytes): MaybePromise<GroupSettings>
  commitPendingProposals(group: Bytes): MaybePromise<CoreCommit>
  commitAccepted(group: Bytes): MaybePromise<void>
  commitRejected(group: Bytes): MaybePromise<void>
  retryCommit(group: Bytes): MaybePromise<CoreCommit | undefined>
  leaveGroup(group: Bytes): MaybePromise<Bytes>
  joinGroup(welcome: Bytes): MaybePromise<Bytes>
  members(group: Bytes): MaybePromise<readonly CoreDevice[]>
  process(message: Bytes): MaybePromise<unknown>
  recipients(group: Bytes): MaybePromise<Bytes[]>
  queueMessage(group: Bytes, body: { type: 'text'; text: string }, sentAt: number): MaybePromise<unknown>
  nextOutbound(group: Bytes): MaybePromise<{ readonly sendId: Bytes; readonly encrypted: Bytes } | undefined>
  outboundSent(group: Bytes, sendId: Bytes): MaybePromise<void>
  conversations(): MaybePromise<readonly { readonly group: Bytes; readonly name: string; readonly lastSentAt?: number; readonly unread: number }[]>
  markRead(group: Bytes): MaybePromise<void>
  queueReceipts(sentAt: number): MaybePromise<Bytes[]>
  readReceipts(): MaybePromise<boolean>
  setReadReceipts(on: boolean): MaybePromise<void>
  safetyNumber(contact: Bytes): MaybePromise<SafetyNumber>
  checkScanned(contact: Bytes, scanned: Bytes): MaybePromise<void>
  markVerified(contact: Bytes): MaybePromise<void>
  unverify(account: string): MaybePromise<void>
  trust(device: Bytes): MaybePromise<Trust>
  acknowledgeDevice(device: Bytes): MaybePromise<void>
}

/** What the messenger asks of the server: `TesselClient`, signed in. */
export type MessengerServer = Pick<TesselClient, 'claimKeyPackages' | 'send' | 'fetch'>

/** What can stop a conversation from starting. */
export class MessengerError extends Error {
  readonly code: 'E_MESSENGER_NO_DEVICE' | 'E_MESSENGER_WRONG_KEY_PACKAGE' | 'E_MESSENGER_NOT_A_MEMBER'

  constructor(code: MessengerError['code']) {
    super(code)
    this.name = 'MessengerError'
    this.code = code
  }
}

/** Whether `error` is the server's refusal of an envelope made in a past epoch (story 04-02). */
function isStale(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    error.code === ServerError.E_DELIVERY_STALE_EPOCH &&
    'message' in error &&
    error.message === 'E_DELIVERY_STALE_EPOCH'
  )
}

/** The group of a proposal the core read (a member leaving, which some member must commit); `undefined` for anything else. */
function proposalGroup(incoming: unknown): Bytes | undefined {
  if (typeof incoming !== 'object' || incoming === null || !('type' in incoming) || incoming.type !== 'proposal') return undefined
  return 'group' in incoming && incoming.group instanceof Uint8Array ? incoming.group : undefined
}

/** The devices a list of what the core read holds (`added`, `verifiedRemoved`); nothing when it is not one. */
function devicesIn(listed: unknown): CoreDevice[] {
  if (!Array.isArray(listed)) return []
  return listed.filter(
    (device): device is CoreDevice =>
      typeof device === 'object' &&
      device !== null &&
      typeof device.account === 'string' &&
      device.deviceId instanceof Uint8Array &&
      device.certificate instanceof Uint8Array,
  )
}

/** A commit the core applied: its group, whether it removed this device, the devices it added and the verified ones it removed. */
function commitOf(incoming: unknown): { group: Bytes; removedSelf: boolean; added: CoreDevice[]; verifiedRemoved: CoreDevice[] } | undefined {
  if (typeof incoming !== 'object' || incoming === null || !('type' in incoming) || incoming.type !== 'commit') return undefined
  if (!('group' in incoming) || !(incoming.group instanceof Uint8Array)) return undefined
  return {
    group: incoming.group,
    removedSelf: 'removedSelf' in incoming && incoming.removedSelf === true,
    added: devicesIn('added' in incoming ? incoming.added : undefined),
    verifiedRemoved: devicesIn('verifiedRemoved' in incoming ? incoming.verifiedRemoved : undefined),
  }
}

/** How much a device's trust weighs on its account's: the worst one says what to show. */
const TRUST_WEIGHT: Record<Trust, number> = { verified: 0, unverified: 1, 'new-device': 2, 'key-changed': 3 }

/** The devices grouped by account, in order of first appearance. */
function byAccount<T extends { readonly account: string }>(devices: readonly T[]): Map<string, T[]> {
  const grouped = new Map<string, T[]>()
  for (const device of devices) grouped.set(device.account, [...(grouped.get(device.account) ?? []), device])
  return grouped
}

const sameBytes = (a: Bytes, b: Bytes) => a.length === b.length && a.every((value, i) => value === b[i])

/** A fresh identifier for an envelope that is not in a send queue (commits, Welcomes, proposals). */
const sendId = () => crypto.getRandomValues(new Uint8Array(16))

export class Messenger {
  readonly #core: MessengerCore
  readonly #server: MessengerServer
  /** Catches up with the mailbox: what came is processed before a refused envelope is sent again. */
  readonly #catchUp: () => Promise<void>
  readonly #clock: () => number
  /** The groups this device proposed to leave, until a commit removes it. */
  readonly #leaving = new Set<string>()
  readonly #listeners = new Set<(alert: Alert) => void>()

  /**
   * `catchUp` drains the device's inbox (`Inbox.drain` of `@tessel/protocol/realtime`,
   * whose `deliver` is {@link Messenger.receive}); `clock` gives the time a message is
   * written, in milliseconds.
   */
  constructor(core: MessengerCore, server: MessengerServer, catchUp: () => Promise<void>, clock: () => number) {
    this.#core = core
    this.#server = server
    this.#catchUp = catchUp
    this.#clock = clock
  }

  /**
   * Starts a conversation with `account` (story 05-01): one MLS group with every device
   * of that account and every other device of this one, so the messages reach all of
   * them. Each KeyPackage the directory hands out is checked by the core, and must be of
   * the account it was asked for. The new group's commit goes to nobody (this device is
   * its only member yet); each added device gets the Welcome. The group's identifier.
   */
  start(account: string): Promise<Bytes> {
    return this.createGroup('', [account])
  }

  /**
   * Creates a group (story 05-02) named `name` (empty: none) with every device of each of
   * `accounts` and every other device of this one. This account administers it. Each
   * account must have a device. The group's identifier.
   */
  async createGroup(name: string, accounts: string[]): Promise<Bytes> {
    const own = await this.#core.device()
    const invited = await this.#claim(accounts)
    const ownDevices = await this.#claim([own.account], false)
    const group = await this.#core.createGroup()
    if (name !== '') await this.#commit(group, () => this.#core.configureGroup(group, { type: 'rename', name }))
    await this.#add(group, [...invited, ...ownDevices])
    return group
  }

  /** Invites `accounts`, all their devices, into the group; refused by the core when the group reserves it to admins. */
  async invite(group: Bytes, accounts: string[]): Promise<void> {
    await this.#add(group, await this.#claim(accounts))
  }

  /** Removes `account`, all its devices, from the group; refused by the core when the group reserves it to admins. */
  async remove(group: Bytes, account: string): Promise<void> {
    await this.#commit(group, () => this.#core.removeDevices(group, [{ account }]))
  }

  /** Renames the group, appoints or dismisses an admin, reserves membership to admins or opens it. */
  async configure(group: Bytes, change: GroupChange): Promise<void> {
    await this.#commit(group, () => this.#core.configureGroup(group, change))
  }

  /** The group's name, admins and restriction. */
  settings(group: Bytes): MaybePromise<GroupSettings> {
    return this.#core.groupSettings(group)
  }

  /**
   * Leaves the group with every device of this account: the other ones are removed by a
   * commit, then this one proposes its own departure, which the first member to receive
   * it commits. The group is gone from this device once that commit arrives.
   */
  async leave(group: Bytes): Promise<void> {
    const own = await this.#core.device()
    const members = await this.#core.members(group)
    if (members.some((member) => member.account === own.account && !sameBytes(member.deviceId, own.deviceId))) {
      await this.#commit(group, () => this.#core.removeDevices(group, [{ account: own.account }]))
    }
    this.#leaving.add(group.join())
    await this.#propose(group)
  }

  /** The KeyPackages the directory hands out for `accounts`, each checked by the core and of the account asked for. */
  async #claim(accounts: string[], required = true): Promise<{ keyPackage: Bytes; device: CoreDevice }[]> {
    const claimed: { keyPackage: Bytes; device: CoreDevice }[] = []
    for (const owner of accounts) {
      const { keyPackages } = await this.#server.claimKeyPackages(owner)
      for (const entry of keyPackages) {
        const keyPackage = fromBase64Url(entry.keyPackage)
        if (keyPackage === undefined) throw new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE')
        const { device } = await this.#core.verifyKeyPackage(keyPackage)
        if (device.account !== owner) throw new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE')
        claimed.push({ keyPackage, device })
      }
      if (required && keyPackages.length === 0) throw new MessengerError('E_MESSENGER_NO_DEVICE')
    }
    return claimed
  }

  /**
   * Adds devices by batches of {@link INVITE_BATCH}; each batch's Welcome goes to its
   * devices. What they change to verified contacts is told, as for a commit received.
   */
  async #add(group: Bytes, devices: { keyPackage: Bytes; device: CoreDevice }[]): Promise<void> {
    for (let start = 0; start < devices.length; start += INVITE_BATCH) {
      const batch = devices.slice(start, start + INVITE_BATCH)
      const welcome = await this.#commit(group, () =>
        this.#core.addDevices(
          group,
          batch.map((each) => each.keyPackage),
        ),
      )
      if (welcome === undefined) continue
      await this.#server.send(
        sendId(),
        welcome,
        batch.map((each) => each.device.deviceId),
      )
      await this.#alertAdded(
        group,
        batch.map((each) => each.device),
      )
    }
  }

  /**
   * Makes a commit and gets it accepted (story 04-02): sent to the group's other devices,
   * the removed ones included; with no other device, accepted at once. When another commit
   * won the epoch, the device catches up and the core replays the intent on the new state,
   * or drops it when the commits received already did it. The Welcome to hand, if any.
   */
  async #commit(group: Bytes, prepare: () => MaybePromise<CoreCommit>, replay = true): Promise<Bytes | undefined> {
    let recipients = await this.#core.recipients(group)
    let outgoing: CoreCommit | undefined = await prepare()
    while (outgoing !== undefined) {
      if (recipients.length > 0) {
        try {
          await this.#server.send(sendId(), outgoing.commit, recipients)
        } catch (error) {
          if (!isStale(error)) throw error
          await this.#core.commitRejected(group)
          // From within the inbox, catching up would wait for itself: the winning commit
          // comes next in the inbox anyway.
          if (!replay) return undefined
          await this.#catchUp()
          recipients = await this.#core.recipients(group)
          outgoing = await this.#core.retryCommit(group)
          continue
        }
      }
      await this.#core.commitAccepted(group)
      return outgoing.welcome
    }
    return undefined
  }

  /** Writes `text` in the group: queued by the core (online or not), then sent. */
  async write(group: Bytes, text: string): Promise<void> {
    await this.#core.queueMessage(group, { type: 'text', text }, this.#clock())
    await this.flush(group)
  }

  /**
   * Sends the group's queue, in order. A message refused because the group moved on is
   * sent again once the device caught up: the core encrypts it anew, under the same send
   * identifier.
   */
  flush(group: Bytes): Promise<void> {
    return this.#flush(group, true)
  }

  /**
   * Sends the group's queue. Without `catchUp` (from within the inbox, which would wait
   * for itself), a refusal for a past epoch stops it: what is left goes with the next
   * flush, once the commit that comes next in the inbox is processed.
   */
  async #flush(group: Bytes, catchUp: boolean): Promise<void> {
    for (;;) {
      const outbound = await this.#core.nextOutbound(group)
      if (outbound === undefined) return
      try {
        await this.#server.send(outbound.sendId, outbound.encrypted, await this.#core.recipients(group))
      } catch (error) {
        if (!isStale(error)) throw error
        if (!catchUp) return
        await this.#catchUp()
        continue
      }
      await this.#core.outboundSent(group, outbound.sendId)
    }
  }

  /**
   * Hands an envelope that arrived to the core: a Welcome joins a group, the rest goes to
   * its group. A member's departure is committed at once, by whichever member gets there
   * first; the others' commits lose the epoch and are dropped. A departure that a commit
   * left out expired with its epoch (MLS): the leaving device proposes it again.
   */
  async receive(envelope: ReceivedEnvelope): Promise<void> {
    const body = fromBase64Url(envelope.envelope)
    if (body === undefined) return
    if (envelope.seq === undefined) {
      const group = await this.#core.joinGroup(body)
      await this.#alertAdded(group, await this.#core.members(group))
      return
    }
    const incoming = await this.#core.process(body)
    const proposed = proposalGroup(incoming)
    if (proposed !== undefined) await this.#commit(proposed, () => this.#core.commitPendingProposals(proposed), false)
    const committed = commitOf(incoming)
    if (committed === undefined) return
    await this.#alertAdded(committed.group, committed.added)
    for (const [account, devices] of byAccount(committed.verifiedRemoved)) {
      this.#alert({ type: 'device-removed', group: committed.group, account, devices: devices.map((device) => device.deviceId) })
    }
    const key = committed.group.join()
    if (committed.removedSelf) this.#leaving.delete(key)
    else if (this.#leaving.has(key)) await this.#propose(committed.group)
  }

  /** Proposes this device's departure to the group's other devices. */
  async #propose(group: Bytes): Promise<void> {
    const proposal = await this.#core.leaveGroup(group)
    await this.#server.send(sendId(), proposal, await this.#core.recipients(group))
  }

  /** The conversation list, the most recently active first, with what is unread. */
  conversations() {
    return this.#core.conversations()
  }

  /**
   * The user read the conversation. With read receipts on, the others see it read, and
   * this account's other devices stop counting it unread (story 05-04).
   */
  async markRead(group: Bytes): Promise<void> {
    await this.#core.markRead(group)
    for (const owed of await this.#core.queueReceipts(this.#clock())) await this.#flush(owed, true)
  }

  /**
   * Sends the receipts this device owes for what it received (story 05-04), a few for
   * many messages: the inbox's `settle`, run once each batch of envelopes is handled.
   * From within the inbox, so without catching up.
   */
  async settle(): Promise<void> {
    for (const owed of await this.#core.queueReceipts(this.#clock())) await this.#flush(owed, false)
  }

  /** Whether this device sends read receipts and sees the others' (on by default). */
  readReceipts(): MaybePromise<boolean> {
    return this.#core.readReceipts()
  }

  /** Read receipts on or off, reciprocal as in WhatsApp: off, this device sends none and records none. Deliveries still go. */
  setReadReceipts(on: boolean): MaybePromise<void> {
    return this.#core.setReadReceipts(on)
  }

  /**
   * Listens to what must be told about verified contacts (story 05-03), as envelopes
   * arrive and invitations go out. A listener runs on its own: what it throws does not
   * stop the messenger. The function that stops listening.
   */
  onAlert(listener: (alert: Alert) => void): () => void {
    this.#listeners.add(listener)
    return () => void this.#listeners.delete(listener)
  }

  /** The devices of `account` in the group; refused when it has none there. */
  async #devices(group: Bytes, account: string): Promise<[CoreDevice, ...CoreDevice[]]> {
    const [first, ...rest] = (await this.#core.members(group)).filter((member) => member.account === account)
    if (first === undefined) throw new MessengerError('E_MESSENGER_NOT_A_MEMBER')
    return [first, ...rest]
  }

  /**
   * The safety number with `account`, a member of the group, to compare on both screens
   * or to show as a QR. It comes from the account key of the contact's first device in
   * the group: if the server slipped in a device under another key, the codes differ, or
   * that device stays unvouched once the others are verified.
   */
  async safetyNumber(group: Bytes, account: string): Promise<SafetyNumber> {
    const [device] = await this.#devices(group, account)
    return this.#core.safetyNumber(device.certificate)
  }

  /**
   * The user compared the safety numbers and they are the same: `account` is verified,
   * under the account key the number came from, with its devices of the moment.
   */
  async markVerified(group: Bytes, account: string): Promise<void> {
    const [device] = await this.#devices(group, account)
    await this.#core.markVerified(device.certificate)
  }

  /**
   * Verifies `account` in one gesture: the QR its device shows was scanned. It must hold
   * the account key of one of its devices in the group, and this account's; refused by
   * the core otherwise (`safety:mismatch`, `safety:malformed`…), and nothing changes.
   */
  async verifyScanned(group: Bytes, account: string, scanned: Bytes): Promise<void> {
    let refusal: unknown
    for (const device of await this.#devices(group, account)) {
      try {
        await this.#core.checkScanned(device.certificate, scanned)
      } catch (error) {
        refusal = error
        continue
      }
      await this.#core.markVerified(device.certificate)
      return
    }
    throw refusal
  }

  /** `account` is no longer verified. */
  unverify(account: string): MaybePromise<void> {
    return this.#core.unverify(account)
  }

  /**
   * What the conversation shows of each other account in it: verified, not, with a new
   * device, or with a safety number that changed. The worst of its devices counts.
   */
  async verification(group: Bytes): Promise<{ account: string; trust: Trust }[]> {
    const own = await this.#core.device()
    const others = (await this.#core.members(group)).filter((member) => member.account !== own.account)
    const accounts: { account: string; trust: Trust }[] = []
    for (const [account, devices] of byAccount(others)) {
      let trust: Trust = 'verified'
      for (const device of devices) {
        const each = await this.#core.trust(device.certificate)
        if (TRUST_WEIGHT[each] > TRUST_WEIGHT[trust]) trust = each
      }
      accounts.push({ account, trust })
    }
    return accounts
  }

  /** The user saw the new devices of a verified `account` in the group: they are no longer reported. */
  async acknowledgeDevices(group: Bytes, account: string): Promise<void> {
    for (const device of await this.#devices(group, account)) {
      if ((await this.#core.trust(device.certificate)) === 'new-device') await this.#core.acknowledgeDevice(device.certificate)
    }
  }

  /** Tells what devices entering a group change to verified contacts: a key that changed, or new devices. */
  async #alertAdded(group: Bytes, devices: readonly CoreDevice[]): Promise<void> {
    for (const [account, own] of byAccount(devices)) {
      const added: Bytes[] = []
      let changed = false
      for (const device of own) {
        const trust = await this.#core.trust(device.certificate)
        if (trust === 'key-changed') changed = true
        else if (trust === 'new-device') added.push(device.deviceId)
      }
      if (changed) this.#alert({ type: 'key-changed', group, account })
      else if (added.length > 0) this.#alert({ type: 'new-device', group, account, devices: added })
    }
  }

  #alert(alert: Alert): void {
    for (const listener of this.#listeners) queueMicrotask(() => listener(alert))
  }
}
