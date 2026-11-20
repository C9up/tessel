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
} as const

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

/** What a client (web or mobile) can ask of the server. */
export interface TesselClient {
  ping(signal?: AbortSignal): Promise<Pong>
}

export function createTesselClient(transport: RpcTransport, url = '/rpc'): TesselClient {
  const rpc = createRpcClient({ transport, url })
  return {
    ping: (signal) => rpc.call(Method.ping, {}, { parse: parsePong, signal }),
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
