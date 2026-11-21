import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'
import { type ApiClient, app, connect, deleteAccounts, fresh } from './messengerApp.js'

/**
 * Delivery and read receipts (story 05-04), end to end over the real server: encrypted
 * messages like any other, one for a batch of envelopes, and read receipts only while
 * the reader keeps them on.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

async function person(client: ApiClient, prefix: string) {
  const name = fresh(prefix)
  created.push(name)
  const phone = throwawayDevice(`${name}@${domain}`)
  devices.push(phone)
  await connect(client).createAccount(phone.certificate)
  return { account: `${name}@${domain}`, ...(await app(client, phone)) }
}

/** The receipts among what the device processed: their kind and how many messages each covers. */
function receipts(processed: unknown[]): { kind: string; messages: number }[] {
  const found: { kind: string; messages: number }[] = []
  for (const incoming of processed) {
    if (typeof incoming !== 'object' || incoming === null || !('message' in incoming)) continue
    const message = incoming.message
    if (typeof message !== 'object' || message === null || !('body' in message)) continue
    const body = message.body
    if (typeof body !== 'object' || body === null || !('type' in body) || body.type !== 'receipt') continue
    const kind = 'kind' in body ? String(body.kind) : ''
    const messages = 'messages' in body && Array.isArray(body.messages) ? body.messages.length : 0
    found.push({ kind, messages })
  }
  return found
}

describe('receipts', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    await deleteAccounts(created.splice(0))
  })

  test('delivered once for a batch, read when read, and no read while turned off', async ({ client }) => {
    const alice = await person(client, 'alice')
    const bob = await person(client, 'bob')
    const group = await alice.messenger.start(bob.account)
    await alice.messenger.write(group, 'one')
    await alice.messenger.write(group, 'two')
    await alice.messenger.write(group, 'three')

    // Bob's device gets the three and owes one delivery for them all.
    await bob.inbox.drain()
    await alice.inbox.drain()
    expect(receipts(alice.processed)).toEqual([{ kind: 'delivered', messages: 3 }])

    // He reads them.
    await bob.messenger.markRead(group)
    await alice.inbox.drain()
    expect(receipts(alice.processed).at(-1)).toEqual({ kind: 'read', messages: 3 })

    // He turns read receipts off: his device still says it got the next one, not that he read it.
    await bob.messenger.setReadReceipts(false)
    await alice.messenger.write(group, 'four')
    await bob.inbox.drain()
    await bob.messenger.markRead(group)
    await alice.inbox.drain()
    expect(receipts(alice.processed).slice(2)).toEqual([{ kind: 'delivered', messages: 1 }])
    expect((await bob.messenger.conversations())[0]?.unread).toBe(0)
  })
})
