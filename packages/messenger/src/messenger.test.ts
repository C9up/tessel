import assert from 'node:assert/strict'
import { test } from 'node:test'
import { type EnvelopeSent, ServerError, toBase64Url } from '@tessel/protocol'
import {
  type Alert,
  type CoreDevice,
  INVITE_BATCH,
  Messenger,
  type MessengerCore,
  MessengerError,
  type MessengerServer,
  safetyGroups,
  type Trust,
} from './index.ts'

const bytes = (...values: number[]) => new Uint8Array(values)

/** A device of `account`: its identifier and its certificate are `id`. */
const member = (account: string, id: number): CoreDevice => ({ account, deviceId: bytes(id), certificate: bytes(id) })

/**
 * A core that keeps a queue of one message, and says each KeyPackage is of `owner`. Its
 * groups have one other device (2), except a group it just created; `calls` records the
 * group operations. A device's trust is `trusts` of its certificate's first byte; a
 * scanned QR matches the certificate that starts with its first byte.
 */
function core(owner: (keyPackage: Uint8Array) => string) {
  const state = { queue: [] as Uint8Array[], sent: [] as Uint8Array[], encryptions: 0, alone: false, calls: [] as string[] }
  const commit = (name: string) => {
    state.calls.push(name)
    return { commit: bytes(7), welcome: name.startsWith('add') ? bytes(8) : undefined }
  }
  const fake: MessengerCore & {
    queue: Uint8Array[]
    sent: Uint8Array[]
    calls: string[]
    incoming: unknown
    team: CoreDevice[]
    trusts: Map<number, Trust>
  } = {
    queue: state.queue,
    sent: state.sent,
    calls: state.calls,
    incoming: undefined,
    team: [],
    trusts: new Map(),
    device: () => member('alice@tessel.example', 1),
    verifyKeyPackage: (keyPackage) => ({ device: { account: owner(keyPackage), deviceId: keyPackage, certificate: keyPackage } }),
    createGroup: () => {
      state.alone = true
      return bytes(9)
    },
    addDevices: (_group, keyPackages) => commit(`add ${keyPackages.length}`),
    removeDevices: (_group, removals) => commit(`remove ${removals.map((r) => r.account).join()}`),
    configureGroup: (_group, change) => commit(`configure ${change.type}`),
    groupSettings: () => ({ name: '', admins: [], restricted: false }),
    commitPendingProposals: () => commit('commit proposals'),
    commitAccepted: () => {
      // Only an addition brings other devices in.
      if (state.calls.at(-1)?.startsWith('add')) state.alone = false
      state.calls.push('accepted')
    },
    commitRejected: () => void state.calls.push('rejected'),
    retryCommit: () => commit('retry'),
    leaveGroup: () => {
      state.calls.push('leave')
      return bytes(6)
    },
    joinGroup: (welcome) => welcome,
    members: () => fake.team,
    process: () => fake.incoming,
    recipients: () => (state.alone ? [] : [bytes(2)]),
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
    safetyNumber: (contact) => ({ digits: String(contact[0]).padStart(60, '0'), qr: contact }),
    checkScanned: (contact, scanned) => {
      if (scanned[0] !== contact[0]) throw Object.assign(new Error('safety codes differ'), { code: 'safety:mismatch' })
    },
    markVerified: (contact) => void state.calls.push(`verified ${contact[0]}`),
    unverify: (account) => void state.calls.push(`unverified ${account}`),
    trust: (device) => fake.trusts.get(device[0] ?? 0) ?? 'unverified',
    acknowledgeDevice: (device) => void state.calls.push(`acknowledged ${device[0]}`),
  }
  return fake
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

const quiet = (fake: MessengerCore, client: MessengerServer, catchUp = async () => undefined) => new Messenger(fake, client, catchUp, () => 1)

test('a named group is renamed while its creator is alone in it, then its devices are added and Welcomed', async () => {
  const fake = core((keyPackage) => (keyPackage[0] === 20 ? 'bob@tessel.example' : 'carol@tessel.example'))
  const { client, sends } = server({ claimed: { 'bob@tessel.example': [bytes(20)], 'carol@tessel.example': [bytes(40)] } })
  await quiet(fake, client).createGroup('Famille', ['bob@tessel.example', 'carol@tessel.example'])
  // The renaming goes to nobody (no other device yet); the addition goes to no one either.
  assert.deepEqual(fake.calls, ['configure rename', 'accepted', 'add 2', 'accepted'])
  assert.deepEqual(sends, [[bytes(8), bytes(20), bytes(40)]])
})

test('an invitation adds devices by batches, each with its Welcome', async () => {
  const fake = core(() => 'bob@tessel.example')
  const many = Array.from({ length: INVITE_BATCH * 2 + 5 }, (_, i) => bytes(100, i % 256, Math.floor(i / 256)))
  const { client, sends } = server({ claimed: { 'bob@tessel.example': many } })
  await quiet(fake, client).invite(bytes(9), ['bob@tessel.example'])
  assert.deepEqual(
    fake.calls.filter((call) => call.startsWith('add')),
    [`add ${INVITE_BATCH}`, `add ${INVITE_BATCH}`, 'add 5'],
  )
  // Per batch: the commit to the group's other device, then the Welcome to the batch.
  assert.equal(sends.length, 6)
  assert.deepEqual(sends[0], [bytes(7), bytes(2)])
  assert.equal(sends[1]?.length, INVITE_BATCH + 1)
})

test('a commit that lost the epoch is replayed once caught up', async () => {
  const fake = core(() => 'bob@tessel.example')
  const { client, sends } = server({ claimed: {}, refuseFirst: true })
  let caughtUp = 0
  await quiet(fake, client, async () => void caughtUp++).remove(bytes(9), 'bob@tessel.example')
  assert.equal(caughtUp, 1)
  assert.deepEqual(fake.calls, ['remove bob@tessel.example', 'rejected', 'retry', 'accepted'])
  assert.deepEqual(sends, [[bytes(7), bytes(2)]])
})

test("a member's departure is committed by the first member to receive it", async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.incoming = { type: 'proposal', group: bytes(9), sender: { account: 'bob@tessel.example', deviceId: bytes(3) } }
  const { client, sends } = server({ claimed: {} })
  await quiet(fake, client).receive({ id: '1', seq: 4, envelope: toBase64Url(bytes(6)) })
  assert.deepEqual(fake.calls, ['commit proposals', 'accepted'])
  assert.deepEqual(sends, [[bytes(7), bytes(2)]])
})

