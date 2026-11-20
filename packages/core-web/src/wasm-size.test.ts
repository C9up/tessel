/**
 * The size of the WASM module is measured on every test run and held under the budget
 * of `wasm-size.json` (story 02-10): the page downloads it before it can do anything.
 * (Brotli would be closer to what is served, but takes seconds at its best quality.)
 */

import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { gzipSync } from 'node:zlib'

test('the WASM module stays within its size budget', async (t) => {
  const wasm = await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url))
  const budget: { raw: number; gzip: number } = JSON.parse(await readFile(new URL('../wasm-size.json', import.meta.url), 'utf8'))
  const sizes = { raw: wasm.byteLength, gzip: gzipSync(wasm, { level: 9 }).byteLength }
  t.diagnostic(`tessel_wasm_bg.wasm: ${sizes.raw} bytes, ${sizes.gzip} gzip`)
  for (const kind of ['raw', 'gzip'] as const) {
    assert.ok(sizes[kind] <= budget[kind], `${kind}: ${sizes[kind]} bytes, over the budget of ${budget[kind]}`)
  }
})
