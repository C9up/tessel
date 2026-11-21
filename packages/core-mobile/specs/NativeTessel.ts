import type { TurboModule } from 'react-native'
import { TurboModuleRegistry } from 'react-native'

/**
 * tessel-core on the phone (story 02-13), over JSI: one call per core operation, its
 * arguments and its answer in MessagePack (bytes stay bytes), in ArrayBuffers that cross
 * without a copy into JSON. The operations are those of `TesselCore`
 * (`@tessel/core-web/api`); `@tessel/core-mobile` gives them that very interface.
 */
export interface Spec extends TurboModule {
  /**
   * Runs `method` with its MessagePack-encoded arguments (a Uint8Array); its MessagePack
   * answer (an ArrayBuffer), or throws an Error whose message is "<code>\n<message>".
   * `Object` because React Native's codegen knows no ArrayBuffer type.
   */
  // biome-ignore lint/complexity/noBannedTypes: React Native's codegen maps `Object` to jsi::Object.
  readonly call: (method: string, args: Object) => Object
}

export default TurboModuleRegistry.getEnforcing<Spec>('NativeTessel')
