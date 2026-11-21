import { createNapiConnection } from '@c9up/atlas'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import emitter from '@c9up/ream/events/services/main'
import { checkKeyPackage, type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, fromBase64Url, type TesselClient, toBase64Url } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { KeyPackagesLow } from '#modules/identity/domain/events.js'
import { LOW_STOCK } from '#modules/identity/domain/keyPackages.js'
import { SqlDevices } from '#modules/identity/infrastructure/SqlDevices.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * The KeyPackage directory (story 03-03), end to end: the KeyPackages are made by the
 * core as in the app, checked by the core on the server, handed out once.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())

async function failure(call: Promise<unknown>): Promise<{ code: unknown; message: unknown }> {
  try {
    await call
  } catch (error) {
    if (error instanceof Error && 'code' in error) return { code: error.code, message: error.message }
    throw error
  }
  throw new Error('the call succeeded')
}

interface ApiRequest {
  json(body: unknown): ApiRequest
  bearerToken(token: string): ApiRequest
  send(): Promise<{ status(): number; body(): unknown }>
}

interface ApiClient {
  post(url: string): ApiRequest
}

function connect(client: ApiClient, token?: string): TesselClient {
  return createTesselClient(async (url, body) => {
    const request = client.post(url).json(body)
    const response = await (token === undefined ? request : request.bearerToken(token)).send()
    return response.status() === 204 ? null : response.body()
  })
}

async function database() {
  return createNapiConnection(databaseConfig.url ?? '', 1, 1)
}

interface Member {
  account: string
  device: ThrowawayDevice
  /** The client, signed in as this device. */
  tessel: TesselClient
}

/** A new account's first device, registered and signed in. */
async function member(client: ApiClient, prefix: string): Promise<Member> {
  const anonymous = connect(client)
  const name = fresh(prefix)
  const account = `${name}@${domain}`
  const device = throwawayDevice(account)
  devices.push(device)
  await anonymous.createAccount(device.certificate)
  created.push(name)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  return { account, device, tessel: connect(client, token) }
}

/** The KeyPackages handed out, as the core reads them. */
function read(keyPackages: readonly { device: string; keyPackage: string }[]) {
  return keyPackages.map((entry) => {
    const checked = checkKeyPackage(fromBase64Url(entry.keyPackage) ?? new Uint8Array())
    if (checked === undefined) throw new Error('the directory handed out an invalid KeyPackage')
    return {
      device: entry.device,
      reference: toBase64Url(checked.reference),
      lastResort: checked.lastResort,
      checkedDevice: toBase64Url(checked.device.deviceId),
    }
  })
}

