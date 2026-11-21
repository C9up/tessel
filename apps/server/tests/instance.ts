/**
 * Another instance of the server, in a process of its own, for the tests that need two
 * (story 04-05): relay's service is one per process. It prints its port, and stops on
 * SIGTERM.
 */
import { Ignitor } from '@c9up/ream'
import { createHyperServerFactory } from '@c9up/ream/bootstrap'

const ignitor = await new Ignitor(new URL('../', import.meta.url), { port: 0, serverFactory: createHyperServerFactory() })
  .useRcFile((await import('../reamrc.js')).default)
  .testMode()
  .start()

process.stdout.write(`INSTANCE_PORT=${await ignitor.port()}\n`)
process.on('SIGTERM', () => {
  void ignitor.stop().finally(() => process.exit(0))
})
