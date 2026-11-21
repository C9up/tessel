import { defineConfig } from '@c9up/relay'

/**
 * Real-time delivery (story 04-03): one SSE stream per device, on which it hears its
 * channel (`devices/<id>`) ring. One instance for now; story 04-05 gives relay a bus.
 */
export default defineConfig({
  // A ring per channel nobody authorized is refused (relay's default, kept explicit).
  allowUnauthorizedChannels: false,
  // A stream with no traffic is dropped by proxies around a minute: a ping every 25 s.
  pingInterval: 25_000,
  // A device listens on its own channel: one is enough, a few spare.
  maxChannelsPerClient: 4,
})
