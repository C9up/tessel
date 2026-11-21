import { limiterStores } from '@c9up/blackhole'
import { defineConfig } from '@c9up/blackhole/config'
import env from '#start/env.js'

export default defineConfig({
  // Rate-limit counters live in KeyDB (story 04-06): every instance of the server counts
  // the same calls, so a limit holds however many instances run.
  limiter: {
    default: 'redis',
    stores: { redis: limiterStores.redis({ connection: 'main' }) },
  },
  xss: true,
  // `/rpc` is exempt: the API knows no cookie, each call is authenticated by the
  // device's key (story 03-02). With no cookie that the browser would attach by itself,
  // a request forged by another site has nothing to impersonate. CSRF stays active for
  // every other route, should a route ever rely on a cookie.
  // The relay's subscribe routes (story 04-03) are in the same case: authenticated by
  // the device's session token, never by a cookie.
  csrf: { exceptRoutes: ['/rpc', '/__relay/subscribe', '/__relay/unsubscribe'] },
  // Through the validated environment, never `process.env` directly; the key is
  // revealed only here, where it is used.
  secret: env.get('APP_KEY').release(),
})
