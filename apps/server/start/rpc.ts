// Les points RPC des modules, enregistrés sur le routeur partagé. Chaque module
// déclare les siens dans sa couche http (`http/*Methods.ts`) ; `start/` ne fait
// que les enregistrer.
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import { registerSystemMethods } from '#modules/system/http/systemMethods.js'

const rpc = await app.container.make<RpcRouter>('rpc')

registerSystemMethods(rpc)
