import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import { Method } from '@tessel/protocol'
import { CreateAccount } from '#modules/identity/application/CreateAccount.js'
import { identityMethods } from '#modules/identity/http/identityMethods.js'
import { coreCertificates } from '#modules/identity/infrastructure/coreCertificates.js'
import { SqlAccounts } from '#modules/identity/infrastructure/SqlAccounts.js'
import env from '#start/env.js'

/** The account creation (03-01), composed once. */
let composed: Promise<CreateAccount> | null = null
function createAccount(): Promise<CreateAccount> {
  composed ??= app.container.make<AsyncDatabaseConnection>('db').then((db) => new CreateAccount(coreCertificates, new SqlAccounts(db), env.get('APP_DOMAIN')))
  return composed
}

/** The identity module's RPC points. */
export function registerIdentityMethods(rpc: RpcRouter): void {
  const points = identityMethods({ createAccount })
  rpc.method(Method.createAccount, points.createAccount)
}
