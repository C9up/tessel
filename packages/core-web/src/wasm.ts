/**
 * Loading the WASM module. Only the crypto worker loads it (story 12-01); the page talks
 * to the core through `connect` (`rpc.ts`).
 */

import init, { generateStoreKey, initSync, selfCheck } from '../generated/tessel_wasm.js'

export { generateStoreKey }

/**
 * Loads the module (in the browser, from the URL next to the generated JS, or from
 * `source`), then checks the core can work on this host (ciphersuite supported,
 * randomness available).
 */
export async function loadWasm(source?: URL | Response | BufferSource): Promise<void> {
  await init(source === undefined ? undefined : { module_or_path: source })
  selfCheck()
}

/** Synchronous variant, when the module's bytes are already at hand (tests). */
export function loadWasmSync(bytes: BufferSource): void {
  initSync({ module: bytes })
  selfCheck()
}
