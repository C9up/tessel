import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'
import { app, connect, deleteAccounts, fresh } from './messengerApp.js'

/**
 * A conversation between two people (story 05-01), end to end: Alice, on her phone and
 * her tablet, writes to Bob; every device of both accounts gets every message, over the
 * real server, through the messenger web and mobile share.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

describe('a conversation between two people', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    await deleteAccounts(created.splice(0))
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
