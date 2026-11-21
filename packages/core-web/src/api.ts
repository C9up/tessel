/**
 * The core's API, as the web client sees it (story 02-10): asynchronous, typed, made of
 * plain objects that cross a Web Worker boundary. The mobile binding (story 02-13)
 * offers the same interface; the contract tests (`contract.test.ts`) run on both.
 *
 * Nothing here holds a private key. A media's key never reaches the page either: the
 * page holds a `secret` the core sealed for the time it runs, and hands it back.
 */

import type { OutboundMessage } from './outbox.ts'
import type { Bytes } from './store.ts'

/** A device, public data only. `certificate` is what to hand back about it. */
export interface DeviceInfo {
  /** `name@domain`. */
  account: string
  deviceId: Bytes
  accountKey: Bytes
  deviceKey: Bytes
  certificate: Bytes
}

/** What to send to the directory after renewing the *KeyPackages*. */
export interface KeyPackageUpdate {
  publish: Bytes[]
  lastResort?: Bytes
  /** References of the *KeyPackages* to withdraw. */
  withdraw: Bytes[]
}

/** A *KeyPackage* checked by the core. */
export interface KeyPackageInfo {
  device: DeviceInfo
  reference: Bytes
  lastResort: boolean
  /** The end of its validity, in Unix seconds. */
  notAfter: number
}

/** A commit for the members, and the *Welcome* for the added devices. */
export interface Commit {
  commit: Bytes
  welcome?: Bytes
}

/** Whom to remove from a group: an account, or one of its devices. */
export interface Removal {
  account: string
  deviceId?: Bytes
}

/** A media to send: the secret `encryptMedia` gave, and where the server keeps the blob. */
export interface OutgoingMedia {
  secret: Bytes
  blob: Bytes
  contentType: string
  name?: string
}

/** A media as received: what to show, and the secret to decrypt it with. */
export interface MediaInfo {
  blob: Bytes
  size: number
  contentType: string
  name: string
  secret: Bytes
}

export type ReceiptKind = 'delivered' | 'read'

/** What the app writes. */
export type OutgoingBody =
  | { type: 'text'; text: string; replyTo?: Bytes }
  | { type: 'media'; media: OutgoingMedia; caption?: string; replyTo?: Bytes }
  | { type: 'receipt'; kind: ReceiptKind; messages: Bytes[] }
  | { type: 'edit'; target: Bytes; text: string }
  | { type: 'delete'; target: Bytes }

/** What the app reads. */
export type Body =
  | { type: 'text'; text: string; replyTo?: Bytes }
  | { type: 'media'; media: MediaInfo; caption: string; replyTo?: Bytes }
  | { type: 'receipt'; kind: ReceiptKind; messages: Bytes[] }
  | { type: 'edit'; target: Bytes; text: string }
  | { type: 'delete'; target: Bytes }

export interface ReceivedMessage {
  id: Bytes
  /** The sender's clock, in milliseconds: it can lie, only the server's order holds. */
  sentAt: number
  body: Body
}

export type Ignored = 'not-author' | 'unknown-target' | 'already-deleted' | 'duplicate-id'

export type Incoming =
  | {
      type: 'message'
      group: Bytes
      /** Authenticated even when the message cannot be read. */
      sender: DeviceInfo
      message?: ReceivedMessage
      /** Why it cannot be read (an error code, `message:unsupported-version`…). */
      unreadable?: string
      effect: 'added' | 'edited' | 'deleted' | 'nothing' | 'ignored'
      ignored?: Ignored
    }
  | { type: 'proposal'; group: Bytes; sender: DeviceInfo }
  | {
      type: 'commit'
      group: Bytes
      sender: DeviceInfo
      added: DeviceInfo[]
      removed: DeviceInfo[]
      /** This device was removed: the group is gone from it. */
      removedSelf: boolean
      /** Its messages that will never be sent. */
      unsent: Bytes[]
    }

export type Content =
  | { type: 'text'; text: string; replyTo?: Bytes; editedAt?: number }
  | { type: 'media'; media: MediaInfo; caption: string; replyTo?: Bytes; editedAt?: number }
  | { type: 'deleted' }

/** A message of the history, its last edit applied. */
export interface HistoryEntry {
  id: Bytes
  author: string
  deviceId: Bytes
  sentAt: number
  content: Content
}

/** A queued message: its id, and what the server deduplicates on. */
export interface Queued {
  messageId: Bytes
  sendId: Bytes
}

export interface EncryptedFile {
  /** The blob to upload. */
  encrypted: Bytes
  size: number
  /** The sealed secret, for `queueMessage`. */
  secret: Bytes
}

export interface SafetyNumber {
  /** 60 digits, to show in groups of five. */
  digits: string
  /** What to show as a QR. */
  qr: Bytes
}

export type Trust = 'unverified' | 'verified' | 'new-device' | 'key-changed'

/**
 * The core. Every call that changes the device's state persists it before it resolves:
 * once a call answered, what it returned can be acted on (sent, shown).
 */
