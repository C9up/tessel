/**
 * The page talks to the core in its Web Worker through messages (story 02-10): `serve`
 * answers in the worker, `connect` gives the page a `TesselCore` that forwards every
 * call. Arguments and results are plain objects, copied by the structured clone.
 */

import { METHODS, type Profile, type TesselCore, TesselError } from './api.ts'
import type { Bytes } from './store.ts'

/** A worker, the worker's global scope, or a `MessagePort`. */
export interface Endpoint {
  postMessage(message: unknown): void
  addEventListener(type: 'message', listener: (event: MessageEvent) => void): void
  removeEventListener(type: 'message', listener: (event: MessageEvent) => void): void
  /** A `MessagePort` only delivers once started. */
  start?(): void
}

type Method = (typeof METHODS)[number]

interface Request {
  id: number
  method: string
  args: unknown[]
}

type Response = { id: number; ok: true; value: unknown } | { id: number; ok: false; code: string; message: string }

const METHOD_NAMES: ReadonlySet<string> = new Set(METHODS)

function isMethod(name: string): name is Method {
  return METHOD_NAMES.has(name)
}

function isRequest(data: unknown): data is Request {
  return (
    typeof data === 'object' &&
    data !== null &&
    'id' in data &&
    typeof data.id === 'number' &&
    'method' in data &&
    typeof data.method === 'string' &&
    'args' in data &&
    Array.isArray(data.args)
  )
}

function isResponse(data: unknown): data is Response {
  return typeof data === 'object' && data !== null && 'id' in data && typeof data.id === 'number' && 'ok' in data && typeof data.ok === 'boolean'
}

/** Answers the calls arriving on `endpoint`; returns what stops answering. */
export function serve(endpoint: Endpoint, core: TesselCore | Promise<TesselCore>): () => void {
  // Listening at once: a message posted before the core is ready waits for it.
  const ready = Promise.resolve(core)
  const listener = (event: MessageEvent) => {
    const request: unknown = event.data
    if (!isRequest(request)) return
    const { id, method, args } = request
    ready
      .then((core) => {
        if (!isMethod(method)) throw new TesselError('host:unknown-method', `no method ${method}`)
        return Reflect.apply(core[method], core, args)
      })
      .then(
        (value: unknown) => endpoint.postMessage({ id, ok: true, value } satisfies Response),
        (error: unknown) => {
          const { code, message } = TesselError.from(error)
          endpoint.postMessage({ id, ok: false, code, message } satisfies Response)
        },
      )
  }
  endpoint.addEventListener('message', listener)
  endpoint.start?.()
  return () => endpoint.removeEventListener('message', listener)
}

type Result<K extends Method> = Awaited<ReturnType<TesselCore[K]>>

