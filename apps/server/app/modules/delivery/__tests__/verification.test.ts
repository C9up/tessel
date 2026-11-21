import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice, throwawayLink } from '@tessel/core-web/server'
import type { Alert } from '@tessel/messenger'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'
import { type ApiClient, app, connect, deleteAccounts, endReservation, fresh } from './messengerApp.js'

/**
 * Verifying a contact (story 05-03), end to end over the real server with the real
 * cores: the same safety number on both screens, a QR scanned, and the alerts when a
 * verified contact adds a device or comes back under another account key.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

/** A new account with one phone, signed in. */
async function person(client: ApiClient, name: string) {
  const phone = throwawayDevice(`${name}@${domain}`)
  devices.push(phone)
  await connect(client).createAccount(phone.certificate)
  return { account: `${name}@${domain}`, ...(await app(client, phone)) }
}

/** The code of a refusal. */
async function refusal(action: Promise<unknown>): Promise<string | undefined> {
  try {
    await action
  } catch (error) {
    return typeof error === 'object' && error !== null && 'code' in error ? String(error.code) : String(error)
  }
  return undefined
}

describe('verifying a contact', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    await deleteAccounts(created.splice(0))
  })

  test('the same code on both screens, a QR scanned, and an alert for each change', async ({ client }) => {
    const [aliceName, bobName] = [fresh('alice'), fresh('bob')]
    created.push(aliceName, bobName)
    const alice = await person(client, aliceName)
    const bob = await person(client, bobName)
    const alerts: Alert[] = []
    alice.messenger.onAlert((alert) => alerts.push(alert))

    // Alice writes to Bob: each sees the same safety number for the other.
    const group = await alice.messenger.start(bob.account)
    await bob.inbox.drain()
    const onAlice = await alice.messenger.safetyNumber(group, bob.account)
    const onBob = await bob.messenger.safetyNumber(group, alice.account)
    expect(onAlice.digits).toHaveLength(60)
    expect(onAlice.digits).toBe(onBob.digits)
    expect(await alice.messenger.verification(group)).toEqual([{ account: bob.account, trust: 'unverified' }])

    // Alice scans her own QR by mistake: refused. Then Bob's: verified in one gesture.
    expect(await refusal(alice.messenger.verifyScanned(group, bob.account, onAlice.qr))).toBe('safety:mismatch')
    await alice.messenger.verifyScanned(group, bob.account, onBob.qr)
    expect(await alice.messenger.verification(group)).toEqual([{ account: bob.account, trust: 'verified' }])

    // Bob links a tablet; Alice makes a group with him: told of the new device.
    const link = throwawayLink()
    const granted = bob.device.grantLink(link.qr)
    await bob.tessel.linkDevice(granted.certificate, granted.link, granted.grant)
    const tablet = link.accept((await connect(client).takeLinkGrant(link.link)) ?? new Uint8Array())
    devices.push(tablet)
    await app(client, tablet)
    const family = await alice.messenger.createGroup('Famille', [bob.account])
    await Promise.resolve()
    expect(alerts).toEqual([{ type: 'new-device', group: family, account: bob.account, devices: [tablet.deviceId] }])
    expect(await alice.messenger.verification(family)).toEqual([{ account: bob.account, trust: 'new-device' }])
    await alice.messenger.acknowledgeDevices(family, bob.account)
    expect(await alice.messenger.verification(family)).toEqual([{ account: bob.account, trust: 'verified' }])

    // Bob deletes his account; once the name is free, someone takes it under another key.
    await bob.tessel.deleteAccount(bob.device.signAccountDeletion())
    await endReservation(bobName)
    const usurper = await person(client, bobName)
    alerts.length = 0
    const again = await alice.messenger.start(usurper.account)
    await Promise.resolve()
    expect(alerts).toEqual([{ type: 'key-changed', group: again, account: bob.account }])
    expect(await alice.messenger.verification(again)).toEqual([{ account: bob.account, trust: 'key-changed' }])
    await usurper.inbox.drain()
    expect((await alice.messenger.safetyNumber(again, bob.account)).digits).not.toBe(onAlice.digits)
  })
})
