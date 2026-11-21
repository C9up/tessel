/**
 * The contract of `TesselCore` (story 02-10) on the web: the core called directly in
 * the worker, and the core reached through messages, as the page does. The scenarios
 * are in `contract.ts`; the mobile binding runs them too (story 02-13).
 */

import { readFile } from 'node:fs/promises'
import { after } from 'node:test'
import { IDBFactory } from 'fake-indexeddb'
import { contract } from './contract.ts'
import { CoreHost } from './host.ts'
import { RemoteCore, serve } from './rpc.ts'
import { loadWasmSync } from './wasm.ts'

loadWasmSync(await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url)))

const factory = new IDBFactory()
const cleanups: (() => void)[] = []
after(() => {
  for (const cleanup of cleanups) cleanup()
})

contract({
  direct: (database) => new CoreHost({ database, factory }),
  'through messages': (database) => {
    const { port1, port2 } = new MessageChannel()
    const stop = serve(port1, new CoreHost({ database, factory }))
    const remote = new RemoteCore(port2)
    cleanups.push(() => {
      stop()
      remote.disconnect()
      port1.close()
      port2.close()
    })
    return remote
  },
})