export interface TesselCore {
  version(): Promise<string>
  /** IANA name of the MLS ciphersuite (ADR-001). */
  ciphersuite(): Promise<string>

  /** Opens this profile's device; `undefined` when no account was created on it yet. */
  start(): Promise<DeviceInfo | undefined>
  /** Creates an account's first device; its account key stays in the local database. */
  createAccount(account: string): Promise<DeviceInfo>
  device(): Promise<DeviceInfo>
  holdsAccountKey(): Promise<boolean>
  /** Signs the server's sign-in challenge with the device key: the proof for `identity.signIn`. */
  signIn(challenge: Bytes): Promise<Bytes>
  /** Releases the core and the database. */
  close(): Promise<void>

  /** Renews the *KeyPackage* pool at `now` (Unix seconds). */
  refreshKeyPackages(now: number): Promise<KeyPackageUpdate>
  verifyKeyPackage(keyPackage: Bytes): Promise<KeyPackageInfo>

  createGroup(): Promise<Bytes>
  addDevices(group: Bytes, keyPackages: Bytes[]): Promise<Commit>
  removeDevices(group: Bytes, removals: Removal[]): Promise<Commit>
  commitPendingProposals(group: Bytes): Promise<Commit>
  commitAccepted(group: Bytes): Promise<void>
  commitRejected(group: Bytes): Promise<void>
  /** `undefined` when the commits received meanwhile already did the job. */
  retryCommit(group: Bytes): Promise<Commit | undefined>
  leaveGroup(group: Bytes): Promise<Bytes>
  joinGroup(welcome: Bytes): Promise<Bytes>
  members(group: Bytes): Promise<DeviceInfo[]>
  epoch(group: Bytes): Promise<number>
  process(message: Bytes): Promise<Incoming>

  /** Records the message and queues it, online or not; `sentAt` in milliseconds. */
  queueMessage(group: Bytes, body: OutgoingBody, sentAt: number): Promise<Queued>
  nextOutbound(group: Bytes): Promise<OutboundMessage | undefined>
  outboundSent(group: Bytes, sendId: Bytes): Promise<void>
  outboundGroups(): Promise<Bytes[]>
  waitingMessages(group: Bytes): Promise<Bytes[]>
  history(group: Bytes): Promise<HistoryEntry[]>

  encryptMedia(file: Bytes): Promise<EncryptedFile>
  decryptMedia(secret: Bytes, encrypted: Bytes): Promise<Bytes>

  /** About a contact's device, by its certificate. */
  safetyNumber(contact: Bytes): Promise<SafetyNumber>
  checkScanned(contact: Bytes, scanned: Bytes): Promise<void>
  markVerified(contact: Bytes): Promise<void>
  unverify(account: string): Promise<void>
  trust(device: Bytes): Promise<Trust>
  acknowledgeDevice(device: Bytes): Promise<void>
}

/** Every method of `TesselCore`, for the worker to answer only those. */
export const METHODS = [
  'version',
  'ciphersuite',
  'start',
  'createAccount',
  'device',
  'holdsAccountKey',
  'signIn',
  'close',
  'refreshKeyPackages',
  'verifyKeyPackage',
  'createGroup',
  'addDevices',
  'removeDevices',
  'commitPendingProposals',
  'commitAccepted',
  'commitRejected',
  'retryCommit',
  'leaveGroup',
  'joinGroup',
  'members',
  'epoch',
  'process',
  'queueMessage',
  'nextOutbound',
  'outboundSent',
  'outboundGroups',
  'waitingMessages',
  'history',
  'encryptMedia',
  'decryptMedia',
  'safetyNumber',
  'checkScanned',
  'markVerified',
  'unverify',
  'trust',
  'acknowledgeDevice',
] as const satisfies readonly (keyof TesselCore)[]

// Fails to compile when a method of `TesselCore` is missing from `METHODS`.
const everyMethod: [Exclude<keyof TesselCore, (typeof METHODS)[number]>] extends [never] ? true : never = true
void everyMethod

/** An error of the core, with a stable code (`group:pending-proposals`, `store:unreadable`…). */
export class TesselError extends Error {
  readonly code: string

  constructor(code: string, message: string) {
    super(message)
    this.name = 'TesselError'
    this.code = code
  }

  /** Any error, as a `TesselError`: the core's keep their code, the others get `host:…`. */
  static from(error: unknown): TesselError {
    if (error instanceof TesselError) return error
    if (error instanceof Error) {
      const code = 'code' in error && typeof error.code === 'string' && error.name === 'TesselError' ? error.code : `host:${error.name}`
      return new TesselError(code, error.message)
    }
    return new TesselError('host:unknown', String(error))
  }
}

/** The core's send queue, for `OutboxSender`. */
export function outboxOf(core: TesselCore) {
  return {
    groups: () => core.outboundGroups(),
    next: (group: Bytes) => core.nextOutbound(group),
    sent: (message: OutboundMessage) => core.outboundSent(message.group, message.sendId),
  }
}
