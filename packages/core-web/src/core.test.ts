import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { loadTesselCoreSync } from './index.ts'

test('the WASM core answers with its crate version', async () => {
  const bytes = await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url))
  const cargo = await readFile(new URL('../../../crates/tessel-core/Cargo.toml', import.meta.url), 'utf8')
  const expected = /^version = "([^"]+)"/m.exec(cargo)?.[1]
  assert.equal(loadTesselCoreSync(bytes).version(), expected)
})
