import { DeliveryError, MAX_ENVELOPE, MAX_RECIPIENTS, type OutgoingEnvelope, type Placed, type StoredEnvelope } from '../domain/envelope.js'
import type { EnvelopeReader, Mailboxes } from '../domain/ports.js'

const hex = (bytes: Uint8Array) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')

/** Envelopes handed out per fetch, at most. */
export const FETCH_LIMIT = 100

/**
 * A device's mailbox (story 04-01): it posts envelopes for other devices, and fetches
 * those for itself, and only those.
 */
export class Mailbox {
  readonly #reader: EnvelopeReader
  readonly #mailboxes: Mailboxes

  constructor(reader: EnvelopeReader, mailboxes: Mailboxes) {
    this.#reader = reader
    this.#mailboxes = mailboxes
  }

  /** Keeps `envelope` from `sender` for its recipients; its identifier. */
  async send(sender: Uint8Array, envelope: OutgoingEnvelope): Promise<Placed> {
    if (envelope.body.length === 0 || envelope.body.length > MAX_ENVELOPE) throw new DeliveryError('invalidEnvelope')
    const header = this.#reader.read(envelope.body)
    if (header === null) throw new DeliveryError('invalidEnvelope')
    // Each device once, and never the sender itself.
    const own = hex(sender)
    const recipients = [...new Map(envelope.recipients.map((device) => [hex(device), device])).entries()]
      .filter(([key]) => key !== own)
      .map(([, device]) => device)
    if (recipients.length === 0) throw new DeliveryError('noRecipient')
    if (recipients.length > MAX_RECIPIENTS) throw new DeliveryError('invalidEnvelope')
    const placed = await this.#mailboxes.store(sender, envelope.sendId, header, envelope.body, recipients)
    if (placed === null) throw new DeliveryError('staleEpoch')
    return placed
  }

  /** The envelopes for `device` after `after`, oldest first. */
  fetch(device: Uint8Array, after: string | null): Promise<StoredEnvelope[]> {
    return this.#mailboxes.fetch(device, after, FETCH_LIMIT)
  }
}
