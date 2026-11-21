import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import { type ThrowawayDevice, throwawayDevice } from '@tessel/core-web/server'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'
import { type ApiClient, app, connect, deleteAccounts, fresh } from './messengerApp.js'

/**
 * Groups (story 05-02), end to end over the real server: a named group, restricted to
 * its admin, an invitation, a removal and a departure, each device seeing what its
 * place in the group lets it see.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []
const devices: ThrowawayDevice[] = []

/** A new account with one phone, signed in. */
async function person(client: ApiClient, prefix: string) {
  const name = fresh(prefix)
  created.push(name)
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

describe('groups', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    for (const device of devices.splice(0)) device.free()
    await deleteAccounts(created.splice(0))
  })

  test('a named group, reserved to its admin: invite, remove, leave', async ({ client }) => {
    const [alice, bob, carol, dave] = [await person(client, 'alice'), await person(client, 'bob'), await person(client, 'carol'), await person(client, 'dave')]

    // Alice makes "Famille" with Bob and Carol; they learn its name from the Welcome.
    const group = await alice.messenger.createGroup('Famille', [bob.account, carol.account])
    for (const each of [bob, carol]) {
      await each.inbox.drain()
      expect((await each.messenger.settings(group)).name).toBe('Famille')
      expect((await each.messenger.conversations())[0]?.name).toBe('Famille')
    }

    // She reserves membership to admins: Carol cannot invite Dave.
    await alice.messenger.configure(group, { type: 'restrict', restricted: true })
    await bob.inbox.drain()
    await carol.inbox.drain()
    expect(await refusal(carol.messenger.invite(group, [dave.account]))).toBe('group:not-admin')

    // Alice can; Dave joins and reads what is written next.
    await alice.messenger.invite(group, [dave.account])
    await dave.inbox.drain()
    expect((await dave.messenger.settings(group)).restricted).toBe(true)
    await alice.messenger.write(group, 'welcome Dave')
    for (const each of [bob, carol, dave]) await each.inbox.drain()
    for (const each of [bob, carol, dave]) expect(each.texts()).toEqual(['welcome Dave'])

    // Alice removes Carol: Carol learns it, and reads nothing after.
    await alice.messenger.remove(group, carol.account)
    await carol.inbox.drain()
    expect(carol.processed.at(-1)).toMatchObject({ type: 'commit', removedSelf: true })
    await bob.inbox.drain()
    await dave.inbox.drain()
    await alice.messenger.write(group, 'without Carol')
    for (const each of [bob, carol, dave]) await each.inbox.drain()
    expect(carol.texts()).toEqual(['welcome Dave'])
    expect(dave.texts()).toEqual(['welcome Dave', 'without Carol'])

    // Bob, no admin, leaves: the first member to get his proposal commits it.
    await bob.messenger.leave(group)
    await dave.inbox.drain()
    await alice.inbox.drain()
    await bob.inbox.drain()
    expect(bob.processed.at(-1)).toMatchObject({ type: 'commit', removedSelf: true })
    const members: { account: string }[] = alice.device.core.members(group)
    expect(members.map((member) => member.account).sort()).toEqual([alice.account, dave.account].sort())
    await alice.messenger.write(group, 'just us')
    await dave.inbox.drain()
    expect(dave.texts().at(-1)).toBe('just us')
  })
})
