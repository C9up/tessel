import { createRpcClient, type RpcTransport } from '@c9up/comet'

/**
 * Tessel's client-server contract: the `comet` methods (JSON-RPC 2.0), their
 * parameters and the validation of their results, declared once and imported by the
 * server, the web and mobile.
 *
 * Nothing that passes through here is conversation content in clear: the server
 * only receives envelopes encrypted by `tessel-core`.
 */

/** Protocol version. It goes up when an old client can no longer talk to the server. */
export const PROTOCOL_VERSION = 1

/** The method names, `<module>.<verb>`. */
export const Method = {
  ping: 'system.ping',
  createAccount: 'identity.createAccount',
  challenge: 'identity.challenge',
  signIn: 'identity.signIn',
  session: 'identity.session',
  signOut: 'identity.signOut',
  publishKeyPackages: 'identity.publishKeyPackages',
  keyPackageStock: 'identity.keyPackageStock',
  claimKeyPackages: 'identity.claimKeyPackages',
  linkDevice: 'identity.linkDevice',
  takeLinkGrant: 'identity.takeLinkGrant',
  devices: 'identity.devices',
  revokeDevice: 'identity.revokeDevice',
  setProfile: 'identity.setProfile',
  profile: 'identity.profile',
  deleteAccount: 'identity.deleteAccount',
  send: 'delivery.send',
  fetch: 'delivery.fetch',
} as const

/**
 * What the server refuses with, beyond JSON-RPC's own errors: the error's `code` is the
 * number, its `message` the stable name, for the client to translate.
 */
export const ServerError = {
  /** The parameters are not what the method takes. */
  E_INVALID_PARAMS: -32602,
  /** The certificate does not decode, or its account key's signature does not verify. */
  E_IDENTITY_INVALID_CERTIFICATE: -32602,
  /** The account belongs to another instance's domain. */
  E_IDENTITY_WRONG_DOMAIN: -32602,
  /** Another account has that name. */
  E_IDENTITY_NAME_TAKEN: -32009,
  /**
   * The sign-in proof is refused: the challenge expired or was used, the device is
   * unknown or revoked, or the signature does not verify. Which one, the server does not
   * say.
   */
  E_IDENTITY_SIGN_IN_REFUSED: -32001,
  /** The call needs a session, and none is valid: missing, expired, closed, or its device revoked. */
  E_UNAUTHENTICATED: -32001,
  /**
   * A KeyPackage is not valid, no longer valid, of the wrong kind, or not this device's
   * own; or the upload is too large. Nothing of the upload was kept.
   */
  E_IDENTITY_INVALID_KEY_PACKAGE: -32602,
  /** The device already keeps as many KeyPackages as the directory takes. */
  E_IDENTITY_KEY_PACKAGE_STOCK_FULL: -32010,
  /** The account already has as many devices as it may: revoke one first. */
  E_IDENTITY_TOO_MANY_DEVICES: -32011,
  /** The link already holds a grant: the new device shows a new QR code. */
  E_IDENTITY_LINK_USED: -32012,
  /** No live device of the caller's account has that identifier. */
  E_IDENTITY_UNKNOWN_DEVICE: -32013,
  /** Revoking another device takes the account key's signature, and this one does not verify. */
  E_IDENTITY_INVALID_REVOCATION: -32602,
  /** Not a sealed profile: unknown version, or too large. */
  E_IDENTITY_INVALID_PROFILE: -32602,
  /** Deleting the account takes the account key's signature, and this one does not verify. */
  E_IDENTITY_INVALID_DELETION: -32602,
  /** Not a group's MLS message, empty, or too large. */
  E_DELIVERY_INVALID_ENVELOPE: -32602,
  /** An envelope for nobody, the sender left aside. */
  E_DELIVERY_NO_RECIPIENT: -32602,
  /**
   * The group has left the envelope's epoch: another commit won it, or the device is
   * behind. A commit: `TesselCore.commitRejected`, fetch and process what came, then
   * `TesselCore.retryCommit`. A message: fetch and process, then send it again (the core
   * encrypts it anew).
   */
  E_DELIVERY_STALE_EPOCH: -32014,
  /** Too many calls from this network: wait, then try again. */
  E_TOO_MANY_REQUESTS: -32029,
} as const

export type ServerErrorName = keyof typeof ServerError

/** Bytes as they travel in JSON: base64url, without padding. */
export function toBase64Url(bytes: Uint8Array): string {
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '')
}

