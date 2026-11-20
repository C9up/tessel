/**
 * The contract of `TesselCore` (story 02-10): the same scenarios run on the core called
 * directly in the worker, and on the core reached through messages, as the page does.
 * The mobile binding (story 02-13) is to pass them too.
 */

import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { after, test } from 'node:test'
import { IDBFactory } from 'fake-indexeddb'
import { type DeviceInfo, outboxOf, type TesselCore, TesselError } from './api.ts'
import { CoreHost } from './host.ts'
import { type OutboundMessage, OutboxSender } from './outbox.ts'
import { RemoteCore, serve } from './rpc.ts'
import { loadWasmSync } from './wasm.ts'

loadWasmSync(await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url)))

const factory = new IDBFactory()
const cleanups: (() => void)[] = []
after(() => {
  for (const cleanup of cleanups) cleanup()
})

/** How a test reaches a profile's core. */
const transports: Record<string, (database: string) => TesselCore> = {
  direct: (database) => new CoreHost({ database, factory }),
  'through messages': (database) => {
    const { port1, port2 } = new MessageChannel()
    const stop = serve(port1, new CoreHost({ database, factory }))
    const remote = new RemoteCore(port2)
    cleanups.push(() => {
      stop()
      remote.disconnect()
      port1.close()
      port2.close()
    })
    return remote
  },
}

let profiles = 0
const encode = (text: string) => new TextEncoder().encode(text)
const now = () => Math.floor(Date.now() / 1000)

/** A new profile with an account. */
async function account(make: (database: string) => TesselCore, name: string): Promise<{ core: TesselCore; device: DeviceInfo; database: string }> {
  const database = `profile-${profiles++}`
  const core = make(database)
  assert.equal(await core.start(), undefined)
  const device = await core.createAccount(`${name}@tessel.example`)
  return { core, device, database }
}

/** Alice creates a group with Bob in it. */
async function pair(make: (database: string) => TesselCore) {
  const alice = await account(make, 'alice')
  const bob = await account(make, 'bob')
  const keyPackage = (await bob.core.refreshKeyPackages(now())).publish[0]
  assert.ok(keyPackage)
  const group = await alice.core.createGroup()
  const { welcome } = await alice.core.addDevices(group, [keyPackage])
  await alice.core.commitAccepted(group)
  assert.ok(welcome)
  assert.deepEqual(await bob.core.joinGroup(welcome), group)
  return { alice, bob, group }
}

/** Sends the head of the queue, as the relay would, and returns it. */
async function sendNext(core: TesselCore, group: Uint8Array<ArrayBuffer>): Promise<OutboundMessage> {
  const outbound = await core.nextOutbound(group)
  assert.ok(outbound)
  await core.outboundSent(group, outbound.sendId)
  return outbound
}

