import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import emitter from '@c9up/ream/events/services/main'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient, toBase64Url } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { MAX_DEVICES } from '#modules/identity/application/LinkDevice.js'
import { DeviceAdded } from '#modules/identity/domain/events.js'
import env from '#start/env.js'

/**
 * Adding a device by QR code (story 03-04), end to end: the QR code, the certificate and
 * the sealed grant are made by the core as in the apps; the server registers the device
 * and relays a grant it cannot open.
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

/** Signs `device` in: a client with its session. */
async function signedIn(client: ApiClient, device: ThrowawayDevice): Promise<TesselClient> {
  const anonymous = connect(client)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge))
  return connect(client, token)
}

/** A new account's first device, registered and signed in. */
async function member(client: ApiClient, prefix: string) {
  const name = fresh(prefix)
  const account = `${name}@${domain}`
  const device = throwawayDevice(account)
  devices.push(device)
  await connect(client).createAccount(device.certificate)
  created.push(name)
  return { account, device, tessel: await signedIn(client, device) }
}

describe('adding a device by QR code', () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('the scanned device becomes a device of the account, signs in and is offered to the account', async ({ client }) => {
    const alice = await member(client, 'alice')
    const anonymous = connect(client)

    // The tablet shows its QR code and waits for its grant.
    const link = throwawayLink()
    expect(await anonymous.takeLinkGrant(link.link)).toBe(undefined)

    // Alice's phone scans it, certifies the tablet and posts the sealed grant.
    const granted = alice.device.grantLink(link.qr)
    const heard: DeviceAdded[] = []
    const unsubscribe = emitter.on(DeviceAdded, (event: DeviceAdded) => void heard.push(event))
    try {
      expect(await alice.tessel.linkDevice(granted.certificate, granted.link, granted.grant)).toEqual({ linked: true })
    } finally {
      unsubscribe()
    }
    expect(heard.length).toBe(1)
    expect(heard[0]?.type).toBe('identity.device.added')

    // The tablet takes its grant, once, and starts as Alice's device.
    const grant = await anonymous.takeLinkGrant(link.link)
    expect(grant).toEqual(granted.grant)
    expect(await anonymous.takeLinkGrant(link.link)).toBe(undefined)
    const tablet = link.accept(grant ?? new Uint8Array())
    devices.push(tablet)
    expect(tablet.certificate).toEqual(granted.certificate)

    const session = await (await signedIn(client, tablet)).session()
    expect(session.account).toBe(alice.account)
    expect(session.device).toBe(toBase64Url(tablet.deviceId))

    // It leaves its KeyPackages; Alice's phone takes one to add it to each group.
    const tabletClient = await signedIn(client, tablet)
    await tabletClient.publishKeyPackages(tablet.refreshKeyPackages(now()))
    const claimed = await alice.tessel.claimKeyPackages(alice.account)
    expect(claimed.keyPackages.map((entry) => entry.device)).toEqual([toBase64Url(tablet.deviceId)])
  })

  test('a link takes one grant, the first', async ({ client }) => {
    const alice = await member(client, 'alice')
    const link = throwawayLink()
    const first = alice.device.grantLink(link.qr)
    await alice.tessel.linkDevice(first.certificate, first.link, first.grant)
    const second = alice.device.grantLink(link.qr)
    expect(await failure(alice.tessel.linkDevice(second.certificate, second.link, second.grant))).toEqual({ code: -32012, message: 'E_IDENTITY_LINK_USED' })
  })

  test('a certificate of another account, or signed by another key, is refused and leaves no grant', async ({ client }) => {
    const alice = await member(client, 'alice')
    const bob = await member(client, 'bob')
    const invalid = { code: -32602, message: 'E_IDENTITY_INVALID_CERTIFICATE' }

    // Bob's certificate, posted from Alice's session.
    const forBob = throwawayLink()
    const bobs = bob.device.grantLink(forBob.qr)
    expect(await failure(alice.tessel.linkDevice(bobs.certificate, bobs.link, bobs.grant))).toEqual(invalid)

    // A certificate naming Alice's account, signed by a key that is not Alice's.
    const impostor = throwawayDevice(alice.account)
    devices.push(impostor)
    const forged = throwawayLink()
    const forgery = impostor.grantLink(forged.qr)
    expect(await failure(alice.tessel.linkDevice(forgery.certificate, forgery.link, forgery.grant))).toEqual(invalid)
    expect(await connect(client).takeLinkGrant(forged.link)).toBe(undefined)
  })

  test(`an account has at most ${MAX_DEVICES} devices`, async ({ client }) => {
    const alice = await member(client, 'alice')
    for (let i = 1; i < MAX_DEVICES; i++) {
      const granted = alice.device.grantLink(throwawayLink().qr)
      await alice.tessel.linkDevice(granted.certificate, granted.link, granted.grant)
    }
    const one = alice.device.grantLink(throwawayLink().qr)
    expect(await failure(alice.tessel.linkDevice(one.certificate, one.link, one.grant))).toEqual({ code: -32011, message: 'E_IDENTITY_TOO_MANY_DEVICES' })
  })

  test('only a signed-in device links; anyone may poll a link, which reveals nothing', async ({ client }) => {
    const alice = await member(client, 'alice')
    const granted = alice.device.grantLink(throwawayLink().qr)
    expect(await failure(connect(client).linkDevice(granted.certificate, granted.link, granted.grant))).toEqual({ code: -32001, message: 'E_UNAUTHENTICATED' })
    expect(await connect(client).takeLinkGrant(crypto.getRandomValues(new Uint8Array(16)))).toBe(undefined)
    expect(await failure(connect(client).takeLinkGrant(new Uint8Array(3)))).toEqual({ code: -32602, message: 'E_INVALID_PARAMS' })
  })
})
