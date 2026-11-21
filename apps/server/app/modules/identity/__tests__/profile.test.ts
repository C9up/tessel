import { createNapiConnection } from '@c9up/atlas'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import emitter from '@c9up/ream/events/services/main'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { ProfileUpdated } from '#modules/identity/domain/events.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * Encrypted profiles (story 03-06), end to end: the blob is sealed by the core, kept by
 * the server as it came, and opened by the account's devices only (and, once the key is
 * shared over MLS, by its contacts: see the core's tests).
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`

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

async function signedIn(client: ApiClient, device: ThrowawayDevice): Promise<TesselClient> {
  const anonymous = connect(client)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  return connect(client, token)
}

async function member(client: ApiClient, prefix: string) {
  const name = fresh(prefix)
  const account = `${name}@${domain}`
  const device = throwawayDevice(account)
  devices.push(device)
  await connect(client).createAccount(device.certificate)
  created.push(name)
  return { name, account, device, tessel: await signedIn(client, device) }
}

describe('encrypted profiles', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('the server keeps the sealed blob as it came, and never the name', async ({ client }) => {
    const alice = await member(client, 'alice')
    const blob = alice.device.sealProfile({ name: 'Alice Liddell' })
    const heard: ProfileUpdated[] = []
    const unsubscribe = emitter.on(ProfileUpdated, (event: ProfileUpdated) => void heard.push(event))
    try {
      expect(await alice.tessel.setProfile(blob)).toEqual({ saved: true })
    } finally {
      unsubscribe()
    }
    expect(heard.length).toBe(1)

    const [row] = await (await database()).query<{ blob: string }>(
      `SELECT encode(p.blob, 'hex') AS blob FROM profiles p JOIN accounts a ON a.id = p.account_id WHERE a.name = $1`,
      [alice.name],
    )
    expect(row?.blob).toBe(Buffer.from(blob).toString('hex'))
    expect(row?.blob.includes(Buffer.from('Alice Liddell').toString('hex'))).toBe(false)

    const bob = await member(client, 'bob')
    const fetched = await bob.tessel.profile(alice.account)
    expect(fetched).toEqual(blob)
    // Bob has no key yet: it does not open. Alice's own device opens it.
    expect(bob.device.openProfileName(alice.account, fetched ?? new Uint8Array())).toBe(undefined)
    expect(alice.device.openProfileName(alice.account, fetched ?? new Uint8Array())).toBe('Alice Liddell')
  })

  test("a linked device opens its account's profile: the key came in the grant", async ({ client }) => {
    const alice = await member(client, 'alice')
    await alice.tessel.setProfile(alice.device.sealProfile({ name: 'Alice' }))
    const link = throwawayLink()
    const granted = alice.device.grantLink(link.qr)
    await alice.tessel.linkDevice(granted.certificate, granted.link, granted.grant)
    const tablet = link.accept((await connect(client).takeLinkGrant(link.link)) ?? new Uint8Array())
    devices.push(tablet)
    const blob = await (await signedIn(client, tablet)).profile(alice.account)
    expect(tablet.openProfileName(alice.account, blob ?? new Uint8Array())).toBe('Alice')
  })

  test('a new profile replaces the old one', async ({ client }) => {
    const alice = await member(client, 'alice')
    await alice.tessel.setProfile(alice.device.sealProfile({ name: 'Alice' }))
    const renamed = alice.device.sealProfile({ name: 'Alice L.' })
    await alice.tessel.setProfile(renamed)
    expect(await alice.tessel.profile(alice.account)).toEqual(renamed)
  })

  test('an account without a profile, or that cannot exist, has none; another instance is refused', async ({ client }) => {
    const alice = await member(client, 'alice')
    expect(await alice.tessel.profile(alice.account)).toBe(undefined)
    expect(await alice.tessel.profile(`N!@${domain}`)).toBe(undefined)
    expect(await failure(alice.tessel.profile('bob@elsewhere.example'))).toEqual({ code: -32602, message: 'E_IDENTITY_WRONG_DOMAIN' })
  })

  test('what is not a sealed profile is refused; a session is needed', async ({ client }) => {
    const alice = await member(client, 'alice')
    const invalid = { code: -32602, message: 'E_IDENTITY_INVALID_PROFILE' }
    expect(await failure(alice.tessel.setProfile(new Uint8Array(10)))).toEqual(invalid)
    const newer = alice.device.sealProfile({ name: 'Alice' })
    newer[0] = 2
    expect(await failure(alice.tessel.setProfile(newer))).toEqual(invalid)
    expect(await failure(connect(client).profile(alice.account))).toEqual({ code: -32001, message: 'E_UNAUTHENTICATED' })
  })
})