/** The bytes of a base64url string; `undefined` when it is not one. */
export function fromBase64Url(text: string): Uint8Array<ArrayBuffer> | undefined {
  if (!/^[A-Za-z0-9_-]*$/.test(text) || text.length % 4 === 1) return undefined
  const binary = atob(text.replaceAll('-', '+').replaceAll('_', '/'))
  return Uint8Array.from(binary, (char) => char.charCodeAt(0))
}

/** Parameters of `identity.createAccount`: the first device's certificate, signed by the new account key. */
export interface CreateAccountParams {
  /** base64url. */
  readonly certificate: string
}

/** Response of `identity.createAccount`. */
export interface AccountCreated {
  /** `name@domain`. */
  readonly account: string
}

/** Response of `identity.challenge`: what the device signs, valid for a minute, once. */
export interface Challenge {
  /** base64url of 32 random bytes. */
  readonly challenge: string
}

/** Parameters of `identity.signIn`: the device, and its signature of the challenge (`TesselCore.signIn`). */
export interface SignInParams {
  /** base64url of the 16-byte device identifier. */
  readonly device: string
  /** base64url, as `identity.challenge` gave it. */
  readonly challenge: string
  /** base64url. */
  readonly proof: string
  /** What the device is, for its account's device list. */
  readonly kind: DeviceKind
}

/** What a device is: the server learns it anyway from the push service it registers with. */
export type DeviceKind = 'web' | 'android' | 'ios'

/** A live device of the account, as `identity.devices` lists it. */
export interface DeviceEntry {
  /** base64url of the device identifier. */
  readonly device: string
  /** Absent until it signs in once. */
  readonly kind?: DeviceKind
  /** `YYYY-MM-DD`: the day only. */
  readonly createdOn: string
  /** `YYYY-MM-DD`: the day only; absent until it signs in once. */
  readonly lastActiveOn?: string
  /** The device asking. */
  readonly current: boolean
}

/** Response of `identity.devices`. */
export interface DeviceList {
  readonly devices: readonly DeviceEntry[]
}

/** Parameters of `identity.revokeDevice`. */
export interface RevokeDeviceParams {
  /** base64url of the device identifier. */
  readonly device: string
  /** base64url of `TesselCore.revokeDevice`; not needed for the calling device itself. */
  readonly revocation?: string
}

/** Response of `identity.revokeDevice`. */
export interface DeviceRevoked {
  readonly revoked: true
}

/** Response of `identity.signIn`. */
export interface SignedIn {
  /**
   * The session token, sent back as `Authorization: Bearer <token>`. It is bound to the
   * device, and lasts until it expires, the device signs out, or the device is revoked.
   */
  readonly token: string
}

/** Response of `identity.session`: who the session is. */
export interface Session {
  /** `name@domain`. */
  readonly account: string
  /** base64url of the device identifier. */
  readonly device: string
}

/** Response of `identity.signOut`. */
export interface SignedOut {
  readonly signedOut: true
}

/** Parameters of `identity.publishKeyPackages`: `TesselCore.refreshKeyPackages`, base64url. */
export interface PublishKeyPackagesParams {
  readonly publish: readonly string[]
  readonly lastResort?: string
  /** References of the KeyPackages to withdraw. */
  readonly withdraw: readonly string[]
}

/** Response of `identity.publishKeyPackages` and `identity.keyPackageStock`. */
export interface KeyPackageStock {
  /** Single-use KeyPackages the device has left in the directory. */
  readonly stock: number
  /** Whether the device should renew its pool now. */
  readonly low: boolean
}

/** Parameters of `identity.claimKeyPackages`. */
export interface ClaimKeyPackagesParams {
  /** `name@domain`. */
  readonly account: string
}

/** A KeyPackage handed out for one device. */
export interface ClaimedKeyPackage {
  /** base64url of the device identifier. */
  readonly device: string
  /** base64url; for `TesselCore.addDevices`, after `TesselCore.verifyKeyPackage`. */
  readonly keyPackage: string
}

/** Response of `identity.claimKeyPackages`: one KeyPackage per device of the account, none for an unknown one. */
export interface ClaimedKeyPackages {
  readonly keyPackages: readonly ClaimedKeyPackage[]
}

/** Parameters of `identity.linkDevice`: `TesselCore.grantLink`, base64url. */
export interface LinkDeviceParams {
  readonly certificate: string
  readonly link: string
  readonly grant: string
}

