/**
 * The core in the crypto worker (story 02-10): the WASM client, and the IndexedDB
 * database it is persisted in (story 02-06).
 *
 * Calls run one at a time. After each one, what the core changed is persisted, in one
 * transaction, before the call resolves: the storage contract of ADR-007, so that
 * nothing the core returns (a message to send, a commit) is acted on before the state
 * that produced it is on disk. Even a failed call persists what it changed.
 */

import { Core, ciphersuite, generateStoreKey, PendingLink, statePartition, verifyKeyPackage, version } from '../generated/tessel_wasm.js'
import {
  type DeviceInfo,
  type LinkGrant,
  type LinkRequestInfo,
  type OutgoingBody,
  type Profile,
  type Removal,
  type TesselCore,
  TesselError,
  type Trust,
} from './api.ts'
import { type Bytes, IndexedDbStore, loadStoreKey, type StoreChanges } from './store.ts'

export interface CoreHostOptions {
  /** The IndexedDB database of this profile. */
  database: string
  factory?: IDBFactory
}

/** Bytes the generated bindings return, typed as this package's `Bytes`. */
function own(bytes: Uint8Array): Bytes {
  return bytes.buffer instanceof ArrayBuffer ? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength) : Uint8Array.from(bytes)
}

const TRUST: readonly string[] = ['unverified', 'verified', 'new-device', 'key-changed'] satisfies Trust[]

function isTrust(value: string): value is Trust {
  return TRUST.includes(value)
}

export class CoreHost implements TesselCore {
  readonly #database: string
  readonly #factory: IDBFactory | undefined
  #store: IndexedDbStore | undefined
  #core: Core | undefined
  /** The store key of a profile without an account yet, until `createAccount`. */
  #pendingKey: Bytes | undefined
  /** A new device's key waiting for its grant, until `completeLink` (story 03-04). */
  #pendingLink: PendingLink | undefined
  #queue: Promise<unknown> = Promise.resolve()

  constructor(options: CoreHostOptions) {
    this.#database = options.database
    this.#factory = options.factory
  }

