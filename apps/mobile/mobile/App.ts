import { component, html, signal } from '@c9up/aurora/core'
import { mobileCore, type NativeTesselModule } from '@tessel/core-mobile'
import { type TurboModule, TurboModuleRegistry } from 'react-native'
import { selfCheck } from './selfCheck.ts'

/** The core's JSI module, which @tessel/core-mobile declares (specs/NativeTessel.ts). */
interface NativeTessel extends TurboModule, NativeTesselModule {}

/** The phone check of the core (story 02-13): each step on its own line. */
export const App = component(() => {
  const lines = signal<string[]>([])
  const verdict = signal('running')
  const native = TurboModuleRegistry.getEnforcing<NativeTessel>('NativeTessel')
  void selfCheck(
    () => mobileCore(native),
    (line) => lines((previous) => [...previous, line]),
  ).then((passed) => verdict(passed ? 'all checks passed' : 'a check failed'))
  return html`<div class="flex-1 p-4 bg-white">
    <h1 class="text-xl font-semibold text-slate-900">Tessel: core on the phone</h1>
    <p class="mt-2 text-base text-slate-700">${() => verdict()}</p>
    <p class="mt-4 text-sm text-slate-800">${() => lines().join('\n')}</p>
  </div>`
})
