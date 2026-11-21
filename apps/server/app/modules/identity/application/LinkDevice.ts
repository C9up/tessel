import { IdentityError } from '../domain/account.js'
import { DeviceAdded } from '../domain/events.js'
import type { CertificateVerifier, Devices, LinkMailbox } from '../domain/ports.js'
import type { SignedInDevice } from '../domain/session.js'

/** Live devices an account may have: the first one and its linked devices. */
export const MAX_DEVICES = 10

const hex = (bytes: Uint8Array) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')

/**
 * Adding a device by QR code (story 03-04). The device holding the account key certified
 * the new device's key; it hands the server that certificate, to register the device,
 * and the grant sealed for the new device, which the server keeps in the link's mailbox
 * without being able to open it. The new device takes it from there, once.
 */
export class LinkDevice {
  readonly #verifier: CertificateVerifier
  readonly #devices: Devices
  readonly #mailbox: LinkMailbox

  constructor(verifier: CertificateVerifier, devices: Devices, mailbox: LinkMailbox) {
    this.#verifier = verifier
    this.#devices = devices
    this.#mailbox = mailbox
  }

  /**
   * Registers the certified device in the caller's account and posts the grant. The
   * certificate must name the caller's account and be signed by its account key.
   */
  async run(caller: SignedInDevice, certificate: Uint8Array, link: Uint8Array, grant: Uint8Array): Promise<void> {
    const device = this.#verifier.verify(certificate)
    if (device === null || device.account !== caller.account) throw new IdentityError('invalidCertificate')
    if ((await this.#devices.live(caller.accountId)) >= MAX_DEVICES) throw new IdentityError('tooManyDevices')
    // The mailbox first: a link takes one grant, the first one.
    if (!(await this.#mailbox.post(link, grant))) throw new IdentityError('linkUsed')
    if (!(await this.#devices.add(caller.accountId, device))) {
      await this.#mailbox.take(link)
      throw new IdentityError('invalidCertificate')
    }
    await new DeviceAdded(caller.accountId, hex(device.deviceId)).emit()
  }

  /** The grant waiting in a link's mailbox, handed out once; `null` when there is none (yet). */
  takeGrant(link: Uint8Array): Promise<Uint8Array | null> {
    return this.#mailbox.take(link)
  }
}
