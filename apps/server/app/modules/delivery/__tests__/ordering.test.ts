import { createNapiConnection } from '@c9up/atlas'
import limiter from '@c9up/blackhole/services/limiter'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, fromBase64Url, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import env from '#start/env.js'

/**
 * The order of a group (story 04-02), end to end with real MLS: one commit wins each
 * epoch, the loser is refused and replays its intent once up to date, and every envelope
 * of the group takes the next sequence number.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())
const sendId = () => crypto.getRandomValues(new Uint8Array(16))

const STALE = { code: -32014, message: 'E_DELIVERY_STALE_EPOCH' }

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
  /** The last envelope it handled. */
  cursor?: string
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
  return { device, tessel: connect(client, token) }
}

/** Fetches what came for `member` and hands it to its core, as the app does. */
async function catchUp(member: Member): Promise<readonly { seq?: number }[]> {
  const { envelopes } = await member.tessel.fetch(member.cursor)
  for (const entry of envelopes) {
    const body = fromBase64Url(entry.envelope) ?? new Uint8Array()
    // A Welcome joins; anything else goes to its group.
    if (entry.seq === undefined) member.device.core.joinGroup(body)
    else member.device.core.process(body)
    member.cursor = entry.id
  }
  return envelopes
}

/** Alice and Bob in a group the server has not seen yet. */
async function pair(client: ApiClient) {
  const alice = await member(client, 'alice')
  const bob = await member(client, 'bob')
  const group: Uint8Array = alice.device.core.createGroup()
  const [keyPackage] = bob.device.refreshKeyPackages(now()).publish
  const { welcome } = alice.device.core.addDevices(group, [keyPackage])
  alice.device.core.commitAccepted(group)
  await alice.tessel.send(sendId(), welcome, [bob.device.deviceId])
  await catchUp(bob)
  return { alice, bob, group }
}

/** A commit adding `newcomer`, sent; what the server answered. */
async function addAndSend(author: Member, group: Uint8Array, newcomer: Member, id = sendId()) {
  const [keyPackage] = newcomer.device.refreshKeyPackages(now()).publish
  const recipients: Uint8Array[] = author.device.core.recipients(group)
  const { commit, welcome } = author.device.core.addDevices(group, [keyPackage])
  return { id, commit, welcome, recipients, sent: author.tessel.send(id, commit, recipients) }
}

describe('the order of a group', () => {
  beforeEach(() => limiter.clear())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('two commits at once: one wins, the other is refused, replayed once up to date, and wins the next epoch', async ({ client }) => {
    const { alice, bob, group } = await pair(client)
    const carol = await member(client, 'carol')
    const dave = await member(client, 'dave')

    const byAlice = await addAndSend(alice, group, carol)
    const byBob = await addAndSend(bob, group, dave)
    const [aliceAnswer, bobAnswer] = await Promise.allSettled([byAlice.sent, byBob.sent])
    const outcomes = [aliceAnswer.status, bobAnswer.status].sort()
    expect(outcomes).toEqual(['fulfilled', 'rejected'])

    const [winner, loser, winning, losing] = aliceAnswer.status === 'fulfilled' ? [alice, bob, byAlice, byBob] : [bob, alice, byBob, byAlice]
    const [winnerNewcomer, loserNewcomer] = winner === alice ? [carol, dave] : [dave, carol]
    expect(await failure(losing.sent)).toEqual(STALE)

    // The winner: accepted, it moves on and welcomes its newcomer.
    winner.device.core.commitAccepted(group)
    await winner.tessel.send(sendId(), winning.welcome, [winnerNewcomer.device.deviceId])
    // The loser: its commit is dropped, it processes the winner's, then replays its intent.
    loser.device.core.commitRejected(group)
    await catchUp(loser)
    const replay = loser.device.core.retryCommit(group)
    const recipients: Uint8Array[] = loser.device.core.recipients(group)
    const second = await loser.tessel.send(sendId(), replay.commit, recipients)
    expect(second.seq).toBeGreaterThan(0)
    loser.device.core.commitAccepted(group)
    await loser.tessel.send(sendId(), replay.welcome, [loserNewcomer.device.deviceId])

    // Everyone catches up: four devices, one epoch, the same for all.
    await catchUp(winner)
    await catchUp(winnerNewcomer)
    await catchUp(loserNewcomer)
    for (const each of [alice, bob, carol, dave]) {
      expect(each.device.core.members(group).length).toBe(4)
      expect(each.device.core.epoch(group)).toBe(3)
    }
  })

  test('a commit is never handed back to its author; sent again, it keeps its place', async ({ client }) => {
    const { alice, bob, group } = await pair(client)
    const carol = await member(client, 'carol')
    const first = await addAndSend(alice, group, carol)
    const placed = await first.sent
    // The answer was lost: the same commit, the same send id, the same place.
    expect(await alice.tessel.send(first.id, first.commit, first.recipients)).toEqual(placed)
    alice.device.core.commitAccepted(group)
    expect(await catchUp(alice)).toEqual([])
    expect((await catchUp(bob)).map((entry) => entry.seq)).toEqual([placed.seq])
  })

  test('a message of an epoch the group has left is refused, then sent again once up to date', async ({ client }) => {
    const { alice, bob, group } = await pair(client)
    const carol = await member(client, 'carol')
    const commit = await addAndSend(alice, group, carol)
    await commit.sent
    alice.device.core.commitAccepted(group)

    // Bob writes before he has processed Alice's commit.
    bob.device.core.queueMessage(group, { type: 'text', text: 'late' }, Date.now())
    const behind = bob.device.core.nextOutbound(group)
    expect(await failure(bob.tessel.send(behind.sendId, behind.encrypted, bob.device.core.recipients(group)))).toEqual(STALE)
    await catchUp(bob)
    // The core encrypts it again, in the new epoch.
    const current = bob.device.core.nextOutbound(group)
    expect(current.sendId).toEqual(behind.sendId)
    const sent = await bob.tessel.send(sendId(), current.encrypted, bob.device.core.recipients(group))
    // The group's second envelope: Alice's commit was the first (a Welcome has no place).
    expect(sent.seq).toBe(2)
  })

  test("a group's envelopes take consecutive sequence numbers", async ({ client }) => {
    const { alice, bob, group } = await pair(client)
    const seqs: (number | undefined)[] = []
    for (const text of ['one', 'two', 'three']) {
      alice.device.core.queueMessage(group, { type: 'text', text }, Date.now())
      const outbound = alice.device.core.nextOutbound(group)
      seqs.push((await alice.tessel.send(outbound.sendId, outbound.encrypted, [bob.device.deviceId])).seq)
      alice.device.core.outboundSent(group, outbound.sendId)
    }
    expect(seqs).toEqual([1, 2, 3])
    expect((await catchUp(bob)).map((entry) => entry.seq)).toEqual([1, 2, 3])
  })
})
