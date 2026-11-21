import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  createTesselClient,
  fetchTransport,
  fromBase64Url,
  ProtocolError,
  parseAccountCreated,
  parseChallenge,
  parsePong,
  parseSession,
  parseSignedIn,
  parseSignedOut,
  toBase64Url,
} from './index.ts'

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

test('bytes travel as base64url both ways, and anything else is refused', () => {
  const bytes = Uint8Array.from([0, 1, 2, 250, 251, 252, 253, 254, 255])
  const text = toBase64Url(bytes)
  assert.match(text, /^[A-Za-z0-9_-]+$/)
  assert.deepEqual(fromBase64Url(text), bytes)
  assert.deepEqual(fromBase64Url(''), new Uint8Array())
  assert.equal(fromBase64Url('not base64!'), undefined)
  assert.equal(fromBase64Url('a'), undefined)
})

test('an account creation answer is checked', () => {
  assert.deepEqual(parseAccountCreated({ account: 'alice@tessel.example' }), { account: 'alice@tessel.example' })
  assert.throws(() => parseAccountCreated({ account: 42 }), ProtocolError)
})

test('sign-in answers are checked', () => {
  const challenge = toBase64Url(new Uint8Array(32).fill(1))
  assert.deepEqual(parseChallenge({ challenge }), { challenge })
  assert.throws(() => parseChallenge({ challenge: toBase64Url(new Uint8Array(16)) }), ProtocolError)
  assert.deepEqual(parseSignedIn({ token: 'tsn_abc.def' }), { token: 'tsn_abc.def' })
  assert.throws(() => parseSignedIn({ token: '' }), ProtocolError)
  assert.deepEqual(parseSession({ account: 'alice@tessel.example', device: 'AAAA' }), { account: 'alice@tessel.example', device: 'AAAA' })
  assert.throws(() => parseSession({ account: 'alice@tessel.example' }), ProtocolError)
  assert.deepEqual(parseSignedOut({ signedOut: true }), { signedOut: true })
  assert.throws(() => parseSignedOut({}), ProtocolError)
})

test('the fetch transport sends the session token in a header, never in the body', async () => {
  let token: string | undefined
  const seen: Array<{ authorization: string | null; body: string }> = []
  const transport = fetchTransport('https://tessel.example', {
    token: () => token,
    fetch: async (_url, init) => {
      seen.push({ authorization: new Headers(init?.headers).get('authorization'), body: String(init?.body) })
      return Response.json({ jsonrpc: '2.0', id: 1, result: { pong: true, protocol: 1 } })
    },
  })
  await transport('/rpc', { jsonrpc: '2.0', id: 1, method: 'system.ping' }, {})
  token = 'tsn_abc.def'
  await transport('/rpc', { jsonrpc: '2.0', id: 1, method: 'system.ping' }, {})
  assert.equal(seen[0]?.authorization, null)
  assert.equal(seen[1]?.authorization, 'Bearer tsn_abc.def')
  assert.ok(!seen[1]?.body.includes('tsn_'))
})
