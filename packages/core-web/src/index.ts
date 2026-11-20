import init, { initSync, version } from '../generated/tessel_wasm.js'

/** Ce que le cœur offre au client web. S'étoffe avec l'epic 02 (story 02-10). */
export interface TesselCore {
  version(): string
}

const core: TesselCore = { version }

/**
 * Charge le module WASM (dans le navigateur, depuis l'URL voisine du JS généré, ou
 * depuis `source` quand on la donne) et rend le cœur. Le web l'appellera dans un Web
 * Worker (story 12-01).
 */
export async function loadTesselCore(source?: URL | Response | BufferSource): Promise<TesselCore> {
  await init(source === undefined ? undefined : { module_or_path: source })
  return core
}

/** Variante synchrone, quand les octets du module sont déjà là (tests, worker). */
export function loadTesselCoreSync(bytes: BufferSource): TesselCore {
  initSync({ module: bytes })
  return core
}
