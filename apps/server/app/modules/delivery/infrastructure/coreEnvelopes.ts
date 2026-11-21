import { readEnvelope } from '@tessel/core-web/server'
import type { EnvelopeHeader } from '../domain/envelope.js'
import type { EnvelopeReader } from '../domain/ports.js'

/** The header reader of `tessel-core`, through its WASM module: the server parses no MLS of its own. */
export const coreEnvelopes: EnvelopeReader = {
  read(body: Uint8Array): EnvelopeHeader | null {
    return readEnvelope(body) ?? null
  },
}