/** Response of `identity.linkDevice`. */
export interface DeviceLinked {
  readonly linked: true
}

/** Parameters of `identity.takeLinkGrant`. */
export interface TakeLinkGrantParams {
  /** base64url. */
  readonly link: string
}

/** Response of `identity.takeLinkGrant`: the grant, once; nothing while it has not come. */
export interface LinkGrantTaken {
  /** base64url, for `TesselCore.completeLink`. */
  readonly grant?: string
}

/** Response of `system.ping`: the server is there, and says which protocol it speaks. */
export interface Pong {
  readonly pong: true
  readonly protocol: number
}

/** A result the client cannot accept. */
export class ProtocolError extends Error {
  readonly code: 'E_PROTOCOL_BAD_RESULT'
  readonly method: string

  constructor(code: 'E_PROTOCOL_BAD_RESULT', method: string) {
    super(`${code}: ${method}`)
    this.name = 'ProtocolError'
    this.code = code
    this.method = method
  }
}

export function parsePong(data: unknown): Pong {
  if (typeof data === 'object' && data !== null && 'pong' in data && data.pong === true && 'protocol' in data && typeof data.protocol === 'number') {
    return { pong: true, protocol: data.protocol }
  }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.ping)
}

export function parseAccountCreated(data: unknown): AccountCreated {
  if (typeof data === 'object' && data !== null && 'account' in data && typeof data.account === 'string') {
    return { account: data.account }
  }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.createAccount)
}

const isRecord = (data: unknown): data is Record<string, unknown> => typeof data === 'object' && data !== null

export function parseChallenge(data: unknown): Challenge {
  if (isRecord(data) && typeof data.challenge === 'string' && fromBase64Url(data.challenge)?.length === 32) return { challenge: data.challenge }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.challenge)
}

export function parseSignedIn(data: unknown): SignedIn {
  if (isRecord(data) && typeof data.token === 'string' && data.token.length > 0) return { token: data.token }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.signIn)
}

export function parseSession(data: unknown): Session {
  if (isRecord(data) && typeof data.account === 'string' && typeof data.device === 'string') return { account: data.account, device: data.device }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.session)
}

export function parseSignedOut(data: unknown): SignedOut {
  if (isRecord(data) && data.signedOut === true) return { signedOut: true }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.signOut)
}

export function parseKeyPackageStock(data: unknown): KeyPackageStock {
  if (isRecord(data) && typeof data.stock === 'number' && typeof data.low === 'boolean') return { stock: data.stock, low: data.low }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.keyPackageStock)
}

export function parseClaimedKeyPackages(data: unknown): ClaimedKeyPackages {
  if (isRecord(data) && Array.isArray(data.keyPackages)) {
    const keyPackages: ClaimedKeyPackage[] = []
    for (const entry of data.keyPackages) {
      if (!isRecord(entry) || typeof entry.device !== 'string' || typeof entry.keyPackage !== 'string') break
      keyPackages.push({ device: entry.device, keyPackage: entry.keyPackage })
    }
    if (keyPackages.length === data.keyPackages.length) return { keyPackages }
  }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.claimKeyPackages)
}

const isDeviceKind = (value: unknown): value is DeviceKind => value === 'web' || value === 'android' || value === 'ios'
const isDay = (value: unknown): value is string => typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)

function deviceEntry(data: unknown): DeviceEntry | undefined {
  if (!isRecord(data) || typeof data.device !== 'string' || !isDay(data.createdOn) || typeof data.current !== 'boolean') return undefined
  if (data.kind !== undefined && !isDeviceKind(data.kind)) return undefined
  if (data.lastActiveOn !== undefined && !isDay(data.lastActiveOn)) return undefined
  return {
    device: data.device,
    createdOn: data.createdOn,
    current: data.current,
    ...(isDeviceKind(data.kind) ? { kind: data.kind } : {}),
    ...(isDay(data.lastActiveOn) ? { lastActiveOn: data.lastActiveOn } : {}),
  }
}

export function parseDeviceList(data: unknown): DeviceList {
  if (isRecord(data) && Array.isArray(data.devices)) {
    const devices = data.devices.map(deviceEntry).filter((entry) => entry !== undefined)
    if (devices.length === data.devices.length) return { devices }
  }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.devices)
}

/** Parameters of `identity.setProfile`: `TesselCore.sealProfile`, base64url. */
export interface SetProfileParams {
  readonly blob: string
}

