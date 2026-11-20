import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { IDBFactory } from 'fake-indexeddb'
import { loadTesselCoreSync } from './index.ts'
import {
  INDEXEDDB_SCHEMA_VERSION,
  IndexedDbStore,
  loadStoreKey,
  MIGRATIONS,
  type Migration,
  openDatabase,
  type StoredRecord,
  UnsupportedSchemaError,
} from './store.ts'

const core = loadTesselCoreSync(await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url)))

const filled = (length: number, byte: number) => new Uint8Array(length).fill(byte)

function record(partition: number, id: number, value: string): StoredRecord {
  return { partition: filled(32, partition), id: filled(32, id), value: new TextEncoder().encode(value) }
}

const sorted = (records: StoredRecord[]) => [...records].sort((a, b) => (a.id[0] ?? 0) - (b.id[0] ?? 0) || (a.partition[0] ?? 0) - (b.partition[0] ?? 0))

test('a new database is at the current schema', async () => {
  const store = await IndexedDbStore.open('tessel', new IDBFactory())
  assert.equal(store.database.version, INDEXEDDB_SCHEMA_VERSION)
  assert.deepEqual([...store.database.objectStoreNames].sort(), ['host', 'records'])
  store.close()
})

test('records are written, replaced, deleted and listed by partition', async () => {
  const store = await IndexedDbStore.open('tessel', new IDBFactory())
  const [a, b, other] = [record(1, 1, 'first'), record(1, 2, 'second'), record(2, 1, 'elsewhere')]
  await store.apply({ put: [a, b, other], delete: [] })
  assert.deepEqual(sorted(await store.list(filled(32, 1))), sorted([a, b]))
  assert.deepEqual(await store.list(filled(32, 2)), [other])

  const replaced = record(1, 1, 'replaced')
  await store.apply({ put: [replaced], delete: [{ partition: b.partition, id: b.id }] })
  assert.deepEqual(await store.list(filled(32, 1)), [replaced])
  store.close()
})

test('changes are applied all together or not at all', async () => {
  const store = await IndexedDbStore.open('tessel', new IDBFactory())
  const valid = record(1, 1, 'valid')
  // A proxy is typed as bytes but is not a valid IndexedDB key: the second write fails.
  const invalid: StoredRecord = { ...record(1, 2, 'invalid'), id: new Proxy(filled(32, 2), {}) }
  await assert.rejects(store.apply({ put: [valid, invalid], delete: [] }))
  assert.deepEqual(await store.list(filled(32, 1)), [])
  store.close()
})

test('a reopened database keeps its records', async () => {
  const factory = new IDBFactory()
  const first = await IndexedDbStore.open('tessel', factory)
  const kept = record(3, 3, 'kept')
  await first.apply({ put: [kept], delete: [] })
  first.close()
  const again = await IndexedDbStore.open('tessel', factory)
  assert.deepEqual(await again.list(filled(32, 3)), [kept])
  again.close()
})

test('a database from a newer version of Tessel is refused', async () => {
  const factory = new IDBFactory()
  const newer = await openDatabase('tessel', factory, [...MIGRATIONS, () => {}])
  newer.close()
  await assert.rejects(IndexedDbStore.open('tessel', factory), UnsupportedSchemaError)
})

test('migrations run in order from the stored version, and a failing one changes nothing', async () => {
  const factory = new IDBFactory()
  const steps: Migration[] = [
    (db) => {
      db.createObjectStore('one')
    },
    (db) => {
      db.createObjectStore('two')
    },
  ]
  ;(await openDatabase('m', factory, steps.slice(0, 1))).close()
  const upgraded = await openDatabase('m', factory, steps)
  assert.equal(upgraded.version, 2)
  assert.deepEqual([...upgraded.objectStoreNames].sort(), ['one', 'two'])
  upgraded.close()

  const broken: Migration[] = [
    ...steps,
    (db) => {
      db.createObjectStore('three')
      throw new Error('migration bug')
    },
  ]
  await assert.rejects(openDatabase('m', factory, broken))
  const unchanged = await openDatabase('m', factory, steps)
  assert.equal(unchanged.version, 2)
  assert.deepEqual([...unchanged.objectStoreNames].sort(), ['one', 'two'])
  unchanged.close()
})

test('the store key is drawn by the core once, sealed by a non-extractable key, then unsealed', async () => {
  const factory = new IDBFactory()
  const store = await IndexedDbStore.open('tessel', factory)
  let draws = 0
  const generate = () => {
    draws += 1
    return core.generateStoreKey()
  }
  const first = await loadStoreKey(store, generate)
  assert.equal(first.length, 32)
  store.close()

  const reopened = await IndexedDbStore.open('tessel', factory)
  const again = await loadStoreKey(reopened, generate)
  assert.deepEqual(again, first)
  assert.equal(draws, 1)

  const wrapping = await reopened.hostValue<CryptoKey>('wrapping-key')
  assert.ok(wrapping)
  assert.equal(wrapping.extractable, false)
  await assert.rejects(crypto.subtle.exportKey('raw', wrapping))

  // What IndexedDB keeps is not the key.
  const sealed = await reopened.hostValue<{ iv: Uint8Array; ciphertext: Uint8Array }>('sealed-store-key')
  assert.ok(sealed)
  assert.notDeepEqual(sealed.ciphertext.slice(0, 32), first)

  // An altered sealed key does not unseal.
  const altered = sealed.ciphertext.slice()
  altered[0] = (altered[0] ?? 0) ^ 1
  await reopened.setHostValues({ 'sealed-store-key': { iv: sealed.iv, ciphertext: altered } })
  await assert.rejects(loadStoreKey(reopened, generate))
  assert.equal(draws, 1)
  reopened.close()
})

test('a half-present store key is never silently replaced', async () => {
  const store = await IndexedDbStore.open('tessel', new IDBFactory())
  await store.setHostValues({ 'sealed-store-key': { iv: filled(12, 0), ciphertext: filled(48, 0) } })
  await assert.rejects(loadStoreKey(store, core.generateStoreKey), /half there/)
  store.close()
})

test('two store keys drawn by the core differ', () => {
  assert.notDeepEqual(core.generateStoreKey(), core.generateStoreKey())
})
