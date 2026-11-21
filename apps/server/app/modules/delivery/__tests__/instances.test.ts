import { type ChildProcess, spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, fetchTransport, fromBase64Url, type TesselClient } from '@tessel/protocol'
import { Inbox, listen } from '@tessel/protocol/realtime'
import databaseConfig from '#config/database.js'
import env from '#start/env.js'

/**
 * Two instances of the server (story 04-05): a device listening on one hears, at once,
 * an envelope kept by the other. The ring crosses over KeyDB (relay's bus); the
 * envelope itself is in PostgreSQL, which both share.
 */

const APP_ROOT = fileURLToPath(new URL('../../../../', import.meta.url))
const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []
let other: ChildProcess | undefined

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())
const sendId = () => crypto.getRandomValues(new Uint8Array(16))
const pause = () => new Promise((resolve) => setTimeout(resolve, 20))

async function until(done: () => boolean): Promise<void> {
  for (let i = 0; i < 250 && !done(); i++) await pause()
  if (!done()) throw new Error('waited in vain')
}

/** Starts the other instance in a process of its own; its base URL. */
function startOtherInstance(): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ['--import', '@swc-node/register/esm-register', 'tests/instance.ts'], {
      cwd: APP_ROOT,
      env: { ...process.env, NODE_ENV: 'test' },
      stdio: ['ignore', 'pipe', 'inherit'],
    })
    other = child
    let output = ''
    child.stdout?.on('data', (chunk: Buffer) => {
      output += chunk.toString()
      const port = /INSTANCE_PORT=(\d+)/.exec(output)?.[1]
      if (port !== undefined) resolve(`http://127.0.0.1:${port}`)
    })
    child.on('exit', (code) => reject(new Error(`the other instance exited (${code})`)))
  })
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

describe('several instances of the server', () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    other?.kill('SIGTERM')
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('a device connected to one instance hears at once an envelope sent through the other', async ({ client }) => {
    const elsewhere = await startOtherInstance()
    const alice = await member(client, 'alice')
    const bob = await member(client, 'bob')
    const group: Uint8Array = alice.device.core.createGroup()
    const [keyPackage] = bob.device.refreshKeyPackages(now()).publish
    const { welcome } = alice.device.core.addDevices(group, [keyPackage])
    alice.device.core.commitAccepted(group)
    await alice.tessel.send(sendId(), welcome, [bob.device.deviceId])

    // Bob is on the other instance, for his stream and his fetches alike.
    const onOther = createTesselClient(fetchTransport(elsewhere, { token: () => bob.token }))
    const read: string[] = []
    let handled = 0
    const inbox = new Inbox(onOther, async (entry) => {
      handled++
      const body = fromBase64Url(entry.envelope) ?? new Uint8Array()
      if (entry.seq === undefined) bob.device.core.joinGroup(body)
      else read.push(bob.device.core.process(body).message.body.text)
    })
    const stop = new AbortController()
    const listening = listen({ baseUrl: elsewhere, token: bob.token, device: bob.device.deviceId, inbox, signal: stop.signal }).catch(() => undefined)
    await until(() => handled === 1)

    // Alice writes through this instance: the ring reaches Bob over the bus.
    alice.device.core.queueMessage(group, { type: 'text', text: 'across instances' }, Date.now())
    const outbound = alice.device.core.nextOutbound(group)
    await alice.tessel.send(outbound.sendId, outbound.encrypted, [bob.device.deviceId])
    await until(() => read.length === 1)
    expect(read).toEqual(['across instances'])

    stop.abort()
    await listening
  })
})