  /** Runs `task` after the previous calls, then persists; errors become `TesselError`. */
  #serial<T>(task: () => T | Promise<T>): Promise<T> {
    const result = this.#queue.then(async () => {
      try {
        return await task()
      } finally {
        await this.#persist()
      }
    })
    this.#queue = result.catch(() => undefined)
    return result.catch((error: unknown) => {
      throw TesselError.from(error)
    })
  }

  /** A call on the started core. */
  #call<T>(task: (core: Core) => T | Promise<T>): Promise<T> {
    return this.#serial(() => task(this.#started()))
  }

  #started(): Core {
    if (this.#core === undefined) throw new TesselError('host:not-started', 'no device: call start, then createAccount if needed')
    return this.#core
  }

  async #persist(): Promise<void> {
    if (this.#core === undefined || this.#store === undefined) return
    const changes: StoreChanges = this.#core.takeChanges()
    if (changes.put.length > 0 || changes.delete.length > 0) await this.#store.apply(changes)
  }

  async version(): Promise<string> {
    return version()
  }

  async ciphersuite(): Promise<string> {
    return ciphersuite()
  }

  start(): Promise<DeviceInfo | undefined> {
    return this.#serial(async () => {
      if (this.#core !== undefined) return this.#core.device()
      if (this.#pendingKey !== undefined) return undefined
      this.#store ??= await IndexedDbStore.open(this.#database, this.#factory)
      const key = await loadStoreKey(this.#store, generateStoreKey)
      const state = await this.#store.list(own(statePartition(key)))
      if (state.length === 0) {
        this.#pendingKey = key
        return undefined
      }
      try {
        this.#core = Core.open(key, state)
      } finally {
        key.fill(0)
      }
      return this.#core.device()
    })
  }

  createAccount(account: string): Promise<DeviceInfo> {
    return this.#serial(() => {
      const key = this.#pendingKey
      if (key === undefined) {
        throw new TesselError(this.#core === undefined ? 'host:not-started' : 'host:account-exists', 'createAccount needs a started profile without a device')
      }
      this.#core = Core.createAccount(account, key)
      key.fill(0)
      this.#pendingKey = undefined
      return this.#core.device()
    })
  }

  device(): Promise<DeviceInfo> {
    return this.#call((core) => core.device())
  }

  holdsAccountKey(): Promise<boolean> {
    return this.#call((core) => core.holdsAccountKey())
  }

  signIn(challenge: Bytes): Promise<Bytes> {
    return this.#call((core) => own(core.signIn(challenge)))
  }

  requestLink(): Promise<LinkRequestInfo> {
    return this.#serial(() => {
      this.#awaitingDevice('requestLink')
      this.#pendingLink?.free()
      this.#pendingLink = new PendingLink()
      return { qr: own(this.#pendingLink.qr()), link: own(this.#pendingLink.link()) }
    })
  }

  completeLink(grant: Bytes): Promise<DeviceInfo> {
    return this.#serial(() => {
      const key = this.#awaitingDevice('completeLink')
      const pending = this.#pendingLink
      if (pending === undefined) throw new TesselError('host:no-link', 'completeLink needs requestLink first')
      // The link is spent whatever happens: a refused grant means a new QR code.
      this.#pendingLink = undefined
      try {
        this.#core = pending.accept(grant, key)
      } finally {
        pending.free()
      }
      key.fill(0)
      this.#pendingKey = undefined
      return this.#core.device()
    })
  }

  grantLink(qr: Bytes): Promise<LinkGrant> {
    return this.#call((core) => {
      const granted: LinkGrant = core.grantLink(qr)
      return { link: own(granted.link), certificate: own(granted.certificate), grant: own(granted.grant) }
    })
  }

  groups(): Promise<Bytes[]> {
    return this.#call((core) => core.groups())
  }

  revokeDevice(deviceId: Bytes): Promise<Bytes> {
    return this.#call((core) => own(core.revokeDevice(deviceId)))
  }

  groupsWith(deviceId: Bytes): Promise<Bytes[]> {
    return this.#call((core) => core.groupsWith(deviceId))
  }

  sealProfile(profile: Profile): Promise<Bytes> {
    return this.#call((core) => own(core.sealProfile(profile)))
  }

  openProfile(account: string, blob: Bytes): Promise<Profile> {
    return this.#call((core) => core.openProfile(account, blob))
  }

  knowsProfile(account: string): Promise<boolean> {
    return this.#call((core) => core.knowsProfile(account))
  }

  shareProfile(group: Bytes, sentAt: number): Promise<Bytes> {
    return this.#call((core) => own(core.shareProfile(group, sentAt)))
  }

  /** The store key of a started profile without a device; refuses otherwise. */
  #awaitingDevice(method: string): Bytes {
    const key = this.#pendingKey
    if (key === undefined) {
      throw new TesselError(this.#core === undefined ? 'host:not-started' : 'host:account-exists', `${method} needs a started profile without a device`)
    }
    return key
  }

  close(): Promise<void> {
    return this.#serial(() => {
      this.#pendingKey?.fill(0)
      this.#pendingKey = undefined
      this.#pendingLink?.free()
      this.#pendingLink = undefined
      // Persist what is left first: `#serial` persists after this task, so free after.
    }).finally(() => {
      this.#core?.free()
      this.#core = undefined
      this.#store?.close()
      this.#store = undefined
    })
  }

  refreshKeyPackages(now: number) {
    return this.#call((core) => core.refreshKeyPackages(now))
  }

  async verifyKeyPackage(keyPackage: Bytes) {
    try {
      return verifyKeyPackage(keyPackage)
    } catch (error) {
      throw TesselError.from(error)
    }
  }

  createGroup(): Promise<Bytes> {
    return this.#call((core) => own(core.createGroup()))
  }

  addDevices(group: Bytes, keyPackages: Bytes[]) {
    return this.#call((core) => core.addDevices(group, keyPackages))
  }

  removeDevices(group: Bytes, removals: Removal[]) {
    return this.#call((core) => core.removeDevices(group, removals))
  }

  commitPendingProposals(group: Bytes) {
    return this.#call((core) => core.commitPendingProposals(group))
  }

  commitAccepted(group: Bytes): Promise<void> {
    return this.#call((core) => core.commitAccepted(group))
  }

  commitRejected(group: Bytes): Promise<void> {
    return this.#call((core) => core.commitRejected(group))
  }

  retryCommit(group: Bytes) {
    return this.#call((core) => core.retryCommit(group))
  }

  leaveGroup(group: Bytes): Promise<Bytes> {
    return this.#call((core) => own(core.leaveGroup(group)))
  }

  joinGroup(welcome: Bytes): Promise<Bytes> {
    return this.#call((core) => own(core.joinGroup(welcome)))
  }

  members(group: Bytes) {
    return this.#call((core) => core.members(group))
  }

  epoch(group: Bytes): Promise<number> {
    return this.#call((core) => core.epoch(group))
  }

  process(message: Bytes) {
    return this.#call((core) => core.process(message))
  }

  queueMessage(group: Bytes, body: OutgoingBody, sentAt: number) {
    return this.#call((core) => core.queueMessage(group, body, sentAt))
  }

  nextOutbound(group: Bytes) {
    return this.#call((core) => core.nextOutbound(group))
  }

  outboundSent(group: Bytes, sendId: Bytes): Promise<void> {
    return this.#call((core) => core.outboundSent(group, sendId))
  }

  outboundGroups() {
    return this.#call((core) => core.outboundGroups())
  }

  waitingMessages(group: Bytes) {
    return this.#call((core) => core.waitingMessages(group))
  }

  history(group: Bytes) {
    return this.#call(async (core) => {
      const store = this.#store
      if (store === undefined) throw new TesselError('host:not-started', 'no database')
      const records = await store.list(own(core.historyPartition(group)))
      return core.readHistory(group, records)
    })
  }

  encryptMedia(file: Bytes) {
    return this.#call((core) => core.encryptMedia(file))
  }

  decryptMedia(secret: Bytes, encrypted: Bytes): Promise<Bytes> {
    return this.#call((core) => own(core.decryptMedia(secret, encrypted)))
  }

  safetyNumber(contact: Bytes) {
    return this.#call((core) => core.safetyNumber(contact))
  }

  checkScanned(contact: Bytes, scanned: Bytes): Promise<void> {
    return this.#call((core) => core.checkScanned(contact, scanned))
  }

  markVerified(contact: Bytes): Promise<void> {
    return this.#call((core) => core.markVerified(contact))
  }

  unverify(account: string): Promise<void> {
    return this.#call((core) => core.unverify(account))
  }

  trust(device: Bytes): Promise<Trust> {
    return this.#call((core) => {
      const trust = core.trust(device)
      if (!isTrust(trust)) throw new TesselError('host:unknown-trust', trust)
      return trust
    })
  }

  acknowledgeDevice(device: Bytes): Promise<void> {
    return this.#call((core) => core.acknowledgeDevice(device))
  }
}
