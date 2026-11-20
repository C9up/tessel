/**
 * Entry point of the crypto worker: the page starts it as a module worker (story 12-01)
 * and talks to it with `new RemoteCore(worker)`. The core and the database live here,
 * never in the page.
 */

import { CoreHost } from './host.ts'
import { serve } from './rpc.ts'
import { loadWasm } from './wasm.ts'

/** The IndexedDB database of the profile. */
const DATABASE = 'tessel'

serve(
  self,
  loadWasm().then(() => new CoreHost({ database: DATABASE })),
)