/** Response of `identity.setProfile`. */
export interface ProfileSet {
  readonly saved: true
}

/** Parameters of `identity.profile`. */
export interface ProfileParams {
  /** `name@domain`. */
  readonly account: string
}

/** Response of `identity.profile`: the sealed blob, for `TesselCore.openProfile`; nothing when the account has none. */
export interface SealedProfile {
  readonly blob?: string
}

/** Parameters of `identity.deleteAccount`: `TesselCore.signAccountDeletion`, base64url. */
export interface DeleteAccountParams {
  readonly deletion: string
}

/** Response of `identity.deleteAccount`. */
export interface AccountDeleted {
  readonly deleted: true
}

/** Parameters of `delivery.send`: an envelope from the core's send queue (`TesselCore.nextOutbound`). */
export interface SendParams {
  /** base64url of the 16-byte send identifier: the same envelope sent again keeps it. */
  readonly sendId: string
  /** base64url of the MLS message. */
  readonly envelope: string
  /** base64url of each device it is for (`TesselCore.recipients`). */
  readonly recipients: readonly string[]
}

/** Response of `delivery.send`: the server kept it (or had already). */
export interface EnvelopeSent {
  /** The server's identifier of the envelope. */
  readonly envelope: string
  /** Its place in its group; absent for a *Welcome*. For a commit, it means accepted: `TesselCore.commitAccepted`. */
  readonly seq?: number
}

/** Parameters of `delivery.fetch`. */
export interface FetchParams {
  /** The last envelope identifier already handled; absent the first time. */
  readonly after?: string
}

/** An envelope for this device. */
export interface ReceivedEnvelope {
  /** The server's identifier: envelopes come in its order. */
  readonly id: string
  /** Its place in its group, without gaps; absent for a *Welcome*. */
  readonly seq?: number
  /** base64url of the MLS message, for `TesselCore.process` or `TesselCore.joinGroup`. */
  readonly envelope: string
}

/** Response of `delivery.fetch`: the next envelopes, oldest first. */
export interface Envelopes {
  readonly envelopes: readonly ReceivedEnvelope[]
}

export function parseEnvelopeSent(data: unknown): EnvelopeSent {
  if (isRecord(data) && typeof data.envelope === 'string' && /^\d+$/.test(data.envelope)) {
    if (data.seq === undefined) return { envelope: data.envelope }
    if (isSeq(data.seq)) return { envelope: data.envelope, seq: data.seq }
  }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.send)
}

const isSeq = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value > 0

export function parseEnvelopes(data: unknown): Envelopes {
  if (isRecord(data) && Array.isArray(data.envelopes)) {
    const envelopes: ReceivedEnvelope[] = []
    for (const entry of data.envelopes) {
      if (!isRecord(entry) || typeof entry.id !== 'string' || !/^\d+$/.test(entry.id) || typeof entry.envelope !== 'string') break
      if (entry.seq !== undefined && !isSeq(entry.seq)) break
      envelopes.push(isSeq(entry.seq) ? { id: entry.id, seq: entry.seq, envelope: entry.envelope } : { id: entry.id, envelope: entry.envelope })
    }
    if (envelopes.length === data.envelopes.length) return { envelopes }
  }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.fetch)
}

export function parseAccountDeleted(data: unknown): AccountDeleted {
  if (isRecord(data) && data.deleted === true) return { deleted: true }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.deleteAccount)
}

export function parseProfileSet(data: unknown): ProfileSet {
  if (isRecord(data) && data.saved === true) return { saved: true }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.setProfile)
}

export function parseSealedProfile(data: unknown): SealedProfile {
  if (isRecord(data) && data.blob === undefined) return {}
  if (isRecord(data) && typeof data.blob === 'string' && fromBase64Url(data.blob) !== undefined) return { blob: data.blob }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.profile)
}

export function parseDeviceRevoked(data: unknown): DeviceRevoked {
  if (isRecord(data) && data.revoked === true) return { revoked: true }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.revokeDevice)
}

export function parseDeviceLinked(data: unknown): DeviceLinked {
  if (isRecord(data) && data.linked === true) return { linked: true }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.linkDevice)
}

