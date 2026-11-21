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

/**
 * What the messenger asks of the core: the web binding's `TesselCore` (story 02-10)
 * answers it, the mobile binding (story 02-13) will too.
 */
export interface MessengerCore {
  device(): MaybePromise<CoreDevice>
  verifyKeyPackage(keyPackage: Bytes): MaybePromise<{ readonly device: CoreDevice }>
  createGroup(): MaybePromise<Bytes>
  addDevices(group: Bytes, keyPackages: Bytes[]): MaybePromise<{ readonly commit: Bytes; readonly welcome?: Bytes }>
  commitAccepted(group: Bytes): MaybePromise<void>
  joinGroup(welcome: Bytes): MaybePromise<Bytes>
  process(message: Bytes): MaybePromise<unknown>
  recipients(group: Bytes): MaybePromise<Bytes[]>
  queueMessage(group: Bytes, body: { type: 'text'; text: string }, sentAt: number): MaybePromise<unknown>
  nextOutbound(group: Bytes): MaybePromise<{ readonly sendId: Bytes; readonly encrypted: Bytes } | undefined>
  outboundSent(group: Bytes, sendId: Bytes): MaybePromise<void>
  conversations(): MaybePromise<readonly { readonly group: Bytes; readonly lastSentAt?: number; readonly unread: number }[]>
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

export class Messenger {
  readonly #core: MessengerCore
  readonly #server: MessengerServer
  /** Catches up with the mailbox: what came is processed before a refused envelope is sent again. */
  readonly #catchUp: () => Promise<void>
  readonly #clock: () => number

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
  async start(account: string): Promise<Bytes> {
    const own = await this.#core.device()
    const keyPackages: Bytes[] = []
    const added: Bytes[] = []
    for (const owner of [account, own.account]) {
      const { keyPackages: claimed } = await this.#server.claimKeyPackages(owner)
      for (const entry of claimed) {
        const keyPackage = fromBase64Url(entry.keyPackage)
        if (keyPackage === undefined) throw new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE')
        const { device } = await this.#core.verifyKeyPackage(keyPackage)
        if (device.account !== owner) throw new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE')
        keyPackages.push(keyPackage)
        added.push(device.deviceId)
      }
      if (owner === account && claimed.length === 0) throw new MessengerError('E_MESSENGER_NO_DEVICE')
    }
    const group = await this.#core.createGroup()
    const { welcome } = await this.#core.addDevices(group, keyPackages)
    await this.#core.commitAccepted(group)
    if (welcome !== undefined) await this.#server.send(crypto.getRandomValues(new Uint8Array(16)), welcome, added)
    return group
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

  /** Hands an envelope that arrived to the core: a Welcome joins a group, the rest goes to its group. */
  async receive(envelope: ReceivedEnvelope): Promise<void> {
    const body = fromBase64Url(envelope.envelope)
    if (body === undefined) return
    if (envelope.seq === undefined) await this.#core.joinGroup(body)
    else await this.#core.process(body)
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
