/**
 * The contract of `TesselCore` on the mobile binding (story 02-13): the web's scenarios,
 * run through this package's `NativeEndpoint` on the phone's Rust host. A computer has
 * no JSI: the host runs as a process (`contract-host`, packages/core-mobile/rust) that
 * answers each call synchronously over two FIFOs, as the JSI module does in the app.
 */

import { type ChildProcess, execFileSync, spawn } from 'node:child_process'
import { closeSync, existsSync, mkdirSync, mkdtempSync, openSync, readSync, rmSync, writeSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { after } from 'node:test'
import { contract } from '@tessel/core-web/contract'
import { mobileCore, type NativeTesselModule } from './index.ts'

const HOST = new URL('../../../target/debug/contract-host', import.meta.url).pathname
if (!existsSync(HOST)) throw new Error(`${HOST} is missing: the test script builds it (pnpm test)`)

const root = mkdtempSync(join(tmpdir(), 'tessel-contract-'))
const hosts: HostProcess[] = []
after(() => {
  for (const host of hosts) host.stop()
  rmSync(root, { recursive: true, force: true })
})

const length = (value: number) => {
  const bytes = new Uint8Array(4)
  new DataView(bytes.buffer).setUint32(0, value, true)
  return bytes
}

/** The phone's host in a process of its own, reached synchronously, as over JSI. */
class HostProcess implements NativeTesselModule {
  readonly #child: ChildProcess
  readonly #requests: number
  readonly #answers: number

  constructor(database: string) {
    const dir = join(root, database)
    mkdirSync(dir, { recursive: true })
    const fifos = mkdtempSync(join(root, 'fifo-'))
    const requests = join(fifos, 'requests')
    const answers = join(fifos, 'answers')
    execFileSync('mkfifo', [requests, answers])
    this.#child = spawn(HOST, [dir, requests, answers], { stdio: ['ignore', 'inherit', 'inherit'] })
    // In the host's order: each open waits for the other end.
    this.#requests = openSync(requests, 'w')
    this.#answers = openSync(answers, 'r')
  }

  #read(count: number): Uint8Array<ArrayBuffer> {
    const bytes = new Uint8Array(count)
    let done = 0
    while (done < count) {
      const read = readSync(this.#answers, bytes, done, count - done, null)
      if (read === 0) throw new Error('host:gone\nthe contract host stopped')
      done += read
    }
    return bytes
  }

  call(method: string, args: Uint8Array): unknown {
    const name = new TextEncoder().encode(method)
    for (const part of [length(name.length), name, length(args.length), args]) writeSync(this.#requests, part)
    const [status] = this.#read(1)
    const size = new DataView(this.#read(4).buffer).getUint32(0, true)
    const answer = this.#read(size)
    if (status !== 0) throw new Error(new TextDecoder().decode(answer))
    return answer.buffer
  }

  stop(): void {
    closeSync(this.#requests)
    closeSync(this.#answers)
    this.#child.kill()
  }
}

contract({
  'mobile binding': (database) => {
    const host = new HostProcess(database)
    hosts.push(host)
    return mobileCore(host)
  },
})
