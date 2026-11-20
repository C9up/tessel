import { expect, test } from '@c9up/helix'
import { createTesselClient, PROTOCOL_VERSION } from '@tessel/protocol'

test('the shared client pings the server over JSON-RPC', async ({ client }) => {
  // The client shared by web and mobile, on top of Ream's test client.
  const tessel = createTesselClient(async (url, body) => {
    const response = await client.post(url).json(body).send()
    return response.status() === 204 ? null : response.body()
  })
  expect(await tessel.ping()).toEqual({ pong: true, protocol: PROTOCOL_VERSION })
})
