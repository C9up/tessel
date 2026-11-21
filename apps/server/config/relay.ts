import { defineConfig, transports } from '@c9up/relay'

/**
 * Real-time delivery (story 04-03): one SSE stream per device, on which it hears its
 * channel (`devices/<id>`) ring. Rings travel between the server's instances over KeyDB
 * (story 04-05): a device connected to one instance hears an envelope another one kept.
 * A ring carries an envelope's identifier, never its body.
 */
export default defineConfig({
  // A ring per channel nobody authorized is refused (relay's default, kept explicit).
  allowUnauthorizedChannels: false,
  // A stream with no traffic is dropped by proxies around a minute: a ping every 25 s.
  pingInterval: 25_000,
  // A device listens on its own channel: one is enough, a few spare.
  maxChannelsPerClient: 4,
  transport: { driver: transports.redis({ connection: 'main' }), channel: 'tessel::relay' },
})
