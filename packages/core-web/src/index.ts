import init, { ciphersuite, initSync, selfCheck, version } from '../generated/tessel_wasm.js'

/** What the core offers to the web client. Grows with epic 02 (story 02-10). */
export interface TesselCore {
  version(): string
  /** IANA name of the MLS ciphersuite (ADR-001). */
  ciphersuite(): string
}

const core: TesselCore = { version, ciphersuite }

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
