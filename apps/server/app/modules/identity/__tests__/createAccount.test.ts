import { createNapiConnection } from '@c9up/atlas'
import { afterAll, beforeEach, describe, expect, test } from '@c9up/helix'
import emitter from '@c9up/ream/events/services/main'
import { throwawayCertificate } from '@tessel/core-web/server'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import databaseConfig from '#config/database.js'
import { AccountCreated } from '#modules/identity/domain/events.js'
import { clearLimits } from '#modules/shared/__tests__/limits.js'
import env from '#start/env.js'

/**
 * Account creation (story 03-01), end to end: the device's keys are made by the core,
 * as in the app, and its certificate goes over JSON-RPC.
 */

const domain = env.get('APP_DOMAIN')
const created: string[] = []

/** A name no earlier run took. */
const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`

/** The error a call failed with: its JSON-RPC code and message. */
async function failure(call: Promise<unknown>): Promise<{ code: unknown; message: unknown }> {
  try {
    await call
  } catch (error) {
    if (error instanceof Error && 'code' in error) return { code: error.code, message: error.message }
    throw error
  }
  throw new Error('the call succeeded')
}

/** Ream's test client, as far as JSON-RPC needs it. */
interface ApiClient {
  post(url: string): { json(body: unknown): { send(): Promise<{ status(): number; body(): unknown }> } }
}

/** The client shared by web and mobile, over Ream's test client. */
function connect(client: ApiClient): TesselClient {
  return createTesselClient(async (url, body) => {
    const response = await client.post(url).json(body).send()
    return response.status() === 204 ? null : response.body()
  })
}

async function database() {
  return createNapiConnection(databaseConfig.url ?? '', 1, 1)
}

describe('creating an account', () => {
  beforeEach(() => clearLimits())

  afterAll(async () => {
    if (created.length > 0)
      await (await database()).execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(created.splice(0))])
  })

  test('a certificate made by the core creates name@domain, and the bus hears it', async ({ client }) => {
    const tessel = connect(client)
    const name = fresh('alice')
    const heard: AccountCreated[] = []
    const unsubscribe = emitter.on(AccountCreated, (event: AccountCreated) => void heard.push(event))
    try {
      expect(await tessel.createAccount(throwawayCertificate(`${name}@${domain}`))).toEqual({ account: `${name}@${domain}` })
      created.push(name)
    } finally {
      unsubscribe()
    }

    expect(heard.length).toBe(1)
    expect(heard[0]?.type).toBe('identity.account.created')
    // The event names an opaque identifier, never the name.
    expect(JSON.stringify(heard[0])).not.toContain(name)

    const [row] = await (await database()).query<{ id: string; devices: string }>(
      'SELECT a.id::text AS id, count(d.id)::text AS devices FROM accounts a JOIN devices d ON d.account_id = a.id WHERE a.name = $1 GROUP BY a.id',
      [name],
    )
    expect(row?.id).toBe(heard[0]?.accountId)
    expect(row?.devices).toBe('1')
  })

  test('a name is taken once', async ({ client }) => {
    const tessel = connect(client)
    const name = fresh('bob')
    await tessel.createAccount(throwawayCertificate(`${name}@${domain}`))
    created.push(name)
    // Another device, another account key, the same name.
    expect(await failure(tessel.createAccount(throwawayCertificate(`${name}@${domain}`)))).toEqual({ code: -32009, message: 'E_IDENTITY_NAME_TAKEN' })
  })

  test('an account of another instance is refused', async ({ client }) => {
    const tessel = connect(client)
    expect(await failure(tessel.createAccount(throwawayCertificate(`${fresh('carol')}@elsewhere.example`)))).toEqual({
      code: -32602,
      message: 'E_IDENTITY_WRONG_DOMAIN',
    })
  })

  test('a certificate whose signature does not verify, or no certificate at all, is refused', async ({ client }) => {
    const tessel = connect(client)
    const tampered = throwawayCertificate(`${fresh('dave')}@${domain}`)
    tampered[tampered.length - 1] = (tampered[tampered.length - 1] ?? 0) ^ 1
    expect(await failure(tessel.createAccount(tampered))).toEqual({ code: -32602, message: 'E_IDENTITY_INVALID_CERTIFICATE' })
    expect(await failure(tessel.createAccount(new Uint8Array([1, 2, 3])))).toEqual({ code: -32602, message: 'E_IDENTITY_INVALID_CERTIFICATE' })
  })

  test('one network creates a few accounts an hour, no more', async ({ client }) => {
    const tessel = connect(client)
    for (let i = 0; i < 5; i++) {
      const refused = await failure(tessel.createAccount(throwawayCertificate(`${fresh('eve')}@elsewhere.example`)))
      expect(refused.message).toBe('E_IDENTITY_WRONG_DOMAIN')
    }
    const sixth = await failure(tessel.createAccount(throwawayCertificate(`${fresh('eve')}@${domain}`)))
    expect(sixth.message).toBe('E_TOO_MANY_REQUESTS')
  })
})
