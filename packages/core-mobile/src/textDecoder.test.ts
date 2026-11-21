import assert from 'node:assert/strict'
import { randomBytes } from 'node:crypto'
import { test } from 'node:test'
import { decodeUtf8 } from './textDecoder.ts'

const reference = new TextDecoder()

test('decodes UTF-8 as the WHATWG decoder does, malformed input included', () => {
  const bom = String.fromCharCode(0xfeff)
  const cases = ['hello', 'é ✓ 漢字 😀 ', `${bom}with a BOM`, `${'x'.repeat(20_000)}😀`].map((text) => new TextEncoder().encode(text))
  cases.push(
    new Uint8Array([0xc0, 0x80]), // overlong
    new Uint8Array([0xed, 0xa0, 0x80]), // surrogate
    new Uint8Array([0xf4, 0x90, 0x80, 0x80]), // above U+10FFFF
    new Uint8Array([0xe2, 0x82]), // truncated
    new Uint8Array([0xf0, 0x9f, 0x98, 0x41]), // broken in the middle
    new Uint8Array([0xff, 0xfe, 0x80]),
  )
  for (const bytes of cases) assert.equal(decodeUtf8(bytes), reference.decode(bytes))
  for (let round = 0; round < 500; round++) {
    const bytes = new Uint8Array(randomBytes(1 + (round % 64)))
    assert.equal(decodeUtf8(bytes), reference.decode(bytes), Buffer.from(bytes).toString('hex'))
  }
})