/** The core, in the worker at the other end of `endpoint`. */
export class RemoteCore implements TesselCore {
  readonly #endpoint: Endpoint
  readonly #pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: TesselError) => void }>()
  #next = 0
  readonly #listener = (event: MessageEvent) => {
    const response: unknown = event.data
    if (!isResponse(response)) return
    const pending = this.#pending.get(response.id)
    if (pending === undefined) return
    this.#pending.delete(response.id)
    if (response.ok) pending.resolve(response.value)
    else pending.reject(new TesselError(response.code, response.message))
  }

  constructor(endpoint: Endpoint) {
    this.#endpoint = endpoint
    endpoint.addEventListener('message', this.#listener)
    endpoint.start?.()
  }

  #call<K extends Method>(method: K, args: Parameters<TesselCore[K]>): Promise<Result<K>> {
    const id = this.#next++
    return new Promise((resolve, reject) => {
      // The worker answers with what the core returned for this very method.
      this.#pending.set(id, { resolve: (value) => resolve(value as Result<K>), reject })
      this.#endpoint.postMessage({ id, method, args } satisfies Request)
    })
  }

  /** Stops listening; the calls still waiting fail. */
  disconnect(): void {
    this.#endpoint.removeEventListener('message', this.#listener)
    for (const { reject } of this.#pending.values()) reject(new TesselError('host:disconnected', 'the core is no longer reachable'))
    this.#pending.clear()
  }

  version() {
    return this.#call('version', [])
  }
  ciphersuite() {
    return this.#call('ciphersuite', [])
  }
  start() {
    return this.#call('start', [])
  }
  createAccount(account: string) {
    return this.#call('createAccount', [account])
  }
  device() {
    return this.#call('device', [])
  }
  holdsAccountKey() {
    return this.#call('holdsAccountKey', [])
  }
  signIn(challenge: Bytes) {
    return this.#call('signIn', [challenge])
  }
  requestLink() {
    return this.#call('requestLink', [])
  }
  completeLink(grant: Bytes) {
    return this.#call('completeLink', [grant])
  }
  grantLink(qr: Bytes) {
    return this.#call('grantLink', [qr])
  }
  groups() {
    return this.#call('groups', [])
  }
  revokeDevice(deviceId: Bytes) {
    return this.#call('revokeDevice', [deviceId])
  }
  groupsWith(deviceId: Bytes) {
    return this.#call('groupsWith', [deviceId])
  }
  recipients(group: Bytes) {
    return this.#call('recipients', [group])
  }
  conversations() {
    return this.#call('conversations', [])
  }
  markRead(group: Bytes) {
    return this.#call('markRead', [group])
  }
  signAccountDeletion() {
    return this.#call('signAccountDeletion', [])
  }
  sealProfile(profile: Profile) {
    return this.#call('sealProfile', [profile])
  }
  openProfile(account: string, blob: Bytes) {
    return this.#call('openProfile', [account, blob])
  }
  knowsProfile(account: string) {
    return this.#call('knowsProfile', [account])
  }
  shareProfile(group: Bytes, sentAt: number) {
    return this.#call('shareProfile', [group, sentAt])
  }
  close() {
    return this.#call('close', [])
  }
  refreshKeyPackages(now: number) {
    return this.#call('refreshKeyPackages', [now])
  }
  verifyKeyPackage(keyPackage: Bytes) {
    return this.#call('verifyKeyPackage', [keyPackage])
  }
  createGroup() {
    return this.#call('createGroup', [])
  }
  addDevices(...args: Parameters<TesselCore['addDevices']>) {
    return this.#call('addDevices', args)
  }
  removeDevices(...args: Parameters<TesselCore['removeDevices']>) {
    return this.#call('removeDevices', args)
  }
  commitPendingProposals(group: Bytes) {
    return this.#call('commitPendingProposals', [group])
  }
  commitAccepted(group: Bytes) {
    return this.#call('commitAccepted', [group])
  }
  commitRejected(group: Bytes) {
    return this.#call('commitRejected', [group])
  }
  retryCommit(group: Bytes) {
    return this.#call('retryCommit', [group])
  }
  leaveGroup(group: Bytes) {
    return this.#call('leaveGroup', [group])
  }
  joinGroup(welcome: Bytes) {
    return this.#call('joinGroup', [welcome])
  }
  members(group: Bytes) {
    return this.#call('members', [group])
  }
  epoch(group: Bytes) {
    return this.#call('epoch', [group])
  }
  process(message: Bytes) {
    return this.#call('process', [message])
  }
  queueMessage(...args: Parameters<TesselCore['queueMessage']>) {
    return this.#call('queueMessage', args)
  }
  nextOutbound(group: Bytes) {
    return this.#call('nextOutbound', [group])
  }
  outboundSent(group: Bytes, sendId: Bytes) {
    return this.#call('outboundSent', [group, sendId])
  }
  outboundGroups() {
    return this.#call('outboundGroups', [])
  }
  waitingMessages(group: Bytes) {
    return this.#call('waitingMessages', [group])
  }
  history(group: Bytes) {
    return this.#call('history', [group])
  }
  encryptMedia(file: Bytes) {
    return this.#call('encryptMedia', [file])
  }
  decryptMedia(secret: Bytes, encrypted: Bytes) {
    return this.#call('decryptMedia', [secret, encrypted])
  }
  safetyNumber(contact: Bytes) {
    return this.#call('safetyNumber', [contact])
  }
  checkScanned(contact: Bytes, scanned: Bytes) {
    return this.#call('checkScanned', [contact, scanned])
  }
  markVerified(contact: Bytes) {
    return this.#call('markVerified', [contact])
  }
  unverify(account: string) {
    return this.#call('unverify', [account])
  }
  trust(device: Bytes) {
    return this.#call('trust', [device])
  }
  acknowledgeDevice(device: Bytes) {
    return this.#call('acknowledgeDevice', [device])
  }
}
