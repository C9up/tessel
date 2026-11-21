// The modules' RPC points, registered on the shared router. Each module declares
// its own in its http layer (`http/*Methods.ts`); `start/` only registers them.
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import { registerSystemMethods } from '#modules/system/http/systemMethods.js'
import { registerDeliveryMethods } from './delivery.js'
import { registerIdentityMethods } from './identity.js'

const rpc = await app.container.make<RpcRouter>('rpc')

registerSystemMethods(rpc)
registerIdentityMethods(rpc)
registerDeliveryMethods(rpc)
