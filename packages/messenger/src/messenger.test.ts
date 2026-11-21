import assert from 'node:assert/strict'
import { test } from 'node:test'
import { type EnvelopeSent, ServerError, toBase64Url } from '@tessel/protocol'
import { Messenger, type MessengerCore, MessengerError, type MessengerServer } from './index.ts'

const bytes = (...values: number[]) => new Uint8Array(values)

/** A core that keeps a queue of one message, and says each KeyPackage is of `owner`. */
function core(owner: (keyPackage: Uint8Array) => string): MessengerCore & { queue: Uint8Array[]; sent: Uint8Array[] } {
  const state = { queue: [] as Uint8Array[], sent: [] as Uint8Array[], encryptions: 0 }
  return {
    ...state,
    queue: state.queue,
    sent: state.sent,
    device: () => ({ account: 'alice@tessel.example', deviceId: bytes(1) }),
    verifyKeyPackage: (keyPackage) => ({ device: { account: owner(keyPackage), deviceId: keyPackage } }),
    createGroup: () => bytes(9),
    addDevices: () => ({ commit: bytes(7), welcome: bytes(8) }),
    commitAccepted: () => undefined,
    joinGroup: (welcome) => welcome,
    process: () => undefined,
    recipients: () => [bytes(2)],
    queueMessage: () => {
      state.queue.push(bytes(5))
    },
    nextOutbound: () => {
      const head = state.queue[0]
      // Each call encrypts anew, under the same send identifier.
      state.encryptions++
      return head === undefined ? undefined : { sendId: head, encrypted: bytes(state.encryptions) }
    },
    outboundSent: () => {
      state.sent.push(state.queue.shift() ?? bytes())
    },
    conversations: () => [],
    markRead: () => undefined,
  }
}

function server(options: { claimed: Record<string, Uint8Array[]>; refuseFirst?: boolean }) {
  const sends: Uint8Array[][] = []
  let refused = false
  const client: MessengerServer = {
    claimKeyPackages: async (account) => ({
      keyPackages: (options.claimed[account] ?? []).map((keyPackage) => ({ device: toBase64Url(keyPackage), keyPackage: toBase64Url(keyPackage) })),
    }),
    send: async (_sendId, envelope, recipients): Promise<EnvelopeSent> => {
      if (options.refuseFirst === true && !refused) {
        refused = true
        throw Object.assign(new Error('E_DELIVERY_STALE_EPOCH'), { code: ServerError.E_DELIVERY_STALE_EPOCH })
      }
      sends.push([envelope, ...recipients])
      return { envelope: String(sends.length), seq: sends.length }
    },
    fetch: async () => ({ envelopes: [] }),
  }
  return { client, sends }
}

test("a conversation takes the contact's devices and this account's other devices, and Welcomes them", async () => {
  const fake = core((keyPackage) => (keyPackage[0] === 20 ? 'bob@tessel.example' : 'alice@tessel.example'))
  const { client, sends } = server({ claimed: { 'bob@tessel.example': [bytes(20)], 'alice@tessel.example': [bytes(30)] } })
  const messenger = new Messenger(
    fake,
    client,
    async () => undefined,
    () => 1,
  )
  assert.deepEqual(await messenger.start('bob@tessel.example'), bytes(9))
  // The Welcome, to Bob's device and Alice's tablet; the commit to nobody.
  assert.deepEqual(sends, [[bytes(8), bytes(20), bytes(30)]])
})

test('a KeyPackage of another account than the one asked for stops everything', async () => {
  const fake = core(() => 'mallory@tessel.example')
  const { client, sends } = server({ claimed: { 'bob@tessel.example': [bytes(20)] } })
  const messenger = new Messenger(
    fake,
    client,
    async () => undefined,
    () => 1,
  )
  await assert.rejects(messenger.start('bob@tessel.example'), new MessengerError('E_MESSENGER_WRONG_KEY_PACKAGE'))
  assert.deepEqual(sends, [])
})

test('a contact without a device cannot be written to', async () => {
  const fake = core(() => 'bob@tessel.example')
  const { client } = server({ claimed: {} })
  const messenger = new Messenger(
    fake,
    client,
    async () => undefined,
    () => 1,
  )
  await assert.rejects(messenger.start('bob@tessel.example'), new MessengerError('E_MESSENGER_NO_DEVICE'))
})

test('a message refused for a past epoch is sent again once caught up, encrypted anew', async () => {
  const fake = core(() => 'bob@tessel.example')
  const { client, sends } = server({ claimed: {}, refuseFirst: true })
  let caughtUp = 0
  const messenger = new Messenger(
    fake,
    client,
    async () => void caughtUp++,
    () => 1,
  )
  await messenger.write(bytes(9), 'hello')
  assert.equal(caughtUp, 1)
  assert.equal(sends.length, 1)
  assert.equal(fake.sent.length, 1)
})
