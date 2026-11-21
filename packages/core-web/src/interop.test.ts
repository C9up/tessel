/**
 * WASM ↔ native interoperability (story 02-12): devices of the core built for the
 * browser (here, its WASM module in Node) and devices of the core built natively, as on
 * a phone (`crates/tessel-core/examples/interop_peer.rs`), share one group, add each
 * other's devices and read each other's messages.
 */

import assert from 'node:assert/strict'
import { type ChildProcessWithoutNullStreams, spawn } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { createInterface } from 'node:readline'
import { after, test } from 'node:test'
import { IDBFactory } from 'fake-indexeddb'
import type { TesselCore } from './api.ts'
import { CoreHost } from './host.ts'
import type { Bytes } from './store.ts'
import { loadWasmSync } from './wasm.ts'

loadWasmSync(await readFile(new URL('../generated/tessel_wasm_bg.wasm', import.meta.url)))

const ROOT = new URL('../../../', import.meta.url).pathname

const toHex = (bytes: Uint8Array) => Buffer.from(bytes).toString('hex')
const fromHex = (hex: string): Bytes => Uint8Array.from(Buffer.from(hex, 'hex'))
const utf8 = (text: string) => toHex(new TextEncoder().encode(text))

/** The native devices, one process. */
class NativePeer {
  readonly #process: ChildProcessWithoutNullStreams
  readonly #waiting: { resolve: (line: string) => void; reject: (error: Error) => void }[] = []
  #stderr = ''
  #exited: Error | undefined

  constructor() {
    // Built on the first run (debug profile, shared with `cargo test`).
    this.#process = spawn('scripts/cargo.sh', ['run', '--quiet', '-p', 'tessel-core', '--example', 'interop_peer'], { cwd: ROOT })
    createInterface({ input: this.#process.stdout }).on('line', (line) => this.#waiting.shift()?.resolve(line))
    this.#process.stderr.on('data', (chunk: Buffer) => {
      this.#stderr += chunk.toString('utf8')
    })
    this.#process.on('exit', (code) => {
      this.#exited = new Error(`native peer exited (${code}): ${this.#stderr.slice(-2000)}`)
      for (const { reject } of this.#waiting.splice(0)) reject(this.#exited)
    })
  }

  /** Runs a command; the words of its answer. */
  async call(...words: string[]): Promise<string[]> {
    const line = await new Promise<string>((resolve, reject) => {
      if (this.#exited) return reject(this.#exited)
      this.#waiting.push({ resolve, reject })
      this.#process.stdin.write(`${words.join(' ')}\n`)
    })
    const [status, ...rest] = line.split(' ')
    if (status !== 'ok') throw new Error(`native peer: ${line}`)
    return rest
  }

  async one(...words: string[]): Promise<string> {
    const [value] = await this.call(...words)
    assert.ok(value !== undefined, `no answer to ${words[0]}`)
    return value
  }

  close(): void {
    this.#process.stdin.end()
  }
}

const native = new NativePeer()
after(() => native.close())

const factory = new IDBFactory()
const now = () => Math.floor(Date.now() / 1000)

async function webDevice(name: string): Promise<TesselCore> {
  const core = new CoreHost({ database: `interop-${name}`, factory })
  await core.start()
  await core.createAccount(`${name}@tessel.example`)
  return core
}

async function webKeyPackage(core: TesselCore): Promise<Bytes> {
  const [keyPackage] = (await core.refreshKeyPackages(now())).publish
  assert.ok(keyPackage)
  return keyPackage
}

/** What a web device reads in an encrypted message. */
async function webReads(core: TesselCore, encrypted: Bytes): Promise<string | undefined> {
  const incoming = await core.process(encrypted)
  assert.ok(incoming.type === 'message')
  const body = incoming.message?.body
  return body?.type === 'text' ? body.text : undefined
}

async function nativeReads(device: string, encrypted: Bytes): Promise<string> {
  const [kind, text] = await native.call('process', device, toHex(encrypted))
  assert.equal(kind, 'message')
  return Buffer.from(text ?? '', 'hex').toString('utf8')
}

async function webSends(core: TesselCore, group: Bytes, text: string): Promise<Bytes> {
  await core.queueMessage(group, { type: 'text', text }, 1_000)
  const outbound = await core.nextOutbound(group)
  assert.ok(outbound)
  await core.outboundSent(group, outbound.sendId)
  return outbound.encrypted
}

test('web and native devices share a group, both ways', async () => {
  const [alice, dave] = [await webDevice('alice'), await webDevice('dave')]
  await native.call('device', 'bob')
  await native.call('device', 'carol')

  // A web device creates the group and adds two native devices.
  const group = await alice.createGroup()
  const packages = [fromHex(await native.one('key-package', 'bob')), fromHex(await native.one('key-package', 'carol'))]
  const { welcome } = await alice.addDevices(group, packages)
  await alice.commitAccepted(group)
  assert.ok(welcome)
  for (const device of ['bob', 'carol']) {
    assert.equal(await native.one('join', device, toHex(welcome)), toHex(group))
  }

  // Web to native, with text that is not ASCII.
  const fromWeb = 'hello from the page, naïve café 🌍'
  const sent = await webSends(alice, group, fromWeb)
  assert.equal(await nativeReads('bob', sent), fromWeb)
  assert.equal(await nativeReads('carol', sent), fromWeb)

  // Native to web.
  const fromNative = 'hello from the phone side'
  const reply = fromHex(await native.one('send', 'bob', toHex(group), utf8(fromNative)))
  assert.equal(await webReads(alice, reply), fromNative)
  assert.equal(await nativeReads('carol', reply), fromNative)

  // A native device adds a web device: everyone moves to the next epoch.
  const [commit, daveWelcome] = await native.call('add', 'bob', toHex(group), toHex(await webKeyPackage(dave)))
  assert.ok(commit && daveWelcome)
  const applied = await alice.process(fromHex(commit))
  assert.equal(applied.type, 'commit')
  assert.deepEqual(applied.type === 'commit' && applied.added.map((d) => d.account), ['dave@tessel.example'])
  const [kind, epoch] = await native.call('process', 'carol', commit)
  assert.equal(kind, 'commit')
  assert.deepEqual(await dave.joinGroup(fromHex(daveWelcome)), group)
  assert.equal(String(await alice.epoch(group)), epoch)
  assert.equal(String(await dave.epoch(group)), epoch)

  // The newest device is read by all, web and native.
  const fromDave = 'four devices, two builds'
  const last = await webSends(dave, group, fromDave)
  assert.equal(await webReads(alice, last), fromDave)
  assert.equal(await nativeReads('bob', last), fromDave)
  assert.equal(await nativeReads('carol', last), fromDave)
  assert.equal((await alice.members(group)).length, 4)
  assert.equal(await native.one('members', 'carol', toHex(group)), '4')

  await Promise.all([alice.close(), dave.close()])
})
