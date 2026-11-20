import init, { ciphersuite, generateStoreKey, initSync, selfCheck, version } from '../generated/tessel_wasm.js'

export { type OutboundMessage, type Outbox, OutboxSender, type OutboxSenderOptions, RETRY_DELAYS, type Relay } from './outbox.ts'
export {
  type Bytes,
  INDEXEDDB_SCHEMA_VERSION,
  IndexedDbStore,
  loadStoreKey,
  type StoreChanges,
  type StoredRecord,
  UnsupportedSchemaError,
} from './store.ts'

/** What the core offers to the web client. Grows with epic 02 (story 02-10). */
export interface TesselCore {
  version(): string
  /** IANA name of the MLS ciphersuite (ADR-001). */
  ciphersuite(): string
  /** Draws a new key for the local database, to seal at once (`loadStoreKey`). */
  generateStoreKey(): Uint8Array
}

const core: TesselCore = { version, ciphersuite, generateStoreKey }

/**
 * Loads the WASM module (in the browser, from the URL next to the generated JS, or
 * from `source` when given) and returns the core, after checking that it can work on
 * this host (ciphersuite supported, randomness available). The web will call it in a
 * Web Worker (story 12-01).
 */
export async function loadTesselCore(source?: URL | Response | BufferSource): Promise<TesselCore> {
  await init(source === undefined ? undefined : { module_or_path: source })
  selfCheck()
  return core
}

/** Synchronous variant, when the module's bytes are already at hand (tests, worker). */
export function loadTesselCoreSync(bytes: BufferSource): TesselCore {
  initSync({ module: bytes })
  selfCheck()
  return core
}
