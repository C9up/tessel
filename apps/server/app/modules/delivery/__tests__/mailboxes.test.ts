import { createNapiConnection } from '@c9up/atlas'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, fromBase64Url, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * The mailboxes (story 04-01), end to end: real MLS groups made by the core, envelopes
 * posted and fetched over JSON-RPC, the server reading the public header only.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())
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

interface Member {
  device: ThrowawayDevice
  tessel: TesselClient
}

/** A new account's first device, registered and signed in. */
async function member(client: ApiClient, prefix: string): Promise<Member> {
  const name = fresh(prefix)
  const device = throwawayDevice(`${name}@${domain}`)
  devices.push(device)
  const anonymous = connect(client)
  await anonymous.createAccount(device.certificate)
  created.push(name)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  return { device, tessel: connect(client, token) }
}

/** The bytes of each envelope fetched. */
async function inbox(member: Member, after?: string) {
  const { envelopes } = await member.tessel.fetch(after)
  return envelopes.map((entry) => ({ id: entry.id, body: fromBase64Url(entry.envelope) ?? new Uint8Array() }))
}

/** Alice makes a group, adds Bob, and posts his Welcome; Bob joins from his mailbox. */
async function pair(client: ApiClient) {
  const alice = await member(client, 'alice')
  const bob = await member(client, 'bob')
  const carol = await member(client, 'carol')
  const group: Uint8Array = alice.device.core.createGroup()
  const [keyPackage] = bob.device.refreshKeyPackages(now()).publish
  const { welcome } = alice.device.core.addDevices(group, [keyPackage])
  alice.device.core.commitAccepted(group)
  await alice.tessel.send(sendId(), welcome, [bob.device.deviceId])
  const [received] = await inbox(bob)
  bob.device.core.joinGroup(received?.body)
  return { alice, bob, carol, group, welcomeId: received?.id ?? '' }
}

describe('the mailboxes', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    const db = await database()
    if (created.length > 0)
      await db.execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('a message reaches the devices it is for, and only them; the server keeps an opaque body', async ({ client }) => {
    const { alice, bob, carol, group, welcomeId } = await pair(client)

    alice.device.core.queueMessage(group, { type: 'text', text: 'hello, Bob' }, Date.now())
    const outbound = alice.device.core.nextOutbound(group)
    const recipients: Uint8Array[] = alice.device.core.recipients(group)
    const { envelope } = await alice.tessel.send(outbound.sendId, outbound.encrypted, recipients)
    alice.device.core.outboundSent(group, outbound.sendId)

    const [received, ...more] = await inbox(bob, welcomeId)
    expect(more.length).toBe(0)
    expect(received?.id).toBe(envelope)
    const incoming = bob.device.core.process(received?.body)
    expect(incoming.message.body).toEqual({ type: 'text', text: 'hello, Bob' })

    // Carol is not in the group: nothing for her. Alice gets nothing back.
    expect(await inbox(carol)).toEqual([])
    expect(await inbox(alice)).toEqual([])

    // What the server keeps: the public header, and a body it cannot read.
    const [row] = await (await database()).query<{ kind: number; group_id: string; epoch: string; body: string }>(
      `SELECT kind, encode(group_id, 'hex') AS group_id, epoch::text AS epoch, encode(body, 'hex') AS body FROM envelopes WHERE id = $1::bigint`,
      [envelope],
    )
    expect(row?.kind).toBe(1)
    expect(row?.group_id).toBe(Buffer.from(group).toString('hex'))
    expect(row?.epoch).toBe('1')
    expect(row?.body.includes(Buffer.from('hello, Bob').toString('hex'))).toBe(false)
  })

  test('the same envelope sent again is kept once, and the same identifier comes back', async ({ client }) => {
    const { alice, bob, group, welcomeId } = await pair(client)
    alice.device.core.queueMessage(group, { type: 'text', text: 'once' }, Date.now())
    const outbound = alice.device.core.nextOutbound(group)
    const recipients: Uint8Array[] = alice.device.core.recipients(group)
    const first = await alice.tessel.send(outbound.sendId, outbound.encrypted, recipients)
    // The acknowledgement was lost: the core encrypts it again, under the same send id.
    const again = alice.device.core.nextOutbound(group)
    expect(again.sendId).toEqual(outbound.sendId)
    expect(await alice.tessel.send(again.sendId, again.encrypted, recipients)).toEqual(first)
    expect((await inbox(bob, welcomeId)).map((entry) => entry.id)).toEqual([first.envelope])
  })

  test('envelopes come in order, after the last one handled', async ({ client }) => {
    const { alice, bob, group, welcomeId } = await pair(client)
    const sent: string[] = []
    for (const text of ['one', 'two', 'three']) {
      alice.device.core.queueMessage(group, { type: 'text', text }, Date.now())
      const outbound = alice.device.core.nextOutbound(group)
      sent.push((await alice.tessel.send(outbound.sendId, outbound.encrypted, [bob.device.deviceId])).envelope)
      alice.device.core.outboundSent(group, outbound.sendId)
    }
    expect((await inbox(bob, welcomeId)).map((entry) => entry.id)).toEqual(sent)
    expect((await inbox(bob, sent[0])).map((entry) => entry.id)).toEqual(sent.slice(1))
  })

  test('what is not a group message, or for nobody, is refused; a session is needed', async ({ client }) => {
    const { alice, bob, group } = await pair(client)
    const invalid = { code: -32602, message: 'E_DELIVERY_INVALID_ENVELOPE' }
    expect(await failure(alice.tessel.send(sendId(), new Uint8Array([1, 2, 3]), [bob.device.deviceId]))).toEqual(invalid)
    // A KeyPackage goes to the directory, not to a mailbox.
    const [keyPackage] = alice.device.refreshKeyPackages(now()).publish
    expect(await failure(alice.tessel.send(sendId(), keyPackage ?? new Uint8Array(), [bob.device.deviceId]))).toEqual(invalid)

    alice.device.core.queueMessage(group, { type: 'text', text: 'to myself' }, Date.now())
    const outbound = alice.device.core.nextOutbound(group)
    expect(await failure(alice.tessel.send(outbound.sendId, outbound.encrypted, [alice.device.deviceId]))).toEqual({
      code: -32602,
      message: 'E_DELIVERY_NO_RECIPIENT',
    })
    expect(await failure(connect(client).fetch())).toEqual({ code: -32001, message: 'E_UNAUTHENTICATED' })
  })
})
