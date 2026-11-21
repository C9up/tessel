import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import emitter from '@c9up/ream/events/services/main'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { createTesselClient, type DeviceKind, type TesselClient, toBase64Url } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { DeviceRevoked } from '#modules/identity/domain/events.js'
import env from '#start/env.js'

/**
 * The device list and revocation (story 03-05), end to end: the revocation is signed by
 * the account key in the core, and checked by the core on the server.
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

async function signedIn(client: ApiClient, device: ThrowawayDevice, kind: DeviceKind): Promise<TesselClient> {
  const anonymous = connect(client)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), kind)
  return connect(client, token)
}

/** Alice's account: her phone (it holds the account key), and a tablet linked by QR code. */
async function aliceWithTablet(client: ApiClient) {
  const name = fresh('alice')
  const account = `${name}@${domain}`
  const phone = throwawayDevice(account)
  devices.push(phone)
  await connect(client).createAccount(phone.certificate)
  created.push(name)
  const onPhone = await signedIn(client, phone, 'android')

  const link = throwawayLink()
  const granted = phone.grantLink(link.qr)
  await onPhone.linkDevice(granted.certificate, granted.link, granted.grant)
  const tablet = link.accept((await connect(client).takeLinkGrant(link.link)) ?? new Uint8Array())
  devices.push(tablet)
  const onTablet = await signedIn(client, tablet, 'web')
  return { account, phone, onPhone, tablet, onTablet }
}

const UNAUTHENTICATED = { code: -32001, message: 'E_UNAUTHENTICATED' }
const INVALID_REVOCATION = { code: -32602, message: 'E_IDENTITY_INVALID_REVOCATION' }
const UNKNOWN_DEVICE = { code: -32013, message: 'E_IDENTITY_UNKNOWN_DEVICE' }

describe("an account's devices", () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('the list shows each live device, its kind, and its days, never an hour', async ({ client }) => {
    const { phone, tablet, onTablet } = await aliceWithTablet(client)
    const { devices: listed } = await onTablet.devices()
    // The day as the database counts it.
    const [{ today } = { today: '' }] = await (await database()).query<{ today: string }>(`SELECT to_char(current_date, 'YYYY-MM-DD') AS today`, [])
    // Two devices of the same day come in no set order: the server knows the day only.
    const byDevice = (a: { device: string }, b: { device: string }) => a.device.localeCompare(b.device)
    expect([...listed].sort(byDevice)).toEqual(
      [
        { device: toBase64Url(phone.deviceId), kind: 'android', createdOn: today, lastActiveOn: today, current: false },
        { device: toBase64Url(tablet.deviceId), kind: 'web', createdOn: today, lastActiveOn: today, current: true },
      ].sort(byDevice),
    )
  })

  test('the phone revokes the lost tablet with the account key: its sessions fall, its contacts are told by the bus', async ({ client }) => {
    const { phone, onPhone, tablet, onTablet } = await aliceWithTablet(client)
    const heard: DeviceRevoked[] = []
    const unsubscribe = emitter.on(DeviceRevoked, (event: DeviceRevoked) => void heard.push(event))
    try {
      expect(await onPhone.revokeDevice(tablet.deviceId, phone.revokeDevice(tablet.deviceId))).toEqual({ revoked: true })
    } finally {
      unsubscribe()
    }
    expect(heard.map((event) => event.deviceId)).toEqual([Buffer.from(tablet.deviceId).toString('hex')])
    expect(await failure(onTablet.session())).toEqual(UNAUTHENTICATED)
    expect((await onPhone.devices()).devices.map((entry) => entry.device)).toEqual([toBase64Url(phone.deviceId)])
    // Revoked once: there is nothing left to revoke.
    expect(await failure(onPhone.revokeDevice(tablet.deviceId, phone.revokeDevice(tablet.deviceId)))).toEqual(UNKNOWN_DEVICE)
  })

  test('revoking another device takes the account key: a session alone, or a revocation of another device, is refused', async ({ client }) => {
    const { phone, onPhone, tablet, onTablet } = await aliceWithTablet(client)
    // The tablet does not hold the account key: it cannot lock Alice out of her phone.
    expect(await failure(onTablet.revokeDevice(phone.deviceId))).toEqual(INVALID_REVOCATION)
    // A revocation signed for another device.
    const other = phone.revokeDevice(crypto.getRandomValues(new Uint8Array(16)))
    expect(await failure(onPhone.revokeDevice(tablet.deviceId, other))).toEqual(INVALID_REVOCATION)
    expect((await onTablet.session()).device).toBe(toBase64Url(tablet.deviceId))
  })

  test('a device revokes itself without the account key', async ({ client }) => {
    const { onPhone, tablet, onTablet } = await aliceWithTablet(client)
    expect(await onTablet.revokeDevice(tablet.deviceId)).toEqual({ revoked: true })
    expect(await failure(onTablet.session())).toEqual(UNAUTHENTICATED)
    expect((await onPhone.devices()).devices.length).toBe(1)
  })

  test("another account's device is unknown", async ({ client }) => {
    const alice = await aliceWithTablet(client)
    const bob = await aliceWithTablet(client)
    expect(await failure(alice.onPhone.revokeDevice(bob.tablet.deviceId, alice.phone.revokeDevice(bob.tablet.deviceId)))).toEqual(UNKNOWN_DEVICE)
    expect((await bob.onTablet.session()).device).toBe(toBase64Url(bob.tablet.deviceId))
  })
})
