/**
 * The mailboxes (story 04-01): the server is a blind postman. An envelope is an MLS
 * message it cannot open; it reads its public header (kind, group, epoch) and nothing
 * else, keeps it once, and hands it to each device it is for. Who those devices are, the
 * sender says: the server does not know a group's members, they are in the encrypted
 * MLS state.
 */

/** What kind of MLS message an envelope carries, as its public header says. */
export type EnvelopeKind = 'application' | 'proposal' | 'commit' | 'welcome'

/** The public header of an envelope. */
export interface EnvelopeHeader {
  readonly kind: EnvelopeKind
  /** Absent for a *Welcome*. */
  readonly group?: Uint8Array
  /** Absent for a *Welcome*. */
  readonly epoch?: number
}

/** At most this many devices per envelope: a large group's worth. */
export const MAX_RECIPIENTS = 1000

/** A coarse bound on an envelope; story 04-06 sets the limits per kind. */
export const MAX_ENVELOPE = 1024 * 1024

/** What a device posts. */
export interface OutgoingEnvelope {
  /** 16 random bytes the sender drew (story 02-07): the same envelope sent again keeps it. */
  readonly sendId: Uint8Array
  readonly body: Uint8Array
  readonly recipients: readonly Uint8Array[]
}

/** An envelope, as the server keeps it and hands it out. */
export interface StoredEnvelope {
  /** The server's identifier: the order envelopes arrived in. */
  readonly id: string
  readonly header: EnvelopeHeader
  readonly body: Uint8Array
}

export type DeliveryErrorCode = 'invalidEnvelope' | 'noRecipient'

export class DeliveryError extends Error {
  readonly code: DeliveryErrorCode

  constructor(code: DeliveryErrorCode) {
    super(code)
    this.name = 'DeliveryError'
    this.code = code
  }
}