export function parseLinkGrantTaken(data: unknown): LinkGrantTaken {
  if (isRecord(data) && data.grant === undefined) return {}
  if (isRecord(data) && typeof data.grant === 'string' && fromBase64Url(data.grant) !== undefined) return { grant: data.grant }
  throw new ProtocolError('E_PROTOCOL_BAD_RESULT', Method.takeLinkGrant)
}

/** What a device sends the directory: `TesselCore.refreshKeyPackages`. */
export interface KeyPackageUpdate {
  readonly publish: readonly Uint8Array[]
  readonly lastResort?: Uint8Array
  readonly withdraw: readonly Uint8Array[]
}

/** What a client (web or mobile) can ask of the server. */
export interface TesselClient {
  ping(signal?: AbortSignal): Promise<Pong>
  /** Registers a new account with its first device's certificate (`TesselCore.createAccount`). */
  createAccount(certificate: Uint8Array, signal?: AbortSignal): Promise<AccountCreated>
  /** A fresh sign-in challenge, as bytes, for `TesselCore.signIn`. */
  challenge(signal?: AbortSignal): Promise<Uint8Array<ArrayBuffer>>
  /** Opens a session for `device`, with its signature of `challenge`. */
  signIn(device: Uint8Array, challenge: Uint8Array, proof: Uint8Array, kind: DeviceKind, signal?: AbortSignal): Promise<SignedIn>
  /** Who the transport's session is. */
  session(signal?: AbortSignal): Promise<Session>
  /** Closes the transport's session. */
  signOut(signal?: AbortSignal): Promise<SignedOut>
  /** Leaves this device's renewed KeyPackages in the directory, all or nothing. */
  publishKeyPackages(update: KeyPackageUpdate, signal?: AbortSignal): Promise<KeyPackageStock>
  /** This device's stock in the directory. */
  keyPackageStock(signal?: AbortSignal): Promise<KeyPackageStock>
  /** One KeyPackage per device of `account`, this device left out: each is handed out once. */
  claimKeyPackages(account: string, signal?: AbortSignal): Promise<ClaimedKeyPackages>
  /** On the device holding the account key: registers the scanned device and posts its grant (`TesselCore.grantLink`). */
  linkDevice(certificate: Uint8Array, link: Uint8Array, grant: Uint8Array, signal?: AbortSignal): Promise<DeviceLinked>
  /** On the new device, without a session: its grant, once; `undefined` while it has not come. */
  takeLinkGrant(link: Uint8Array, signal?: AbortSignal): Promise<Uint8Array<ArrayBuffer> | undefined>
  /** The account's live devices. */
  devices(signal?: AbortSignal): Promise<DeviceList>
  /** Revokes a device of the account: itself without `revocation`, another one with `TesselCore.revokeDevice`. */
  revokeDevice(device: Uint8Array, revocation?: Uint8Array, signal?: AbortSignal): Promise<DeviceRevoked>
  /** Leaves this account's sealed profile (`TesselCore.sealProfile`); then share the key in each group. */
  setProfile(blob: Uint8Array, signal?: AbortSignal): Promise<ProfileSet>
  /** The sealed profile of `account`; `undefined` when it has none. */
  profile(account: string, signal?: AbortSignal): Promise<Uint8Array<ArrayBuffer> | undefined>
  /** Deletes the account for good (`TesselCore.signAccountDeletion`), once its devices left their groups. */
  deleteAccount(deletion: Uint8Array, signal?: AbortSignal): Promise<AccountDeleted>
  /** Posts an envelope for `recipients`; sent again with the same `sendId`, it is kept once. */
  send(sendId: Uint8Array, envelope: Uint8Array, recipients: readonly Uint8Array[], signal?: AbortSignal): Promise<EnvelopeSent>
  /** The envelopes for this device after `after`, oldest first. */
  fetch(after?: string, signal?: AbortSignal): Promise<Envelopes>
}

