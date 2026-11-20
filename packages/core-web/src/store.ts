/**
 * The local database in the browser (story 02-06): IndexedDB, holding only the sealed
 * records the core hands over (see `crates/tessel-core/src/store.rs`). Partitions,
 * identifiers and values are opaque bytes; nothing here can read them.
 *
 * The store key the core seals records with is itself sealed by a non-extractable
 * WebCrypto key that the browser keeps in IndexedDB: script running in the page can use
 * it while the page is open, but can never export it.
 */

/** Bytes backed by an `ArrayBuffer`, as WebCrypto and IndexedDB keys require. */
export type Bytes = Uint8Array<ArrayBuffer>

/** A sealed record, as the core hands it over. */
export interface StoredRecord {
  partition: Bytes
  id: Bytes
  value: Bytes
}

/** What the core asks to persist after a call, in one transaction. */
export interface StoreChanges {
  put: StoredRecord[]
  delete: { partition: Bytes; id: Bytes }[]
}

/** A schema migration: migration `n` brings the database from version `n` to `n + 1`. */
export type Migration = (database: IDBDatabase) => void

/** The released migrations. One is never edited once released; a change is a new one. */
export const MIGRATIONS: readonly Migration[] = [
  // Version 1: the sealed records, indexed by partition, and the host's own values (the
  // wrapping key and the sealed store key).
  (database) => {
    const records = database.createObjectStore('records', { keyPath: ['partition', 'id'] })
    records.createIndex('partition', 'partition')
    database.createObjectStore('host')
  },
]

/** Schema version this code writes. */
export const INDEXEDDB_SCHEMA_VERSION = MIGRATIONS.length

/** The database was written by a newer version of Tessel. */
export class UnsupportedSchemaError extends Error {
  constructor(name: string) {
    super(`database ${name} was written by a newer version of Tessel`)
    this.name = 'UnsupportedSchemaError'
  }
}

function request<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

function done(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve()
    transaction.onerror = () => reject(transaction.error)
    transaction.onabort = () => reject(transaction.error ?? new Error('transaction aborted'))
  })
}

/** Opens `name`, applying the migrations it lacks; refuses a newer schema. */
export function openDatabase(name: string, factory: IDBFactory = indexedDB, migrations: readonly Migration[] = MIGRATIONS): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = factory.open(name, migrations.length)
    // Every migration runs in the single upgrade transaction: if one fails, the
    // database stays at its previous version, untouched.
    req.onupgradeneeded = (event) => {
      for (const migration of migrations.slice(event.oldVersion)) {
        migration(req.result)
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => {
      reject(req.error?.name === 'VersionError' ? new UnsupportedSchemaError(name) : req.error)
    }
    req.onblocked = () => reject(new Error(`database ${name} is open elsewhere in an older version`))
  })
}

/** IndexedDB returns binary keys and values as `ArrayBuffer` or views: always bytes. */
function bytes(value: unknown): Bytes {
  if (value instanceof ArrayBuffer) return new Uint8Array(value)
  if (ArrayBuffer.isView(value)) return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength))
  throw new TypeError('stored record is not bytes')
}

/** The sealed records of a device. */
export class IndexedDbStore {
  readonly database: IDBDatabase

  private constructor(database: IDBDatabase) {
    this.database = database
  }

  static async open(name: string, factory: IDBFactory = indexedDB): Promise<IndexedDbStore> {
    return new IndexedDbStore(await openDatabase(name, factory))
  }

  /** Applies the changes atomically: all of them, or none. */
  async apply(changes: StoreChanges): Promise<void> {
    const transaction = this.database.transaction('records', 'readwrite', { durability: 'strict' })
    const records = transaction.objectStore('records')
    try {
      for (const record of changes.put) {
        records.put({ partition: record.partition, id: record.id, value: record.value })
      }
      for (const { partition, id } of changes.delete) {
        records.delete([partition, id])
      }
    } catch (error) {
      transaction.abort()
      await done(transaction).catch(() => undefined)
      throw error
    }
    await done(transaction)
  }

  /** Every record of a partition, in any order. */
  async list(partition: Bytes): Promise<StoredRecord[]> {
    const transaction = this.database.transaction('records', 'readonly')
    const rows = await request(transaction.objectStore('records').index('partition').getAll(partition))
    return rows.map((row: { partition: unknown; id: unknown; value: unknown }) => ({
      partition: bytes(row.partition),
      id: bytes(row.id),
      value: bytes(row.value),
    }))
  }

  /** A value the host keeps next to the records. */
  async hostValue<T>(name: string): Promise<T | undefined> {
    const transaction = this.database.transaction('host', 'readonly')
    return (await request(transaction.objectStore('host').get(name))) as T | undefined
  }

  /** Writes host values, in one transaction. */
  async setHostValues(values: Record<string, unknown>): Promise<void> {
    const transaction = this.database.transaction('host', 'readwrite', { durability: 'strict' })
    const host = transaction.objectStore('host')
    for (const [name, value] of Object.entries(values)) host.put(value, name)
    await done(transaction)
  }

  close(): void {
    this.database.close()
  }
}

const WRAPPING_KEY = 'wrapping-key'
const SEALED_STORE_KEY = 'sealed-store-key'
/** Binds the sealed store key to its purpose. */
const STORE_KEY_LABEL = new TextEncoder().encode('Tessel 1.0 store key')

interface SealedStoreKey {
  iv: Bytes
  ciphertext: Bytes
}

/**
 * The store key of this browser profile: unsealed with the non-extractable wrapping key
 * when it exists, otherwise drawn by the core (`generate`), sealed and kept. The caller
 * hands the bytes to the core, then wipes them (`fill(0)`).
 */
export async function loadStoreKey(store: IndexedDbStore, generate: () => Uint8Array): Promise<Bytes> {
  const wrapping = await store.hostValue<CryptoKey>(WRAPPING_KEY)
  const sealed = await store.hostValue<SealedStoreKey>(SEALED_STORE_KEY)
  if (wrapping !== undefined && sealed !== undefined) {
    const key = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: bytes(sealed.iv), additionalData: STORE_KEY_LABEL }, wrapping, bytes(sealed.ciphertext))
    return new Uint8Array(key)
  }
  if (wrapping !== undefined || sealed !== undefined) {
    throw new Error('the store key is half there: refusing to replace it')
  }

  const key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt'])
  const drawn = generate()
  const storeKey = Uint8Array.from(drawn)
  drawn.fill(0)
  const iv = crypto.getRandomValues(new Uint8Array(12))
  const ciphertext = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv, additionalData: STORE_KEY_LABEL }, key, storeKey))
  // Both in one transaction: never a wrapping key without the key it seals.
  await store.setHostValues({ [WRAPPING_KEY]: key, [SEALED_STORE_KEY]: { iv, ciphertext } })
  return storeKey
}
