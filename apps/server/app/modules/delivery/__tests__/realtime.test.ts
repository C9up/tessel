import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, fromBase64Url, type TesselClient } from '@tessel/protocol'
import { deviceChannel, Inbox, listen, readSse } from '@tessel/protocol/realtime'
import databaseConfig from '#config/database.js'
import env from '#start/env.js'

/**
 * Real-time delivery (story 04-03), end to end: a device's SSE stream over relay, opened
 * with its session token; a ring on each envelope; the inbox fetching from its cursor,
 * again after a cut, without loss or duplicate.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())
const sendId = () => crypto.getRandomValues(new Uint8Array(16))
const pause = () => new Promise((resolve) => setTimeout(resolve, 20))

/** Waits, up to five seconds, for `done` to hold. */
async function until(done: () => boolean): Promise<void> {
  for (let i = 0; i < 250 && !done(); i++) await pause()
  if (!done()) throw new Error('waited in vain')
}

interface ApiRequest {
  json(body: unknown): ApiRequest
  bearerToken(token: string): ApiRequest
  send(): Promise<{ status(): number; body(): unknown }>
}

interface ApiClient {
  post(url: string): ApiRequest
  readonly port: number
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
  token: string
}

async function member(client: ApiClient, prefix: string): Promise<Member> {
  const name = fresh(prefix)
  const device = throwawayDevice(`${name}@${domain}`)
  devices.push(device)
  const anonymous = connect(client)
  await anonymous.createAccount(device.certificate)
  created.push(name)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  return { device, tessel: connect(client, token), token }
}

/** Alice writes `text` in the group, for Bob. */
async function write(alice: Member, group: Uint8Array, bob: Member, text: string): Promise<string> {
  alice.device.core.queueMessage(group, { type: 'text', text }, Date.now())
  const outbound = alice.device.core.nextOutbound(group)
  const { envelope } = await alice.tessel.send(outbound.sendId, outbound.encrypted, [bob.device.deviceId])
  alice.device.core.outboundSent(group, outbound.sendId)
  return envelope
}

describe('real-time delivery', () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('an envelope reaches a listening device at once, and after a cut it resumes from its cursor', async ({ client }) => {
    const alice = await member(client, 'alice')
    const bob = await member(client, 'bob')
    const group: Uint8Array = alice.device.core.createGroup()
    const [keyPackage] = bob.device.refreshKeyPackages(now()).publish
    const { welcome } = alice.device.core.addDevices(group, [keyPackage])
    alice.device.core.commitAccepted(group)
    await alice.tessel.send(sendId(), welcome, [bob.device.deviceId])

    // Bob's app: each envelope goes to his core, once.
    const read: string[] = []
    const handled: string[] = []
    const inbox = new Inbox(bob.tessel, async (entry) => {
      handled.push(entry.id)
      const body = fromBase64Url(entry.envelope) ?? new Uint8Array()
      if (entry.seq === undefined) bob.device.core.joinGroup(body)
      else read.push(bob.device.core.process(body).message.body.text)
    })
    const baseUrl = `http://127.0.0.1:${client.port}`
    const open = () => {
      const stop = new AbortController()
      const listening = listen({ baseUrl, token: bob.token, device: bob.device.deviceId, inbox, signal: stop.signal }).catch(() => undefined)
      return { stop, listening }
    }

    // He connects: what was waiting (the Welcome) comes first.
    let stream = open()
    await until(() => handled.length === 1)

    // Alice writes: it reaches him without his asking.
    await write(alice, group, bob, 'live')
    await until(() => read.length === 1)
    expect(read).toEqual(['live'])

    // The stream is cut; Alice writes twice meanwhile; he comes back.
    stream.stop.abort()
    await stream.listening
    await write(alice, group, bob, 'while away 1')
    await write(alice, group, bob, 'while away 2')
    stream = open()
    await until(() => read.length === 3)
    await write(alice, group, bob, 'back')
    await until(() => read.length === 4)
    expect(read).toEqual(['live', 'while away 1', 'while away 2', 'back'])
    expect(new Set(handled).size).toBe(handled.length)
    stream.stop.abort()
    await stream.listening
  })

  test('a device hears its own channel only, and a stream needs a session', async ({ client }) => {
    const bob = await member(client, 'bob')
    const carol = await member(client, 'carol')
    const baseUrl = `http://127.0.0.1:${client.port}`

    expect((await fetch(new URL('/__relay/events', baseUrl))).status).toBe(401)

    // Carol opens her stream, then asks for Bob's channel: refused.
    const stop = new AbortController()
    const stream = await fetch(new URL('/__relay/events', baseUrl), { headers: { authorization: `Bearer ${carol.token}` }, signal: stop.signal })
    // The first frame, read without leaving the loop: that would close the stream.
    const frames = readSse(stream.body ?? new ReadableStream())
    const first = await frames.next()
    const uid = first.done === true ? '' : String(JSON.parse(first.value.data).uid)
    const subscribe = (channel: string) =>
      fetch(new URL('/__relay/subscribe', baseUrl), {
        method: 'POST',
        headers: { authorization: `Bearer ${carol.token}`, 'content-type': 'application/json' },
        body: JSON.stringify({ uid, channel }),
      })
    expect((await subscribe(deviceChannel(bob.device.deviceId))).status).toBe(403)
    expect((await subscribe(deviceChannel(carol.device.deviceId))).status).toBe(204)
    stop.abort()
  })
})
