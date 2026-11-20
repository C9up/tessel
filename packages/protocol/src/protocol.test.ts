import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createTesselClient, ProtocolError, parsePong } from './index.ts'

test('a pong is accepted, anything else is refused', () => {
  assert.deepEqual(parsePong({ pong: true, protocol: 1 }), { pong: true, protocol: 1 })
  assert.throws(() => parsePong({ pong: 'yes', protocol: 1 }), ProtocolError)
  assert.throws(() => parsePong(null), ProtocolError)
})

test('the client sends a JSON-RPC 2.0 envelope to /rpc', async () => {
  const sent: Array<{ url: string; body: unknown }> = []
  const client = createTesselClient(async (url, body) => {
    sent.push({ url, body })
    const id = typeof body === 'object' && body !== null && 'id' in body ? body.id : null
    return { jsonrpc: '2.0', id, result: { pong: true, protocol: 1 } }
  })
  assert.deepEqual(await client.ping(), { pong: true, protocol: 1 })
  assert.equal(sent.length, 1)
  const [call] = sent
  assert.equal(call?.url, '/rpc')
  const body = call?.body
  assert.ok(typeof body === 'object' && body !== null && 'method' in body)
  assert.equal(body.method, 'system.ping')
})
