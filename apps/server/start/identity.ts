import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import { Method } from '@tessel/protocol'
import { Authenticate } from '#modules/identity/application/Authenticate.js'
import { CreateAccount } from '#modules/identity/application/CreateAccount.js'
import { KeyPackageDirectory } from '#modules/identity/application/KeyPackageDirectory.js'
import { SignIn } from '#modules/identity/application/SignIn.js'
import { identityMethods } from '#modules/identity/http/identityMethods.js'
import { coreCertificates } from '#modules/identity/infrastructure/coreCertificates.js'
import { coreKeyPackages } from '#modules/identity/infrastructure/coreKeyPackages.js'
import { coreSignIns } from '#modules/identity/infrastructure/coreSignIns.js'
import { keyDbChallenges } from '#modules/identity/infrastructure/KeyDbChallenges.js'
import { SqlAccounts } from '#modules/identity/infrastructure/SqlAccounts.js'
import { SqlDevices } from '#modules/identity/infrastructure/SqlDevices.js'
import { SqlKeyPackages } from '#modules/identity/infrastructure/SqlKeyPackages.js'
import { WardenSessions } from '#modules/identity/infrastructure/WardenSessions.js'
import env from '#start/env.js'

interface Identity {
  createAccount: CreateAccount
  signIn: SignIn
  authenticate: Authenticate
  directory: KeyPackageDirectory
}

/** The identity module's use cases (03-01 to 03-03), composed once, on first use. */
let composed: Promise<Identity> | null = null
function identity(): Promise<Identity> {
  composed ??= app.container.make<AsyncDatabaseConnection>('db').then((db) => {
    const domain = env.get('APP_DOMAIN')
    const devices = new SqlDevices(db)
    const sessions = new WardenSessions(db)
    return {
      createAccount: new CreateAccount(coreCertificates, new SqlAccounts(db), domain),
      signIn: new SignIn(keyDbChallenges, devices, coreSignIns, sessions),
      authenticate: new Authenticate(sessions, devices, domain),
      directory: new KeyPackageDirectory(coreKeyPackages, devices, new SqlKeyPackages(db), domain),
    }
  })
  return composed
}

/** What other modules' points use to know the calling device. */
export const identityDeps = {
  createAccount: async () => (await identity()).createAccount,
  signIn: async () => (await identity()).signIn,
  authenticate: async () => (await identity()).authenticate,
  directory: async () => (await identity()).directory,
}

/** The identity module's RPC points. */
export function registerIdentityMethods(rpc: RpcRouter): void {
  const points = identityMethods(identityDeps)
  rpc.method(Method.createAccount, points.createAccount)
  rpc.method(Method.challenge, points.challenge)
  rpc.method(Method.signIn, points.signIn)
  rpc.method(Method.session, points.session)
  rpc.method(Method.signOut, points.signOut)
  rpc.method(Method.publishKeyPackages, points.publishKeyPackages)
  rpc.method(Method.keyPackageStock, points.keyPackageStock)
  rpc.method(Method.claimKeyPackages, points.claimKeyPackages)
}
