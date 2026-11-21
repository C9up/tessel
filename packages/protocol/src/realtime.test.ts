import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { Acknowledged, Envelopes, ReceivedEnvelope } from './index.ts'
import { deviceChannel, Inbox, parseRing, readSse } from './realtime.ts'

/** A mailbox holding envelopes `1`…`count()`, handed out two at a time as `delivery.fetch` does. */
function mailbox(count: () => number) {
  let fetches = 0
  const acks: string[] = []
  return {
    fetches: () => fetches,
    acks,
    client: {
      fetch: async (after?: string): Promise<Envelopes> => {
        fetches++
        const from = after === undefined ? 0 : Number(after)
        const envelopes: ReceivedEnvelope[] = []
        for (let id = from + 1; id <= Math.min(count(), from + 2); id++) envelopes.push({ id: String(id), envelope: 'AQID' })
        return { envelopes }
      },
      ack: async (through: string): Promise<Acknowledged> => {
        acks.push(through)
        return { acknowledged: 2 }
      },
    },
  }
}

test('the inbox acknowledges each batch once handled, never before', async () => {
  const { client, acks } = mailbox(() => 5)
  const inbox = new Inbox(client, async (envelope) => {
    if (envelope.id === '5') throw new Error('the app could not handle it')
  })
  await assert.rejects(inbox.drain())
  // Batches 1-2 and 3-4 were handled and acknowledged; 5 was not.
  assert.deepEqual(acks, ['2', '4'])
})

test('the inbox hands every envelope once, in order, from its cursor', async () => {
  let count = 5
  const { client } = mailbox(() => count)
  const delivered: string[] = []
  const inbox = new Inbox(client, async (envelope) => void delivered.push(envelope.id), '1')
  await inbox.drain()
  assert.deepEqual(delivered, ['2', '3', '4', '5'])
  assert.equal(inbox.cursor, '5')

  // A ring for what was already delivered fetches nothing; a new one, what came.
  await inbox.ring({ envelope: '4' })
  assert.deepEqual(delivered, ['2', '3', '4', '5'])
  count = 7
  await inbox.ring({ envelope: '7' })
  assert.deepEqual(delivered, ['2', '3', '4', '5', '6', '7'])
})

test('rings during a drain are not lost, nor delivered twice', async () => {
  let count = 2
  const { client } = mailbox(() => count)
  const delivered: string[] = []
  let ringsDuringDrain: Promise<void>[] = []
  const inbox = new Inbox(client, async (envelope) => {
    delivered.push(envelope.id)
    // Two more envelopes arrive while the first is being handled.
    if (envelope.id === '1') {
      count = 4
      ringsDuringDrain = [inbox.ring({ envelope: '3' }), inbox.ring({ envelope: '4' })]
    }
  })
  await inbox.drain()
  await Promise.all(ringsDuringDrain)
  assert.deepEqual(delivered, ['1', '2', '3', '4'])
})

test('the cursor moves only once an envelope is handled', async () => {
  const { client } = mailbox(() => 3)
  const inbox = new Inbox(client, async (envelope) => {
    if (envelope.id === '2') throw new Error('the app could not handle it')
  })
  await assert.rejects(inbox.drain())
  assert.equal(inbox.cursor, '1')
})

test('SSE frames are read across chunks, and rings parsed', async () => {
  const text = 'event: connected\ndata: {"uid":"u1"}\n\n: comment\nevent: devices/ab\ndata: {"envelope"\ndata: :"42"}\n\n'
  const bytes = new TextEncoder().encode(text)
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      for (let i = 0; i < bytes.length; i += 7) controller.enqueue(bytes.slice(i, i + 7))
      controller.close()
    },
  })
  const frames = []
  for await (const frame of readSse(body)) frames.push(frame)
  assert.deepEqual(frames, [
    { event: 'connected', data: '{"uid":"u1"}' },
    { event: 'devices/ab', data: '{"envelope"\n:"42"}' },
  ])
  assert.deepEqual(parseRing({ envelope: '42' }), { envelope: '42' })
  assert.equal(parseRing({ envelope: 42 }), undefined)
  assert.equal(deviceChannel(new Uint8Array([0, 171, 255])), 'devices/00abff')
})
