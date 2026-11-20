import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { loadTesselCoreSync } from './index.ts'

const bytes = await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url))
const core = loadTesselCoreSync(bytes)

test('the WASM core answers with its crate version', async () => {
  const cargo = await readFile(new URL('../../../crates/tessel-core/Cargo.toml', import.meta.url), 'utf8')
  const expected = /^version = "([^"]+)"/m.exec(cargo)?.[1]
  assert.equal(core.version(), expected)
})

test('the WASM core speaks the mandatory MLS ciphersuite', () => {
  assert.equal(core.ciphersuite(), 'MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519')
})
