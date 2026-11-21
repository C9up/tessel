/**
 * What the phone checks of the core's mobile binding (story 02-13), through the very
 * `TesselCore` the app will use: an account in a database sealed by the Android
 * Keystore, a group, a message queued and encrypted, and the device found again after
 * the core is reopened. Each step reports a line; the first failure stops the run.
 */

import type { TesselCore } from '@tessel/core-web/api'

const hex = (bytes: Uint8Array) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')

export async function selfCheck(open: () => TesselCore, report: (line: string) => void): Promise<boolean> {
  const step = async <T>(name: string, run: () => Promise<T>, describe: (value: T) => string = () => ''): Promise<T> => {
    try {
      const value = await run()
      const detail = describe(value)
      report(`OK  ${name}${detail === '' ? '' : `: ${detail}`}`)
      return value
    } catch (error) {
      const code = typeof error === 'object' && error !== null && 'code' in error ? String(error.code) : ''
      report(`FAIL ${name}: ${code} ${error instanceof Error ? error.message : String(error)}`)
      throw error
    }
  }
  try {
    let core = open()
    await step(
      'core',
      () => core.version(),
      (version) => `tessel-core ${version}`,
    )
    await step(
      'suite',
      () => core.ciphersuite(),
      (suite) => suite,
    )
    const found = await step(
      'database (Keystore)',
      () => core.start(),
      (device) => (device === undefined ? 'new profile' : 'reopened'),
    )
    const device =
      found ??
      (await step(
        'account',
        () => core.createAccount(`phone-${Math.floor(Math.random() * 1e9)}@tessel.example`),
        (created) => created.account,
      ))
    const group = await step(
      'group',
      () => core.createGroup(),
      (id) => hex(id).slice(0, 12),
    )
    await step('message queued', () => core.queueMessage(group, { type: 'text', text: 'hello from the phone' }, Date.now()))
    const outbound = await step(
      'message encrypted',
      () => core.nextOutbound(group),
      (out) => `${out?.encrypted.length ?? 0} bytes`,
    )
    if (outbound !== undefined) await step('message sent', () => core.outboundSent(group, outbound.sendId))
    await step(
      'conversations',
      () => core.conversations(),
      (list) => `${list.length}`,
    )
    await step('close', () => core.close())
    core = open()
    await step(
      'reopened from the sealed database',
      async () => {
        const again = await core.start()
        if (again === undefined || hex(again.deviceId) !== hex(device.deviceId)) throw new Error('another device came back')
        return again
      },
      (again) => again.account,
    )
    return true
  } catch {
    return false
  }
}
