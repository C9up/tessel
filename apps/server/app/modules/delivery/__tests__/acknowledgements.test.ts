import { createNapiConnection } from '@c9up/atlas'
import { DateTime } from '@c9up/chronos'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { SqlMailboxes } from '#modules/delivery/infrastructure/SqlMailboxes.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * Acknowledgements and purge (story 04-04): an envelope's content goes once every
 * recipient acknowledged it; what is never acknowledged goes after the retention.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
const now = () => Math.floor(DateTime.now().toSeconds())

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

/** What the server keeps of an envelope, and for how many devices. */
async function kept(id: string) {
  const [row] = await (await database()).query<{ body: boolean; header: boolean; dedup: boolean; waiting: string }>(
    `SELECT body IS NOT NULL AS body, (kind IS NOT NULL OR group_id IS NOT NULL OR epoch IS NOT NULL) AS header,
            (sender_device IS NOT NULL AND send_id IS NOT NULL) AS dedup,
            (SELECT count(*) FROM deliveries d WHERE d.envelope_id = e.id)::text AS waiting
       FROM envelopes e WHERE id = $1::bigint`,
    [id],
  )
  return row
}

/** Alice in a group with Bob and Carol, then her message to both. */
async function threeAndAMessage(client: ApiClient) {
  const alice = await member(client, 'alice')
  const bob = await member(client, 'bob')
  const carol = await member(client, 'carol')
  const group: Uint8Array = alice.device.core.createGroup()
  const keyPackages = [bob, carol].map((each) => each.device.refreshKeyPackages(now()).publish[0])
  alice.device.core.addDevices(group, keyPackages)
  alice.device.core.commitAccepted(group)
  alice.device.core.queueMessage(group, { type: 'text', text: 'to both' }, Date.now())
  const outbound = alice.device.core.nextOutbound(group)
  const recipients: Uint8Array[] = alice.device.core.recipients(group)
  const { envelope } = await alice.tessel.send(outbound.sendId, outbound.encrypted, recipients)
  return { alice, bob, carol, outbound, recipients, envelope }
}

describe('acknowledgements and purge', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test("an envelope's content goes once every recipient acknowledged it; its deduplication key stays", async ({ client }) => {
    const { alice, bob, carol, outbound, recipients, envelope } = await threeAndAMessage(client)
    expect(await kept(envelope)).toEqual({ body: true, header: true, dedup: true, waiting: '2' })

    // Bob acknowledges: Carol still waits for it.
    expect(await bob.tessel.ack(envelope)).toEqual({ acknowledged: 1 })
    expect((await bob.tessel.fetch()).envelopes).toEqual([])
    expect(await kept(envelope)).toEqual({ body: true, header: true, dedup: true, waiting: '1' })

    // Carol acknowledges: nothing of the message stays, but its key.
    expect(await carol.tessel.ack(envelope)).toEqual({ acknowledged: 1 })
    expect(await kept(envelope)).toEqual({ body: false, header: false, dedup: true, waiting: '0' })

    // Alice sends it again (an answer was lost): answered, not stored, not handed again.
    expect(await alice.tessel.send(outbound.sendId, outbound.encrypted, recipients)).toEqual({ envelope, seq: 1 })
    expect((await bob.tessel.fetch()).envelopes).toEqual([])
    expect((await carol.tessel.fetch()).envelopes).toEqual([])
  })

  test('a device acknowledges its own envelopes only, and only up to where it says', async ({ client }) => {
    const { bob, carol, envelope } = await threeAndAMessage(client)
    // Before the envelope: nothing of it is acknowledged.
    expect(await bob.tessel.ack(String(BigInt(envelope) - 1n))).toEqual({ acknowledged: 0 })
    expect((await bob.tessel.fetch()).envelopes.map((entry) => entry.id)).toEqual([envelope])
    // Bob acknowledges far beyond: Carol's own delivery stays.
    await bob.tessel.ack('9223372036854775807')
    expect((await carol.tessel.fetch()).envelopes.map((entry) => entry.id)).toEqual([envelope])
  })

  test('what is never acknowledged goes after the retention period, and only that', async ({ client }) => {
    const { bob, envelope } = await threeAndAMessage(client)
    const db = await database()
    const mailboxes = new SqlMailboxes(db)
    // Within the retention: it stays.
    await mailboxes.purge(30)
    expect((await kept(envelope))?.waiting).toBe('2')
    // Kept longer than that: it goes, with its deliveries.
    await db.execute(`UPDATE envelopes SET created_on = current_date - 31 WHERE id = $1::bigint`, [envelope])
    expect(await mailboxes.purge(30)).toBeGreaterThan(0)
    expect(await kept(envelope)).toBe(undefined)
    expect((await bob.tessel.fetch()).envelopes).toEqual([])
  })
})
