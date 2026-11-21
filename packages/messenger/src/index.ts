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

/** A device, as the core describes one. */
export interface CoreDevice {
  readonly account: string
  readonly deviceId: Bytes
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
}

/** What the messenger asks of the server: `TesselClient`, signed in. */
export type MessengerServer = Pick<TesselClient, 'claimKeyPackages' | 'send' | 'fetch'>

/** What can stop a conversation from starting. */
export class MessengerError extends Error {
  readonly code: 'E_MESSENGER_NO_DEVICE' | 'E_MESSENGER_WRONG_KEY_PACKAGE'

  constructor(code: 'E_MESSENGER_NO_DEVICE' | 'E_MESSENGER_WRONG_KEY_PACKAGE') {
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

/** A commit the core applied: its group, and whether it removed this device. */
function commitOf(incoming: unknown): { group: Bytes; removedSelf: boolean } | undefined {
  if (typeof incoming !== 'object' || incoming === null || !('type' in incoming) || incoming.type !== 'commit') return undefined
  if (!('group' in incoming) || !(incoming.group instanceof Uint8Array)) return undefined
  return { group: incoming.group, removedSelf: 'removedSelf' in incoming && incoming.removedSelf === true }
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
  async #claim(accounts: string[], required = true): Promise<{ keyPackage: Bytes; device: Bytes }[]> {
    const claimed: { keyPackage: Bytes; device: Bytes }[] = []
    for (const owner of accounts) {
      const { keyPackages } = await this.#server.claimKeyPackages(owner)
      for (const entry of keyPackages) {
        const keyPackage = fromBase64Url(entry.keyPackage)
        if (keyPackage === undefined) throw new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE')
        const { device } = await this.#core.verifyKeyPackage(keyPackage)
        if (device.account !== owner) throw new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE')
        claimed.push({ keyPackage, device: device.deviceId })
      }
      if (required && keyPackages.length === 0) throw new MessengerError('E_MESSENGER_NO_DEVICE')
    }
    return claimed
  }

  /** Adds devices by batches of {@link INVITE_BATCH}; each batch's Welcome goes to its devices. */
  async #add(group: Bytes, devices: { keyPackage: Bytes; device: Bytes }[]): Promise<void> {
    for (let start = 0; start < devices.length; start += INVITE_BATCH) {
      const batch = devices.slice(start, start + INVITE_BATCH)
      const welcome = await this.#commit(group, () =>
        this.#core.addDevices(
          group,
          batch.map((each) => each.keyPackage),
        ),
      )
      if (welcome !== undefined)
        await this.#server.send(
          sendId(),
          welcome,
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
  async flush(group: Bytes): Promise<void> {
    for (;;) {
      const outbound = await this.#core.nextOutbound(group)
      if (outbound === undefined) return
      try {
        await this.#server.send(outbound.sendId, outbound.encrypted, await this.#core.recipients(group))
      } catch (error) {
        if (!isStale(error)) throw error
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
      await this.#core.joinGroup(body)
      return
    }
    const incoming = await this.#core.process(body)
    const proposed = proposalGroup(incoming)
    if (proposed !== undefined) await this.#commit(proposed, () => this.#core.commitPendingProposals(proposed), false)
    const committed = commitOf(incoming)
    if (committed === undefined) return
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

  /** The user read the conversation. */
  markRead(group: Bytes): MaybePromise<void> {
    return this.#core.markRead(group)
  }
}
