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

/** What a client (web or mobile) can ask of the server. */
export interface TesselClient {
  ping(signal?: AbortSignal): Promise<Pong>
  /** Registers a new account with its first device's certificate (`TesselCore.createAccount`). */
  createAccount(certificate: Uint8Array, signal?: AbortSignal): Promise<AccountCreated>
}

export function createTesselClient(transport: RpcTransport, url = '/rpc'): TesselClient {
  const rpc = createRpcClient({ transport, url })
  return {
    ping: (signal) => rpc.call(Method.ping, {}, { parse: parsePong, signal }),
    createAccount: (certificate, signal) =>
      rpc.call(Method.createAccount, { certificate: toBase64Url(certificate) } satisfies CreateAccountParams, { parse: parseAccountCreated, signal }),
  }
}

/**
 * The `fetch` transport, for the browser and React Native: JSON POST to
 * `baseUrl` + path, empty response for a 204 (notification).
 */
export function fetchTransport(baseUrl: string, fetchImpl: typeof fetch = fetch): RpcTransport {
  return async (url, body, { signal }) => {
    const response = await fetchImpl(new URL(url, baseUrl), {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
      signal,
    })
    return response.status === 204 ? null : response.json()
  }
}
