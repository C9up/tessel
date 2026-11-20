import 'reflect-metadata'
import { Ignitor, prettyPrintError } from '@c9up/ream'
import { createHyperServerFactory } from '@c9up/ream/bootstrap'
import { setOutboxActive } from '#modules/shared/infrastructure/events.js'
import { outboxRelay } from '#modules/shared/infrastructure/outboxRelay.js'

const APP_ROOT = new URL('../', import.meta.url)

// PORT is not read here: `#start/env` loads `.env` in `booting()`, and the Ignitor
// reads the port when it binds.
new Ignitor(APP_ROOT, {
  serverFactory: createHyperServerFactory(),
})
  .tap((app) => {
    // Validate the environment before anything starts: a missing APP_KEY stops the app here.
    app.booting(async () => {
      await import('#start/env.js')
    })
  })
  .useRcFile((await import('../reamrc.js')).default)
  .httpServer()
  .start()
  .then(async (app) => {
    // The outbox and its relay run only in the real server, never under the test harness.
    setOutboxActive(true)
    outboxRelay.start()
    console.log(`\n  ➜ Tessel ready on http://${app.host()}:${await app.port()}\n`)
  })
  .catch((err) => {
    prettyPrintError(err)
    process.exit(1)
  })
