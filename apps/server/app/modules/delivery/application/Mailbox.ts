import {
  DeliveryError,
  ENVELOPE_LIMITS,
  type EnvelopeLimits,
  MAX_RECIPIENTS,
  type OutgoingEnvelope,
  type Placed,
  type StoredEnvelope,
} from '../domain/envelope.js'
import type { Doorbell, EnvelopeReader, Mailboxes } from '../domain/ports.js'

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
  readonly #doorbell: Doorbell
  readonly #limits: EnvelopeLimits

  constructor(reader: EnvelopeReader, mailboxes: Mailboxes, doorbell: Doorbell, limits: EnvelopeLimits = ENVELOPE_LIMITS) {
    this.#reader = reader
    this.#mailboxes = mailboxes
    this.#doorbell = doorbell
    this.#limits = limits
  }

  /** Keeps `envelope` from `sender` for its recipients; its identifier. */
  async send(sender: Uint8Array, envelope: OutgoingEnvelope): Promise<Placed> {
    if (envelope.body.length === 0) throw new DeliveryError('invalidEnvelope')
    // Nothing larger than the largest kind is even parsed; then each kind its own size.
    if (envelope.body.length > Math.max(...Object.values(this.#limits))) throw new DeliveryError('tooLarge')
    const header = this.#reader.read(envelope.body)
    if (header === null) throw new DeliveryError('invalidEnvelope')
    if (envelope.body.length > this.#limits[header.kind]) throw new DeliveryError('tooLarge')
    // Each device once, and never the sender itself.
    const own = hex(sender)
    const recipients = [...new Map(envelope.recipients.map((device) => [hex(device), device])).entries()]
      .filter(([key]) => key !== own)
      .map(([, device]) => device)
    if (recipients.length === 0) throw new DeliveryError('noRecipient')
    if (recipients.length > MAX_RECIPIENTS) throw new DeliveryError('invalidEnvelope')
    const placed = await this.#mailboxes.store(sender, envelope.sendId, header, envelope.body, recipients)
    if (placed === null) throw new DeliveryError('staleEpoch')
    // Kept first, rung after: a device that hears the ring finds the envelope. (A
    // duplicate rings again; the device's cursor makes that a no-op.)
    this.#doorbell.ring(recipients, placed.id)
    return placed
  }

  /** `device` handled every envelope up to `through`: the server lets them go. */
  ack(device: Uint8Array, through: string): Promise<number> {
    return this.#mailboxes.ack(device, through)
  }

  /** The purge (story 04-04): what was kept for more than `retentionDays` goes. */
  purge(retentionDays: number): Promise<number> {
    return this.#mailboxes.purge(retentionDays)
  }

  /** The envelopes for `device` after `after`, oldest first. */
  fetch(device: Uint8Array, after: string | null): Promise<StoredEnvelope[]> {
    return this.#mailboxes.fetch(device, after, FETCH_LIMIT)
  }
}
