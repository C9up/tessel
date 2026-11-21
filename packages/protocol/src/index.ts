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

/** What a client (web or mobile) can ask of the server. */
export interface TesselClient {
  ping(signal?: AbortSignal): Promise<Pong>
  /** Registers a new account with its first device's certificate (`TesselCore.createAccount`). */
  createAccount(certificate: Uint8Array, signal?: AbortSignal): Promise<AccountCreated>
  /** A fresh sign-in challenge, as bytes, for `TesselCore.signIn`. */
  challenge(signal?: AbortSignal): Promise<Uint8Array<ArrayBuffer>>
  /** Opens a session for `device`, with its signature of `challenge`. */
  signIn(device: Uint8Array, challenge: Uint8Array, proof: Uint8Array, signal?: AbortSignal): Promise<SignedIn>
  /** Who the transport's session is. */
  session(signal?: AbortSignal): Promise<Session>
  /** Closes the transport's session. */
  signOut(signal?: AbortSignal): Promise<SignedOut>
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
    signIn: (device, challenge, proof, signal) =>
      rpc.call(Method.signIn, { device: toBase64Url(device), challenge: toBase64Url(challenge), proof: toBase64Url(proof) } satisfies SignInParams, {
        parse: parseSignedIn,
        signal,
      }),
    session: (signal) => rpc.call(Method.session, {}, { parse: parseSession, signal }),
    signOut: (signal) => rpc.call(Method.signOut, {}, { parse: parseSignedOut, signal }),
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