describe('the KeyPackage directory', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('a device leaves its pool; whoever adds it takes one KeyPackage, which is gone', async ({ client }) => {
    const bob = await member(client, 'bob')
    const alice = await member(client, 'alice')
    const pool = bob.device.refreshKeyPackages(now())
    expect(pool.publish.length).toBe(100)
    expect(await bob.tessel.publishKeyPackages(pool)).toEqual({ stock: 100, low: false })

    const [taken, ...others] = read((await alice.tessel.claimKeyPackages(bob.account)).keyPackages)
    expect(others.length).toBe(0)
    expect(taken?.device).toBe(toBase64Url(bob.device.deviceId))
    expect(taken?.checkedDevice).toBe(toBase64Url(bob.device.deviceId))
    expect(taken?.lastResort).toBe(false)
    expect(await bob.tessel.keyPackageStock()).toEqual({ stock: 99, low: false })

    // Its own devices are left out; an unknown account has none.
    expect((await bob.tessel.claimKeyPackages(bob.account)).keyPackages).toEqual([])
    expect((await alice.tessel.claimKeyPackages(`${fresh('nobody')}@${domain}`)).keyPackages).toEqual([])
  })

  test('concurrent claims never get the same KeyPackage; the last-resort one serves when none is left', async ({ client }) => {
    const bob = await member(client, 'bob')
    const alice = await member(client, 'alice')
    const pool = bob.device.refreshKeyPackages(now())
    await bob.tessel.publishKeyPackages({ publish: pool.publish.slice(0, 5), lastResort: pool.lastResort, withdraw: [] })

    const claims = await Promise.all(Array.from({ length: 8 }, () => alice.tessel.claimKeyPackages(bob.account)))
    const handed = read(claims.flatMap((claim) => claim.keyPackages))
    expect(handed.length).toBe(8)
    const single = handed.filter((entry) => !entry.lastResort)
    expect(single.length).toBe(5)
    expect(new Set(single.map((entry) => entry.reference)).size).toBe(5)
    expect(handed.filter((entry) => entry.lastResort).length).toBe(3)
    expect(await bob.tessel.keyPackageStock()).toEqual({ stock: 0, low: true })
  })

  test('the device hears when a claim runs its stock low', async ({ client }) => {
    const bob = await member(client, 'bob')
    const alice = await member(client, 'alice')
    const pool = bob.device.refreshKeyPackages(now())
    await bob.tessel.publishKeyPackages({ publish: pool.publish.slice(0, LOW_STOCK + 1), withdraw: [] })

    const heard: KeyPackagesLow[] = []
    const unsubscribe = emitter.on(KeyPackagesLow, (event: KeyPackagesLow) => void heard.push(event))
    try {
      await alice.tessel.claimKeyPackages(bob.account)
      expect(heard.length).toBe(0)
      await alice.tessel.claimKeyPackages(bob.account)
      await alice.tessel.claimKeyPackages(bob.account)
    } finally {
      unsubscribe()
    }
    // Once, on crossing: not again on the next claim.
    expect(heard.length).toBe(1)
    expect(heard[0]?.deviceId).toBe(Buffer.from(bob.device.deviceId).toString('hex'))
    expect(heard[0]?.stockLeft).toBe(LOW_STOCK - 1)
  })

  test('a device leaves only its own, valid KeyPackages, and nothing of a refused upload is kept', async ({ client }) => {
    const bob = await member(client, 'bob')
    const mallory = await member(client, 'mallory')
    const pool = bob.device.refreshKeyPackages(now())
    const theirs = mallory.device.refreshKeyPackages(now())
    const invalid = { code: -32602, message: 'E_IDENTITY_INVALID_KEY_PACKAGE' }

    // Another device's KeyPackage, mixed with its own.
    expect(await failure(bob.tessel.publishKeyPackages({ publish: [...pool.publish.slice(0, 3), ...theirs.publish.slice(0, 1)], withdraw: [] }))).toEqual(
      invalid,
    )
    // A last-resort KeyPackage passed as a single-use one.
    expect(await failure(bob.tessel.publishKeyPackages({ publish: pool.lastResort === undefined ? [] : [pool.lastResort], withdraw: [] }))).toEqual(invalid)
    // Bytes that are no KeyPackage.
    expect(await failure(bob.tessel.publishKeyPackages({ publish: [new Uint8Array([1, 2, 3])], withdraw: [] }))).toEqual(invalid)
    expect(await bob.tessel.keyPackageStock()).toEqual({ stock: 0, low: true })
  })

  test('a withdrawn KeyPackage is handed out no more', async ({ client }) => {
    const bob = await member(client, 'bob')
    const alice = await member(client, 'alice')
    const pool = bob.device.refreshKeyPackages(now())
    const [kept, withdrawn] = pool.publish
    if (kept === undefined || withdrawn === undefined) throw new Error('a pool of 100')
    await bob.tessel.publishKeyPackages({ publish: [kept, withdrawn], withdraw: [] })
    const reference = checkKeyPackage(withdrawn)?.reference ?? new Uint8Array()
    expect(await bob.tessel.publishKeyPackages({ publish: [], withdraw: [reference] })).toEqual({ stock: 1, low: true })
    const [handed] = read((await alice.tessel.claimKeyPackages(bob.account)).keyPackages)
    expect(handed?.reference).toBe(toBase64Url(checkKeyPackage(kept)?.reference ?? new Uint8Array()))
  })

  test('a revoked device is offered to nobody', async ({ client }) => {
    const bob = await member(client, 'bob')
    const alice = await member(client, 'alice')
    await bob.tessel.publishKeyPackages(bob.device.refreshKeyPackages(now()))
    await new SqlDevices(await database()).revoke(bob.device.deviceId)
    expect((await alice.tessel.claimKeyPackages(bob.account)).keyPackages).toEqual([])
  })

  test('the directory needs a session', async ({ client }) => {
    const anonymous = connect(client)
    expect(await failure(anonymous.claimKeyPackages(`alice@${domain}`))).toEqual({ code: -32001, message: 'E_UNAUTHENTICATED' })
    expect(await failure(anonymous.keyPackageStock())).toEqual({ code: -32001, message: 'E_UNAUTHENTICATED' })
  })
})