test("leaving takes this account's other devices out, then proposes this one's departure", async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.team = [member('alice@tessel.example', 1), member('alice@tessel.example', 11), member('bob@tessel.example', 2)]
  const { client, sends } = server({ claimed: {} })
  await quiet(fake, client).leave(bytes(9))
  assert.deepEqual(fake.calls, ['remove alice@tessel.example', 'accepted', 'leave'])
  assert.deepEqual(sends, [
    [bytes(7), bytes(2)],
    [bytes(6), bytes(2)],
  ])

  // Alone on this account: only the proposal.
  fake.calls.length = 0
  fake.team = fake.team.filter((member) => member.deviceId[0] !== 11)
  await quiet(fake, client).leave(bytes(9))
  assert.deepEqual(fake.calls, ['leave'])
})

test('a departure committed too late is dropped without catching up from within the inbox', async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.incoming = { type: 'proposal', group: bytes(9), sender: { account: 'bob@tessel.example', deviceId: bytes(3) } }
  const { client, sends } = server({ claimed: {}, refuseFirst: true })
  let caughtUp = 0
  await quiet(fake, client, async () => void caughtUp++).receive({ id: '1', seq: 4, envelope: toBase64Url(bytes(6)) })
  assert.equal(caughtUp, 0)
  assert.deepEqual(fake.calls, ['commit proposals', 'rejected'])
  assert.deepEqual(sends, [])
})

test('a leaving device proposes again until a commit removes it', async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.team = [member('alice@tessel.example', 1)]
  const { client, sends } = server({ claimed: {} })
  const messenger = quiet(fake, client)
  await messenger.leave(bytes(9))
  const envelope = { id: '1', seq: 4, envelope: toBase64Url(bytes(7)) }

  // A commit that left the departure out: proposed again.
  fake.incoming = { type: 'commit', group: bytes(9), removedSelf: false }
  await messenger.receive(envelope)
  assert.deepEqual(fake.calls, ['leave', 'leave'])
  // The commit that removes it: done.
  fake.incoming = { type: 'commit', group: bytes(9), removedSelf: true }
  await messenger.receive(envelope)
  fake.incoming = { type: 'commit', group: bytes(9), removedSelf: false }
  await messenger.receive(envelope)
  assert.deepEqual(fake.calls, ['leave', 'leave'])
  assert.equal(sends.length, 2)
})

/** What the messenger told, once the listeners ran. */
async function told(messenger: Messenger, act: () => Promise<void>): Promise<Alert[]> {
  const alerts: Alert[] = []
  const stop = messenger.onAlert((alert) => alerts.push(alert))
  await act()
  await new Promise((resolve) => setTimeout(resolve, 0))
  stop()
  return alerts
}

