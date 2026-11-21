import { createNapiConnection } from '@c9up/atlas'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { Mailbox } from '#modules/delivery/application/Mailbox.js'
import { DeliveryError, ENVELOPE_LIMITS } from '#modules/delivery/domain/envelope.js'
import { ACCOUNT_SENDS, SENDS } from '#modules/delivery/http/deliveryMethods.js'
import { coreEnvelopes } from '#modules/delivery/infrastructure/coreEnvelopes.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * Limits (story 04-06): an envelope no larger than its kind allows, refused by name;
 * envelopes counted per device and per account, in KeyDB, so that no device nor account
 * drowns the server.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const sendId = () => crypto.getRandomValues(new Uint8Array(16))

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

/** An account's first device, and `linked` more, each signed in. */
async function account(client: ApiClient, linked: number) {
  const name = fresh('alice')
  const phone = throwawayDevice(`${name}@${domain}`)
  devices.push(phone)
  await connect(client).createAccount(phone.certificate)
  created.push(name)
  const onPhone = await signedIn(client, phone)
  const sessions = [onPhone]
  for (let i = 0; i < linked; i++) {
    const link = throwawayLink()
    const granted = phone.grantLink(link.qr)
    await onPhone.linkDevice(granted.certificate, granted.link, granted.grant)
    const device = link.accept((await connect(client).takeLinkGrant(link.link)) ?? new Uint8Array())
    devices.push(device)
    sessions.push(await signedIn(client, device))
  }
  return { phone, sessions }
}

const TOO_MANY = 'E_TOO_MANY_REQUESTS'
const junk = new Uint8Array([1, 2, 3])

describe('limits', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('each kind has its size; larger is refused by name, with the limits', async ({ client }) => {
    const { phone, sessions } = await account(client, 0)
    const [onPhone] = sessions
    const huge = new Uint8Array(Math.max(...Object.values(ENVELOPE_LIMITS)) + 1)
    const refused = onPhone?.send(sendId(), huge, [crypto.getRandomValues(new Uint8Array(16))]) ?? Promise.resolve()
    expect(await failure(refused)).toEqual({ code: -32015, message: 'E_DELIVERY_TOO_LARGE' })

    // Per kind: a real message, against a mailbox whose message limit is below its size.
    const group: Uint8Array = phone.core.createGroup()
    phone.core.queueMessage(group, { type: 'text', text: 'x'.repeat(2000) }, Date.now())
    const message: Uint8Array = phone.core.nextOutbound(group).encrypted
    const kept = { store: async () => ({ id: '1', seq: 1 }), fetch: async () => [], ack: async () => 0, purge: async () => 0 }
    const silent = { ring: () => undefined }
    const strict = new Mailbox(coreEnvelopes, kept, silent, { ...ENVELOPE_LIMITS, application: message.length - 1 })
    const roomy = new Mailbox(coreEnvelopes, kept, silent, { ...ENVELOPE_LIMITS, application: message.length })
    const envelope = { sendId: sendId(), body: message, recipients: [crypto.getRandomValues(new Uint8Array(16))] }
    await expect(strict.send(phone.deviceId, envelope)).rejects.toEqual(new DeliveryError('tooLarge'))
    expect(await roomy.send(phone.deviceId, envelope)).toEqual({ id: '1', seq: 1 })
  })

  test('a device sends so many envelopes a minute, no more', async ({ client }) => {
    const { sessions } = await account(client, 0)
    const [onPhone] = sessions
    const to = [crypto.getRandomValues(new Uint8Array(16))]
    for (let i = 0; i < SENDS.requests; i++) await failure(onPhone?.send(sendId(), junk, to) ?? Promise.resolve())
    expect((await failure(onPhone?.send(sendId(), junk, to) ?? Promise.resolve())).message).toBe(TOO_MANY)
  })

  test('an account cannot get around it by linking devices', async ({ client }) => {
    const { sessions } = await account(client, 2)
    const to = [crypto.getRandomValues(new Uint8Array(16))]
    const [phone, tablet, laptop] = sessions
    // Each device under its own limit, the account at its own.
    const each = SENDS.requests - 1
    const rest = ACCOUNT_SENDS.requests - 2 * each
    for (const [session, count] of [
      [phone, each],
      [tablet, each],
      [laptop, rest],
    ] as const) {
      for (let i = 0; i < count; i++) await failure(session?.send(sendId(), junk, to) ?? Promise.resolve())
    }
    expect((await failure(laptop?.send(sendId(), junk, to) ?? Promise.resolve())).message).toBe(TOO_MANY)
  })
})
