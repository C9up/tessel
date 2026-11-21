import assert from 'node:assert/strict'
import { test } from 'node:test'
import { decode, encode } from '@msgpack/msgpack'
import { mobileCore, type NativeTesselModule } from './index.ts'

/** A native module that answers like tessel-mobile (../rust), from a table of answers. */
function fake(answers: Record<string, (args: unknown[]) => unknown>): NativeTesselModule & { calls: { method: string; args: unknown }[] } {
  const calls: { method: string; args: unknown }[] = []
  return {
    calls,
    call(method, args) {
      const decoded = decode(args)
      calls.push({ method, args: decoded })
      const answer = answers[method]
      if (answer === undefined) throw new Error(`host:unknown-method\nno method ${method}`)
      const encoded = encode(answer(Array.isArray(decoded) ? decoded : []))
      return encoded.buffer.slice(encoded.byteOffset, encoded.byteOffset + encoded.byteLength)
    },
  }
}

test('calls cross as MessagePack: bytes stay bytes, nil is an absent value', async () => {
  const module = fake({
    device: () => ({ account: 'alice@tessel.example', deviceId: new Uint8Array([1, 2]) }),
    nextOutbound: () => null,
    markRead: () => null,
  })
  const core = mobileCore(module)
  const device = await core.device()
  assert.equal(device.account, 'alice@tessel.example')
  assert.deepEqual(device.deviceId, new Uint8Array([1, 2]))
  assert.equal(await core.nextOutbound(new Uint8Array([9])), undefined)
  await core.markRead(new Uint8Array([7, 7]))
  assert.deepEqual(module.calls.at(-1), { method: 'markRead', args: [new Uint8Array([7, 7])] })
})

test("the core's refusals come back as TesselErrors with their code", async () => {
  const module = fake({
    grantLink: () => {
      throw new Error('link:malformed-qr\nnot a device link QR code')
    },
  })
  const core = mobileCore(module)
  await assert.rejects(core.grantLink(new Uint8Array(3)), { name: 'TesselError', code: 'link:malformed-qr', message: 'not a device link QR code' })
  // A failure that carries no code still comes back as a TesselError.
  await assert.rejects(core.version(), { name: 'TesselError', code: 'host:unknown-method' })
})
