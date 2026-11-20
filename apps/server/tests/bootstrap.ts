import type { ConfigureOptions, Plugin, RunnerHook } from '@c9up/helix'
import { apiClient } from '@c9up/helix-plugin-ream'
import { Ignitor } from '@c9up/ream'
import { createHyperServerFactory } from '@c9up/ream/bootstrap'
import { createTestUtils } from '@c9up/ream/testing/utils'

const APP_ROOT = new URL('../', import.meta.url)

export const testUtils = createTestUtils(async (port) => {
  const ignitor = await new Ignitor(APP_ROOT, {
    port,
    serverFactory: createHyperServerFactory(),
  })
    .useRcFile((await import('../reamrc.js')).default)
    // `testMode()`, not `httpServer()`: the suite still gets a server (the
    // factory above is what asks for one), but the application declares itself
    // as `test`. That is what a provider entry's `environment: ['web']` reads,
    // so a scheduler or a worker can be kept out of a test run declaratively
    // instead of firing in the middle of it.
    .testMode()
    .start()

  // Port 0 asks the OS for a free one, so the caller has to be told which.
  return { port: await ignitor.port(), close: () => ignitor.stop() }
})

/**
 * The plugins every suite gets. helix reads this export, along with
 * `runnerHooks` and `configureSuite` below: this file never calls
 * `configure()` itself.
 */
export const plugins: Plugin[] = [apiClient({ testUtils })]

/**
 * Run once for the whole run, before and after every suite.
 */
export const runnerHooks: { setup: RunnerHook[]; teardown: RunnerHook[] } = {
  setup: [],
  teardown: [],
}

/**
 * Une seule suite, `app` : les tests vivent à côté de leur module (`__tests__/`).
 * Le serveur HTTP ne démarre qu'une fois pour la suite.
 */
export const configureSuite: ConfigureOptions['configureSuite'] = (suite) => {
  if (suite.name === 'app') {
    return suite.setup(() => testUtils.httpServer().start())
  }
}
