import { createRpcClient, type RpcTransport } from '@c9up/comet'

/**
 * Contrat client-serveur de Tessel : les méthodes `comet` (JSON-RPC 2.0), leurs
 * paramètres et la validation de leurs résultats, déclarés une seule fois et
 * importés par le serveur, le web et le mobile.
 *
 * Rien de ce qui passe ici n'est un contenu de conversation en clair : le serveur
 * ne reçoit que des enveloppes chiffrées par `tessel-core`.
 */

/** Version du protocole. Elle monte quand un client ancien ne peut plus parler au serveur. */
export const PROTOCOL_VERSION = 1

/** Les noms des méthodes, `<module>.<verbe>`. */
export const Method = {
  ping: 'system.ping',
} as const

/** Réponse de `system.ping` : le serveur est là, et dit quel protocole il parle. */
export interface Pong {
  readonly pong: true
  readonly protocol: number
}

/** Un résultat que le client ne peut pas accepter. */
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

/** Ce qu'un client (web ou mobile) peut demander au serveur. */
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
 * Le transport par `fetch`, pour le navigateur et React Native : POST JSON vers
 * `baseUrl` + chemin, réponse vide pour un 204 (notification).
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
