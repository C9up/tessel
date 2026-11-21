import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import emitter from '@c9up/ream/events/services/main'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { AccountDeleted } from '#modules/identity/domain/events.js'
import env from '#start/env.js'

/**
 * Deleting an account (story 03-07), end to end: the deletion is signed by the account
 * key in the core, checked by the core on the server, and leaves nothing but the name,
 * reserved for a while.
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

async function signedIn(client: ApiClient, device: ThrowawayDevice): Promise<TesselClient> {
  const anonymous = connect(client)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  return connect(client, token)
}

/** Alice, with a phone (it holds the account key), a linked tablet, KeyPackages and a profile. */
async function alice(client: ApiClient) {
  const name = fresh('alice')
  const account = `${name}@${domain}`
  const phone = throwawayDevice(account)
  devices.push(phone)
  await connect(client).createAccount(phone.certificate)
  created.push(name)
  const onPhone = await signedIn(client, phone)
  await onPhone.publishKeyPackages(phone.refreshKeyPackages(now()))
  await onPhone.setProfile(phone.sealProfile({ name: 'Alice' }))

  const link = throwawayLink()
  const granted = phone.grantLink(link.qr)
  await onPhone.linkDevice(granted.certificate, granted.link, granted.grant)
  const tablet = link.accept((await connect(client).takeLinkGrant(link.link)) ?? new Uint8Array())
  devices.push(tablet)
  const onTablet = await signedIn(client, tablet)
  return { name, account, phone, onPhone, tablet, onTablet }
}

/** What the server still keeps under a name: account, devices, sessions, KeyPackages, profile, reservation. */
async function traces(name: string, deviceIds: Uint8Array[]) {
  const ids = JSON.stringify(deviceIds.map((id) => Buffer.from(id).toString('hex')))
  const [row] = await (await database()).query<Record<string, string>>(
    `SELECT
       (SELECT count(*) FROM accounts WHERE name = $1)::text AS accounts,
       (SELECT count(*) FROM devices WHERE encode(id, 'hex') IN (SELECT jsonb_array_elements_text($2::jsonb)))::text AS devices,
       (SELECT count(*) FROM sessions WHERE encode(device_id, 'hex') IN (SELECT jsonb_array_elements_text($2::jsonb)))::text AS sessions,
       (SELECT count(*) FROM key_packages WHERE encode(device_id, 'hex') IN (SELECT jsonb_array_elements_text($2::jsonb)))::text AS key_packages,
       (SELECT count(*) FROM profiles p JOIN accounts a ON a.id = p.account_id WHERE a.name = $1)::text AS profiles,
       (SELECT count(*) FROM reserved_names WHERE name = $1)::text AS reserved`,
    [name, ids],
  )
  return row
}

describe('deleting an account', () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    const names = JSON.stringify(created.splice(0))
    const db = await database()
    await db.execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [names])
    await db.execute('DELETE FROM reserved_names WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [names])
  })

  test('the phone deletes the account with its key: nothing is left but the reserved name', async ({ client }) => {
    const { name, phone, onPhone, tablet, onTablet } = await alice(client)
    expect(await traces(name, [phone.deviceId, tablet.deviceId])).toEqual({
      accounts: '1',
      devices: '2',
      sessions: '2',
      key_packages: '101',
      profiles: '1',
      reserved: '0',
    })

    const heard: AccountDeleted[] = []
    const unsubscribe = emitter.on(AccountDeleted, (event: AccountDeleted) => void heard.push(event))
    try {
      expect(await onPhone.deleteAccount(phone.signAccountDeletion())).toEqual({ deleted: true })
    } finally {
      unsubscribe()
    }
    expect(heard.length).toBe(1)
    expect(await traces(name, [phone.deviceId, tablet.deviceId])).toEqual({
      accounts: '0',
      devices: '0',
      sessions: '0',
      key_packages: '0',
      profiles: '0',
      reserved: '1',
    })
    expect(await failure(onTablet.session())).toEqual({ code: -32001, message: 'E_UNAUTHENTICATED' })
  })

  test('the name stays reserved for a while, then is free', async ({ client }) => {
    const { name, account, phone, onPhone } = await alice(client)
    await onPhone.deleteAccount(phone.signAccountDeletion())

    // Someone else, with another key, wants the name at once: refused.
    const usurper = throwawayDevice(account)
    devices.push(usurper)
    expect(await failure(connect(client).createAccount(usurper.certificate))).toEqual({ code: -32009, message: 'E_IDENTITY_NAME_TAKEN' })

    // The reservation is over: the name is free, and the reservation is gone.
    await (await database()).execute('UPDATE reserved_names SET reserved_until = current_date WHERE name = $1', [name])
    expect(await connect(client).createAccount(usurper.certificate)).toEqual({ account })
    expect((await traces(name, []))?.reserved).toBe('0')
  })

  test('the deletion takes the account key: a device without it, or a signature for another account, is refused', async ({ client }) => {
    const first = await alice(client)
    const second = await alice(client)
    const invalid = { code: -32602, message: 'E_IDENTITY_INVALID_DELETION' }
    // The tablet has no account key: whatever it sends does not verify.
    expect(await failure(first.onTablet.deleteAccount(new Uint8Array(64)))).toEqual(invalid)
    // Another account's deletion, sent from this account's phone.
    expect(await failure(first.onPhone.deleteAccount(second.phone.signAccountDeletion()))).toEqual(invalid)
    expect((await traces(first.name, [first.phone.deviceId]))?.accounts).toBe('1')
    expect((await traces(second.name, [second.phone.deviceId]))?.accounts).toBe('1')
  })
})
