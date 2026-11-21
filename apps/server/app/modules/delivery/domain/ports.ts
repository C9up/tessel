import type { EnvelopeHeader, Placed, StoredEnvelope } from './envelope.js'

/** Reads an envelope's public header: the core does. */
export interface EnvelopeReader {
  /** `null` when it is not a group's MLS message. */
  read(body: Uint8Array): EnvelopeHeader | null
}

/**
 * Tells devices that an envelope arrived for them (story 04-03): a ring that carries its
 * identifier, never its body. The mailbox stays the one source of truth: a ring lost on
 * the way costs nothing, the device fetches from its cursor when it reconnects.
 */
export interface Doorbell {
  ring(devices: readonly Uint8Array[], envelope: string): void
}

/** The mailboxes. */
export interface Mailboxes {
  /**
   * Keeps the envelope once, with the devices it is for. An envelope the same device
   * already sent under `sendId` is not kept again: where it was put comes back. A
   * group's envelope is put in the group's order, in one step: it must be of the
   * group's current epoch, and a commit moves the group to the next. `null` when it is
   * not (story 04-02).
   */
  store(sender: Uint8Array, sendId: Uint8Array, header: EnvelopeHeader, body: Uint8Array, recipients: readonly Uint8Array[]): Promise<Placed | null>
  /** The envelopes for `device` that came after `after`, oldest first, at most `limit`. */
  fetch(device: Uint8Array, after: string | null, limit: number): Promise<StoredEnvelope[]>
}