test('a safety number is cut in twelve groups of five digits', () => {
  const groups = safetyGroups('0123456789'.repeat(6))
  assert.equal(groups.length, 12)
  assert.deepEqual(groups.slice(0, 2), ['01234', '56789'])
})

test("a contact's safety number and verification go through its device in the group", async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.team = [member('alice@tessel.example', 1), member('bob@tessel.example', 20)]
  const { client } = server({ claimed: {} })
  const messenger = quiet(fake, client)
  assert.equal((await messenger.safetyNumber(bytes(9), 'bob@tessel.example')).digits.slice(-2), '20')
  assert.deepEqual(await messenger.verification(bytes(9)), [{ account: 'bob@tessel.example', trust: 'unverified' }])

  // The codes were the same on both screens.
  await messenger.markVerified(bytes(9), 'bob@tessel.example')
  assert.deepEqual(fake.calls, ['verified 20'])
  await assert.rejects(messenger.safetyNumber(bytes(9), 'carol@tessel.example'), new MessengerError('E_MESSENGER_NOT_A_MEMBER'))
})

test("scanning the contact's QR verifies it in one gesture, and a wrong QR changes nothing", async () => {
  const fake = core(() => 'bob@tessel.example')
  // Bob has a device under a key the server slipped in (21), and his own (22).
  fake.team = [member('alice@tessel.example', 1), member('bob@tessel.example', 21), member('bob@tessel.example', 22)]
  const { client } = server({ claimed: {} })
  const messenger = quiet(fake, client)
  await assert.rejects(messenger.verifyScanned(bytes(9), 'bob@tessel.example', bytes(99)), { code: 'safety:mismatch' })
  assert.deepEqual(fake.calls, [])
  await messenger.verifyScanned(bytes(9), 'bob@tessel.example', bytes(22))
  assert.deepEqual(fake.calls, ['verified 22'])
})

test("a conversation shows the worst of each contact's devices", async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.team = [member('alice@tessel.example', 1), member('bob@tessel.example', 20), member('bob@tessel.example', 21), member('carol@tessel.example', 40)]
  fake.trusts.set(20, 'verified').set(21, 'new-device').set(40, 'verified')
  const { client } = server({ claimed: {} })
  const messenger = quiet(fake, client)
  assert.deepEqual(await messenger.verification(bytes(9)), [
    { account: 'bob@tessel.example', trust: 'new-device' },
    { account: 'carol@tessel.example', trust: 'verified' },
  ])

  // Alice saw Bob's new device: only that one is acknowledged.
  await messenger.acknowledgeDevices(bytes(9), 'bob@tessel.example')
  assert.deepEqual(fake.calls, ['acknowledged 21'])
})

test('a commit received tells of a changed safety number, new devices and removed verified ones', async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.trusts.set(20, 'key-changed').set(30, 'new-device').set(31, 'new-device').set(40, 'unverified')
  const { client } = server({ claimed: {} })
  const messenger = quiet(fake, client)
  fake.incoming = {
    type: 'commit',
    group: bytes(9),
    removedSelf: false,
    added: [member('bob@tessel.example', 20), member('carol@tessel.example', 30), member('carol@tessel.example', 31), member('dave@tessel.example', 40)],
    verifiedRemoved: [member('erin@tessel.example', 50)],
  }
  const alerts = await told(messenger, () => messenger.receive({ id: '1', seq: 4, envelope: toBase64Url(bytes(7)) }))
  assert.deepEqual(alerts, [
    { type: 'key-changed', group: bytes(9), account: 'bob@tessel.example' },
    { type: 'new-device', group: bytes(9), account: 'carol@tessel.example', devices: [bytes(30), bytes(31)] },
    { type: 'device-removed', group: bytes(9), account: 'erin@tessel.example', devices: [bytes(50)] },
  ])
})

test('joining a group, or inviting, tells of what changed for verified contacts', async () => {
  const fake = core(() => 'bob@tessel.example')
  fake.trusts.set(20, 'key-changed')
  const { client } = server({ claimed: { 'bob@tessel.example': [bytes(20)] } })
  const messenger = quiet(fake, client)
  const invited = await told(messenger, () => messenger.invite(bytes(9), ['bob@tessel.example']))
  assert.deepEqual(invited, [{ type: 'key-changed', group: bytes(9), account: 'bob@tessel.example' }])

  fake.team = [member('alice@tessel.example', 1), member('bob@tessel.example', 20)]
  const joined = await told(messenger, () => messenger.receive({ id: '2', envelope: toBase64Url(bytes(9)) }))
  assert.deepEqual(joined, [{ type: 'key-changed', group: bytes(9), account: 'bob@tessel.example' }])
})
