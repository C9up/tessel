import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient, toBase64Url } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { SIGN_INS } from '#modules/identity/http/identityMethods.js'
import { SqlDevices } from '#modules/identity/infrastructure/SqlDevices.js'
import env from '#start/env.js'

/**
 * Signing in with the device key (story 03-02), end to end: the challenge comes from
 * KeyDB, the proof is made by the core as in the app, the session is warden's.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`

/** The error a call failed with: its JSON-RPC code and message. */
async function failure(call: Promise<unknown>): Promise<{ code: unknown; message: unknown }> {
  try {
    await call
  } catch (error) {
    if (error instanceof Error && 'code' in error) return { code: error.code, message: error.message }
    throw error
  }
  throw new Error('the call succeeded')
}

const REFUSED = { code: -32001, message: 'E_IDENTITY_SIGN_IN_REFUSED' }
const UNAUTHENTICATED = { code: -32001, message: 'E_UNAUTHENTICATED' }

interface ApiRequest {
  json(body: unknown): ApiRequest
  bearerToken(token: string): ApiRequest
  send(): Promise<{ status(): number; body(): unknown }>
}

/** Ream's test client, as far as JSON-RPC needs it. */
interface ApiClient {
  post(url: string): ApiRequest
}

/** The client shared by web and mobile, over Ream's test client, with a session or without. */
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

/** A new account's first device, registered. */
async function register(tessel: TesselClient, prefix: string): Promise<ThrowawayDevice> {
  const name = fresh(prefix)
  const device = throwawayDevice(`${name}@${domain}`)
  devices.push(device)
  await tessel.createAccount(device.certificate)
  created.push(name)
  return device
}

/** What the app does: draw a challenge, sign it, open a session. */
async function signIn(tessel: TesselClient, device: ThrowawayDevice): Promise<string> {
  const challenge = await tessel.challenge()
  return (await tessel.signIn(device.deviceId, challenge, device.signIn(challenge))).token
}

describe('signing in with the device key', () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('a device signs a fresh challenge and gets a session bound to it', async ({ client }) => {
    const tessel = connect(client)
    const device = await register(tessel, 'alice')
    const token = await signIn(tessel, device)

    const session = await connect(client, token).session()
    expect(session.device).toBe(toBase64Url(device.deviceId))
    expect(session.account.endsWith(`@${domain}`)).toBe(true)

    // The server keeps the hash of the token's secret, never the token.
    const [row] = await (await database()).query<{ hash: string; device: string }>(
      `SELECT hash, encode(device_id, 'hex') AS device FROM sessions WHERE device_id = decode($1, 'hex')`,
      [Buffer.from(device.deviceId).toString('hex')],
    )
    expect(row?.hash).toMatch(/^[0-9a-f]{64}$/)
    expect(token.includes(row?.hash ?? '')).toBe(false)
  })

  test('a challenge is good once, and every wrong proof gets the same refusal', async ({ client }) => {
    const tessel = connect(client)
    const alice = await register(tessel, 'alice')
    const bob = await register(tessel, 'bob')

    const challenge = await tessel.challenge()
    const proof = alice.signIn(challenge)
    await tessel.signIn(alice.deviceId, challenge, proof)
    // Replayed: the challenge is spent.
    expect(await failure(tessel.signIn(alice.deviceId, challenge, proof))).toEqual(REFUSED)

    // Bob's proof for Alice's device, a challenge the server never drew, a device it does not know.
    const fresh = await tessel.challenge()
    expect(await failure(tessel.signIn(alice.deviceId, fresh, bob.signIn(fresh)))).toEqual(REFUSED)
    const invented = crypto.getRandomValues(new Uint8Array(32))
    expect(await failure(tessel.signIn(alice.deviceId, invented, alice.signIn(invented)))).toEqual(REFUSED)
    const another = await tessel.challenge()
    expect(await failure(tessel.signIn(crypto.getRandomValues(new Uint8Array(16)), another, alice.signIn(another)))).toEqual(REFUSED)
  })

  test('no session, or a forged one, is refused', async ({ client }) => {
    expect(await failure(connect(client).session())).toEqual(UNAUTHENTICATED)
    expect(await failure(connect(client, 'tsn_forged.token').session())).toEqual(UNAUTHENTICATED)
    expect(await failure(connect(client, 'not a token').signOut())).toEqual(UNAUTHENTICATED)
  })

  test('signing out closes that session only', async ({ client }) => {
    const tessel = connect(client)
    const device = await register(tessel, 'carol')
    const first = await signIn(tessel, device)
    const second = await signIn(tessel, device)

    expect(await connect(client, first).signOut()).toEqual({ signedOut: true })
    expect(await failure(connect(client, first).session())).toEqual(UNAUTHENTICATED)
    expect((await connect(client, second).session()).device).toBe(toBase64Url(device.deviceId))
  })

  test('a revoked device signs in no more, and its sessions are gone', async ({ client }) => {
    const tessel = connect(client)
    const device = await register(tessel, 'dave')
    const token = await signIn(tessel, device)

    expect(await new SqlDevices(await database()).revoke(device.deviceId)).toBe(true)

    expect(await failure(connect(client, token).session())).toEqual(UNAUTHENTICATED)
    const [row] = await (await database()).query<{ sessions: string }>(`SELECT count(*)::text AS sessions FROM sessions WHERE device_id = decode($1, 'hex')`, [
      Buffer.from(device.deviceId).toString('hex'),
    ])
    expect(row?.sessions).toBe('0')
    const challenge = await tessel.challenge()
    expect(await failure(tessel.signIn(device.deviceId, challenge, device.signIn(challenge)))).toEqual(REFUSED)
  })

  test('one network signs in a few times a minute, no more', async ({ client }) => {
    const tessel = connect(client)
    const device = await register(tessel, 'eve')
    for (let i = 0; i < SIGN_INS.requests; i++) await signIn(tessel, device)
    const challenge = await tessel.challenge()
    const over = await failure(tessel.signIn(device.deviceId, challenge, device.signIn(challenge)))
    expect(over.message).toBe('E_TOO_MANY_REQUESTS')
  })
})
