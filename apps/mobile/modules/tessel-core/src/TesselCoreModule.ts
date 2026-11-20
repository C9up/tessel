import { NativeModule, requireNativeModule } from 'expo'

/** Ce que le cœur offre à l'app mobile. S'étoffe avec l'epic 02 (story 02-11). */
declare class TesselCoreModule extends NativeModule<Record<string, never>> {
  version(): string
}

export default requireNativeModule<TesselCoreModule>('TesselCore')
