import { createNapiConnection } from '@c9up/atlas'
import { DateTime } from '@c9up/chronos'
import { checkKeyPackage, type ThrowawayDevice } from '@tessel/core-web/server'
import { Messenger, type MessengerCore } from '@tessel/messenger'
import { createTesselClient, type TesselClient } from '@tessel/protocol'
import { Inbox } from '@tessel/protocol/realtime'
import databaseConfig from '#config/database.js'

/**
 * Test devices that talk through the real server with `@tessel/messenger` (epic 05):
 * each one signed in, its KeyPackages in the directory, its messenger and its inbox.
 */

export const fresh = (prefix: string) => `${prefix}-${crypto.randomUUID().slice(0, 8)}`
export const now = () => Math.floor(DateTime.now().toSeconds())
const clock = () => DateTime.now().toMillis()

interface ApiRequest {
  json(body: unknown): ApiRequest
  bearerToken(token: string): ApiRequest
  send(): Promise<{ status(): number; body(): unknown }>
}

export interface ApiClient {
  post(url: string): ApiRequest
}

export function connect(client: ApiClient, token?: string): TesselClient {
  return createTesselClient(async (url, body) => {
    const request = client.post(url).json(body)
    const response = await (token === undefined ? request : request.bearerToken(token)).send()
    return response.status() === 204 ? null : response.body()
  })
}

/** Deletes the test accounts (their devices, sessions and mailboxes go with them). */
export async function deleteAccounts(names: string[]): Promise<void> {
  if (names.length === 0) return
  const database = await createNapiConnection(databaseConfig.url ?? '', 1, 1)
  await database.execute('DELETE FROM accounts WHERE name IN (SELECT jsonb_array_elements_text($1::jsonb))', [JSON.stringify(names)])
}

/** A message the core read: its text, if it is one. */
function textOf(incoming: unknown): string | undefined {
  if (typeof incoming !== 'object' || incoming === null || !('message' in incoming)) return undefined
  const message = incoming.message
  if (typeof message !== 'object' || message === null || !('body' in message)) return undefined
  const body = message.body
  return typeof body === 'object' && body !== null && 'text' in body && typeof body.text === 'string' ? body.text : undefined
}

/**
 * The core of a test device, as the messenger asks for it. The texts it writes and
 * reads go to `seen`, in order, and what it processes to `processed`: a test device
 * keeps no database to read its history back from.
 */
function coreOf(device: ThrowawayDevice, seen: string[], processed: unknown[]): MessengerCore {
  const core = device.core
  return {
    device: () => core.device(),
    verifyKeyPackage: (keyPackage) => {
      const checked = checkKeyPackage(keyPackage)
      if (checked === undefined) throw new Error('not a valid KeyPackage')
      return checked
    },
    createGroup: () => core.createGroup(),
    addDevices: (group, keyPackages) => core.addDevices(group, keyPackages),
    removeDevices: (group, removals) => core.removeDevices(group, removals),
    configureGroup: (group, change) => core.configureGroup(group, change),
    groupSettings: (group) => core.groupSettings(group),
    commitPendingProposals: (group) => core.commitPendingProposals(group),
    commitAccepted: (group) => core.commitAccepted(group),
    commitRejected: (group) => core.commitRejected(group),
    retryCommit: (group) => core.retryCommit(group),
    leaveGroup: (group) => core.leaveGroup(group),
    joinGroup: (welcome) => core.joinGroup(welcome),
    members: (group) => core.members(group),
    process: (message) => {
      const incoming = core.process(message)
      processed.push(incoming)
      const text = textOf(incoming)
      if (text !== undefined) seen.push(text)
      return incoming
    },
    recipients: (group) => core.recipients(group),
    queueMessage: (group, body, sentAt) => {
      seen.push(body.text)
      return core.queueMessage(group, body, sentAt)
    },
    nextOutbound: (group) => core.nextOutbound(group),
    outboundSent: (group, sendId) => core.outboundSent(group, sendId),
    conversations: () => core.conversations(),
    markRead: (group) => core.markRead(group),
  }
}

/** A device signed in, its KeyPackages in the directory, its messenger and its inbox. */
export async function app(client: ApiClient, device: ThrowawayDevice) {
  const anonymous = connect(client)
  const challenge = await anonymous.challenge()
  const { token } = await anonymous.signIn(device.deviceId, challenge, device.signIn(challenge), 'web')
  const tessel = connect(client, token)
  await tessel.publishKeyPackages(device.refreshKeyPackages(now()))
  const holder: { messenger?: Messenger } = {}
  const inbox = new Inbox(tessel, async (envelope) => holder.messenger?.receive(envelope))
  const seen: string[] = []
  const processed: unknown[] = []
  const messenger = new Messenger(coreOf(device, seen, processed), tessel, () => inbox.drain(), clock)
  holder.messenger = messenger
  return { device, tessel, messenger, inbox, texts: () => [...seen], processed }
}
