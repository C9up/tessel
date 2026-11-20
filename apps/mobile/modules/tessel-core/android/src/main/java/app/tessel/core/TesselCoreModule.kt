package app.tessel.core

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * tessel-core pour l'app mobile. Une couche mince : chaque fonction appelle les
 * liaisons uniffi du cœur Rust et ne décide rien.
 */
class TesselCoreModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("TesselCore")

    Function("version") {
      uniffi.tessel_ffi.version()
    }
  }
}