for (const [transport, make] of Object.entries(transports)) {
  test(`${transport}: a profile has no device until its account is created, and keeps it`, async () => {
    const { core, device, database } = await account(make, 'alice')
    const cargo = await readFile(new URL('../../../crates/tessel-core/Cargo.toml', import.meta.url), 'utf8')
    assert.equal(await core.version(), /^version = "([^"]+)"/m.exec(cargo)?.[1])
    assert.equal(await core.ciphersuite(), 'MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519')
    assert.equal(device.account, 'alice@tessel.example')
    assert.equal(device.deviceId.length, 16)
    assert.equal(await core.holdsAccountKey(), true)
    await assert.rejects(core.createAccount('alice@tessel.example'), { name: 'TesselError', code: 'host:account-exists' })
    await core.close()

    const again = make(database)
    assert.deepEqual((await again.start())?.deviceId, device.deviceId)
    assert.equal(await again.holdsAccountKey(), true)
    await again.close()
  })

  test(`${transport}: calls before start fail with a code`, async () => {
    const core = make(`profile-${profiles++}`)
    await assert.rejects(core.createGroup(), { name: 'TesselError', code: 'host:not-started' })
    await assert.rejects(core.createAccount('not an account'), { code: 'host:not-started' })
    await core.start()
    await assert.rejects(core.createAccount('not an account'), { code: 'identity:invalid-account' })
  })

  test(`${transport}: two devices write, edit and read their history`, async () => {
    const { alice, bob, group } = await pair(make)
    assert.equal((await alice.core.members(group)).length, 2)
    assert.equal(await bob.core.epoch(group), await alice.core.epoch(group))

    const queued = await alice.core.queueMessage(group, { type: 'text', text: 'hello bob' }, 1_000)
    assert.deepEqual(await alice.core.waitingMessages(group), [queued.messageId])
    const outbound = await sendNext(alice.core, group)
    assert.deepEqual(outbound.messageId, queued.messageId)
    assert.deepEqual(await alice.core.outboundGroups(), [])

    const incoming = await bob.core.process(outbound.encrypted)
    assert.equal(incoming.type, 'message')
    assert.ok(incoming.type === 'message')
    assert.equal(incoming.sender.account, 'alice@tessel.example')
    assert.equal(incoming.effect, 'added')
    assert.deepEqual(incoming.message?.body, { type: 'text', text: 'hello bob' })

    await alice.core.queueMessage(group, { type: 'edit', target: queued.messageId, text: 'hello, bob' }, 2_000)
    assert.equal((await bob.core.process((await sendNext(alice.core, group)).encrypted)).type, 'message')
    for (const core of [alice.core, bob.core]) {
      const [entry, ...rest] = await core.history(group)
      assert.equal(rest.length, 0)
      assert.equal(entry?.author, 'alice@tessel.example')
      assert.deepEqual(entry?.content, { type: 'text', text: 'hello, bob', editedAt: 2_000 })
    }

    // Bob cannot edit Alice's message: refused before anything is sent.
    await assert.rejects(bob.core.queueMessage(group, { type: 'edit', target: queued.messageId, text: 'bye' }, 3_000), {
      code: 'group:ignored:not-author',
    })
    await assert.rejects(bob.core.process(encode('not mls')), { code: 'group:malformed' })
    await assert.rejects(bob.core.commitAccepted(group), (error: unknown) => error instanceof TesselError && error.code === 'group:no-pending-commit')
  })

  test(`${transport}: a media goes through encrypted, its key sealed in the core`, async () => {
    const { alice, bob, group } = await pair(make)
    const file = new Uint8Array(200_000).map((_, i) => i % 251)
    const { encrypted, secret, size } = await alice.core.encryptMedia(file)
    assert.equal(size, file.length)
    assert.notDeepEqual(encrypted.subarray(0, 1000), file.subarray(0, 1000))

    const blob = encode('blob-1')
    await alice.core.queueMessage(group, { type: 'media', media: { secret, blob, contentType: 'image/png', name: 'cat.png' }, caption: 'a cat' }, 1_000)
    const incoming = await bob.core.process((await sendNext(alice.core, group)).encrypted)
    assert.ok(incoming.type === 'message' && incoming.message?.body.type === 'media')
    const { media, caption } = incoming.message.body
    assert.equal(caption, 'a cat')
    assert.deepEqual([media.blob, media.size, media.contentType, media.name], [blob, file.length, 'image/png', 'cat.png'])
    assert.deepEqual(await bob.core.decryptMedia(media.secret, encrypted), file)

    // A secret only opens in the core that sealed it; the history hands out new ones.
    await assert.rejects(alice.core.decryptMedia(media.secret, encrypted), { code: 'media:altered' })
    const [entry] = await bob.core.history(group)
    assert.ok(entry?.content.type === 'media')
    assert.deepEqual(await bob.core.decryptMedia(entry.content.media.secret, encrypted), file)
    const altered = Uint8Array.from(encrypted)
    altered[100] = (altered[100] ?? 0) ^ 1
    await assert.rejects(bob.core.decryptMedia(media.secret, altered), { code: 'media:altered' })
  })

  test(`${transport}: safety numbers match, and trust follows the verification`, async () => {
    const { alice, bob } = await pair(make)
    const seenByAlice = await alice.core.safetyNumber(bob.device.certificate)
    const seenByBob = await bob.core.safetyNumber(alice.device.certificate)
    assert.equal(seenByAlice.digits, seenByBob.digits)
    assert.equal(seenByAlice.digits.length, 60)
    await alice.core.checkScanned(bob.device.certificate, seenByBob.qr)
    await assert.rejects(alice.core.checkScanned(bob.device.certificate, seenByAlice.qr), { code: 'safety:mismatch' })

    assert.equal(await alice.core.trust(bob.device.certificate), 'unverified')
    await assert.rejects(alice.core.acknowledgeDevice(bob.device.certificate), { code: 'safety:not-verified' })
    await alice.core.markVerified(bob.device.certificate)
    assert.equal(await alice.core.trust(bob.device.certificate), 'verified')
    await alice.core.unverify('bob@tessel.example')
    assert.equal(await alice.core.trust(bob.device.certificate), 'unverified')
  })

  test(`${transport}: a message written offline is sent after a restart`, async () => {
    const { alice, bob, group } = await pair(make)
    const { messageId } = await alice.core.queueMessage(group, { type: 'text', text: 'written offline' }, 1_000)
    await alice.core.close()

    const restarted = make(alice.database)
    await restarted.start()
    const relayed: OutboundMessage[] = []
    const sender = new OutboxSender(outboxOf(restarted), { send: async (message) => void relayed.push(message) }, { connectivity: new EventTarget() })
    await sender.flush()
    assert.deepEqual(
      relayed.map((message) => message.messageId),
      [messageId],
    )
    assert.deepEqual(await restarted.waitingMessages(group), [])
    const [message] = relayed
    assert.ok(message)
    const incoming = await bob.core.process(message.encrypted)
    assert.ok(incoming.type === 'message')
    assert.deepEqual(incoming.message?.body, { type: 'text', text: 'written offline' })
  })
}
