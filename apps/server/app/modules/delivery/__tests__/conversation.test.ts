import { createNapiConnection } from '@c9up/atlas'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { checkKeyPackage, type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { Messenger, type MessengerCore } from '@tessel/messenger'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import { Inbox } from '@tessel/protocol/realtime'
import databaseConfig from '#config/database.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * A conversation between two people (story 05-01), end to end: Alice, on her phone and
 * her tablet, writes to Bob; every device of both accounts gets every message, over the
 * real server, through the messenger web and mobile share.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())
const clock = () => DateTime.now().toMillis()

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

/** A message the core read: its text, if it is one. */
function textOf(incoming: unknown): string | undefined {
  if (typeof incoming !== 'object' || incoming === null || !('message' in incoming)) return undefined
  const message = incoming.message
  if (typeof message !== 'object' || message === null || !('body' in message)) return undefined
  const body = message.body
  return typeof body === 'object' && body !== null && 'text' in body && typeof body.text === 'string' ? body.text : undefined
}

/**
 * The core of a test device, as the messenger asks for it. The texts it writes and
 * reads go to `seen`, in order: a test device keeps no database to read its history
 * back from.
 */
function coreOf(device: ThrowawayDevice, seen: string[]): MessengerCore {
  const core = device.core
  return {
    device: () => core.device(),
    verifyKeyPackage: (keyPackage) => {
      const checked = checkKeyPackage(keyPackage)
      if (checked === undefined) throw new Error('not a valid KeyPackage')
      return checked
    },
    createGroup: () => core.createGroup(),
    addDevices: (group, keyPackages) => core.addDevices(group, keyPackages),
    commitAccepted: (group) => core.commitAccepted(group),
    joinGroup: (welcome) => core.joinGroup(welcome),
    process: (message) => {
      const incoming = core.process(message)
      const text = textOf(incoming)
      if (text !== undefined) seen.push(text)
      return incoming
    },
    recipients: (group) => core.recipients(group),
    queueMessage: (group, body, sentAt) => {
      seen.push(body.text)
      return core.queueMessage(group, body, sentAt)
    },
    nextOutbound: (group) => core.nextOutbound(group),
    outboundSent: (group, sendId) => core.outboundSent(group, sendId),
    conversations: () => core.conversations(),
    markRead: (group) => core.markRead(group),
  }
}

/** A device signed in, its KeyPackages in the directory, its messenger and its inbox. */
async function app(client: ApiClient, device: ThrowawayDevice) {
  const anonymous = connect(client)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  const tessel = connect(client, token)
  await tessel.publishKeyPackages(device.refreshKeyPackages(now()))
  const holder: { messenger?: Messenger } = {}
  const inbox = new Inbox(tessel, async (envelope) => holder.messenger?.receive(envelope))
  const seen: string[] = []
  const messenger = new Messenger(coreOf(device, seen), tessel, () => inbox.drain(), clock)
  holder.messenger = messenger
  return { device, tessel, messenger, inbox, texts: () => [...seen] }
}

describe('a conversation between two people', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('messages reach every device of both accounts; the list counts what is unread', async ({ client }) => {
    // Alice: a phone, and a tablet linked to it. Bob: a phone.
    const alice = fresh('alice')
    const bob = fresh('bob')
    created.push(alice, bob)
    const alicePhone = throwawayDevice(`${alice}@${domain}`)
    const bobPhone = throwawayDevice(`${bob}@${domain}`)
    devices.push(alicePhone, bobPhone)
    await connect(client).createAccount(alicePhone.certificate)
    await connect(client).createAccount(bobPhone.certificate)
    const onAlicePhone = await app(client, alicePhone)
    const link = throwawayLink()
    const granted = alicePhone.grantLink(link.qr)
    await onAlicePhone.tessel.linkDevice(granted.certificate, granted.link, granted.grant)
    const aliceTablet = link.accept((await connect(client).takeLinkGrant(link.link)) ?? new Uint8Array())
    devices.push(aliceTablet)
    const onAliceTablet = await app(client, aliceTablet)
    const onBobPhone = await app(client, bobPhone)

    // Alice starts the conversation from her phone, and writes.
    const group = await onAlicePhone.messenger.start(`${bob}@${domain}`)
    await onAlicePhone.messenger.write(group, 'hello Bob')

    // Bob and Alice's tablet get it; Bob answers.
    await onBobPhone.inbox.drain()
    await onAliceTablet.inbox.drain()
    expect(onBobPhone.texts()).toEqual(['hello Bob'])
    expect(onAliceTablet.texts()).toEqual(['hello Bob'])
    await onBobPhone.messenger.write(group, 'hello Alice')

    // Both of Alice's devices get the answer.
    await onAlicePhone.inbox.drain()
    await onAliceTablet.inbox.drain()
    for (const each of [onAlicePhone, onAliceTablet, onBobPhone]) {
      await each.inbox.drain()
      expect(each.texts()).toEqual(['hello Bob', 'hello Alice'])
    }

    // The list: on Alice's tablet, one unread, Bob's (the first message is Alice's own,
    // written on her phone); on Bob's phone, one unread, Alice's, until he reads it.
    const [onTablet] = await onAliceTablet.messenger.conversations()
    expect(onTablet?.unread).toBe(1)
    const [onBob] = await onBobPhone.messenger.conversations()
    expect(onBob?.unread).toBe(1)
    await onBobPhone.messenger.markRead(group)
    expect((await onBobPhone.messenger.conversations())[0]?.unread).toBe(0)
  })
})
