import type { AsyncDatabaseConnection } from '@c9up/atlas'
import type { RpcRouter } from '@c9up/ream/rpc/router'
import app from '@c9up/ream/services/app'
import { Method } from '@tessel/protocol'
import { AccountDevices } from '#modules/identity/application/AccountDevices.js'
import { Authenticate } from '#modules/identity/application/Authenticate.js'
import { CreateAccount } from '#modules/identity/application/CreateAccount.js'
import { KeyPackageDirectory } from '#modules/identity/application/KeyPackageDirectory.js'
import { LinkDevice } from '#modules/identity/application/LinkDevice.js'
import { SignIn } from '#modules/identity/application/SignIn.js'
import { identityMethods } from '#modules/identity/http/identityMethods.js'
import { coreCertificates } from '#modules/identity/infrastructure/coreCertificates.js'
import { coreKeyPackages } from '#modules/identity/infrastructure/coreKeyPackages.js'
import { coreRevocations } from '#modules/identity/infrastructure/coreRevocations.js'
import { coreSignIns } from '#modules/identity/infrastructure/coreSignIns.js'
import { keyDbChallenges } from '#modules/identity/infrastructure/KeyDbChallenges.js'
import { keyDbLinkMailbox } from '#modules/identity/infrastructure/KeyDbLinkMailbox.js'
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
  link: LinkDevice
  devices: AccountDevices
}

/** The identity module's use cases (03-01 to 03-05), composed once, on first use. */
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
      link: new LinkDevice(coreCertificates, devices, keyDbLinkMailbox),
      devices: new AccountDevices(devices, coreRevocations),
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
  link: async () => (await identity()).link,
  devices: async () => (await identity()).devices,
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
  rpc.method(Method.linkDevice, points.linkDevice)
  rpc.method(Method.takeLinkGrant, points.takeLinkGrant)
  rpc.method(Method.devices, points.devices)
  rpc.method(Method.revokeDevice, points.revokeDevice)
}