export function createTesselClient(transport: RpcTransport, url = '/rpc'): TesselClient {
  const rpc = createRpcClient({ transport, url })
  return {
    ping: (signal) => rpc.call(Method.ping, {}, { parse: parsePong, signal }),
    createAccount: (certificate, signal) =>
      rpc.call(Method.createAccount, { certificate: toBase64Url(certificate) } satisfies CreateAccountParams, { parse: parseAccountCreated, signal }),
    challenge: async (signal) => {
      const { challenge } = await rpc.call(Method.challenge, {}, { parse: parseChallenge, signal })
      // parseChallenge checked that it decodes.
      return fromBase64Url(challenge) ?? new Uint8Array()
    },
    signIn: (device, challenge, proof, kind, signal) =>
      rpc.call(Method.signIn, { device: toBase64Url(device), challenge: toBase64Url(challenge), proof: toBase64Url(proof), kind } satisfies SignInParams, {
        parse: parseSignedIn,
        signal,
      }),
    session: (signal) => rpc.call(Method.session, {}, { parse: parseSession, signal }),
    signOut: (signal) => rpc.call(Method.signOut, {}, { parse: parseSignedOut, signal }),
    publishKeyPackages: (update, signal) => {
      const params: PublishKeyPackagesParams = {
        publish: update.publish.map(toBase64Url),
        withdraw: update.withdraw.map(toBase64Url),
        ...(update.lastResort === undefined ? {} : { lastResort: toBase64Url(update.lastResort) }),
      }
      return rpc.call(Method.publishKeyPackages, params, { parse: parseKeyPackageStock, signal })
    },
    keyPackageStock: (signal) => rpc.call(Method.keyPackageStock, {}, { parse: parseKeyPackageStock, signal }),
    claimKeyPackages: (account, signal) =>
      rpc.call(Method.claimKeyPackages, { account } satisfies ClaimKeyPackagesParams, { parse: parseClaimedKeyPackages, signal }),
    linkDevice: (certificate, link, grant, signal) =>
      rpc.call(Method.linkDevice, { certificate: toBase64Url(certificate), link: toBase64Url(link), grant: toBase64Url(grant) } satisfies LinkDeviceParams, {
        parse: parseDeviceLinked,
        signal,
      }),
    devices: (signal) => rpc.call(Method.devices, {}, { parse: parseDeviceList, signal }),
    setProfile: (blob, signal) => rpc.call(Method.setProfile, { blob: toBase64Url(blob) } satisfies SetProfileParams, { parse: parseProfileSet, signal }),
    profile: async (account, signal) => {
      const { blob } = await rpc.call(Method.profile, { account } satisfies ProfileParams, { parse: parseSealedProfile, signal })
      return blob === undefined ? undefined : fromBase64Url(blob)
    },
    send: (sendId, envelope, recipients, signal) =>
      rpc.call(Method.send, { sendId: toBase64Url(sendId), envelope: toBase64Url(envelope), recipients: recipients.map(toBase64Url) } satisfies SendParams, {
        parse: parseEnvelopeSent,
        signal,
      }),
    fetch: (after, signal) => rpc.call(Method.fetch, (after === undefined ? {} : { after }) satisfies FetchParams, { parse: parseEnvelopes, signal }),
    deleteAccount: (deletion, signal) =>
      rpc.call(Method.deleteAccount, { deletion: toBase64Url(deletion) } satisfies DeleteAccountParams, { parse: parseAccountDeleted, signal }),
    revokeDevice: (device, revocation, signal) => {
      const params: RevokeDeviceParams =
        revocation === undefined ? { device: toBase64Url(device) } : { device: toBase64Url(device), revocation: toBase64Url(revocation) }
      return rpc.call(Method.revokeDevice, params, { parse: parseDeviceRevoked, signal })
    },
    takeLinkGrant: async (link, signal) => {
      const { grant } = await rpc.call(Method.takeLinkGrant, { link: toBase64Url(link) } satisfies TakeLinkGrantParams, { parse: parseLinkGrantTaken, signal })
      return grant === undefined ? undefined : fromBase64Url(grant)
    },
  }
}

/** Options of {@link fetchTransport}. */
export interface FetchTransportOptions {
  fetch?: typeof fetch
  /** The session token to send, read before each call: `undefined` while signed out. */
  token?: () => string | undefined
}

/**
 * The `fetch` transport, for the browser and React Native: JSON POST to
 * `baseUrl` + path, empty response for a 204 (notification). The session token, when
 * there is one, goes in `Authorization`, never in the body.
 */
export function fetchTransport(baseUrl: string, options: FetchTransportOptions = {}): RpcTransport {
  const fetchImpl = options.fetch ?? fetch
  return async (url, body, { signal }) => {
    const headers: Record<string, string> = { 'content-type': 'application/json' }
    const token = options.token?.()
    if (token !== undefined) headers.authorization = `Bearer ${token}`
    const response = await fetchImpl(new URL(url, baseUrl), { method: 'POST', headers, body: JSON.stringify(body), signal })
    return response.status === 204 ? null : response.json()
  }
}
