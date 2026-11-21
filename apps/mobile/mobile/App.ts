import { component, html, signal } from '@c9up/aurora/core'
import { colorScheme } from '@c9up/orbit'
import { mobileCore, type NativeTesselModule } from '@tessel/core-mobile'
import { Appearance, type TurboModule, TurboModuleRegistry } from 'react-native'
import { selfCheck } from './selfCheck.ts'

// `dark:` follows the phone's setting (orbit does not do it yet: orbit bug 003).
const followScheme = () => colorScheme(Appearance.getColorScheme() === 'dark' ? 'dark' : 'light')
followScheme()
Appearance.addChangeListener(followScheme)

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
  return html`<div class="flex-1 p-4 pt-12 bg-white dark:bg-slate-950">
    <h1 class="text-xl font-semibold text-slate-900 dark:text-slate-50">Tessel: core on the phone</h1>
    <p class="mt-2 text-base text-slate-700 dark:text-slate-300">${() => verdict()}</p>
    <p class="mt-4 text-sm text-slate-800 dark:text-slate-200">${() => lines().join('\n')}</p>
  </div>`
})
