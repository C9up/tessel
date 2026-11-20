import { defineConfig } from '@c9up/blackhole/config'
import env from '#start/env.js'

export default defineConfig({
  xss: true,
  // `/rpc` is exempt: the API knows no cookie, each call is authenticated by the
  // device's key (story 03-02). With no cookie that the browser would attach by itself,
  // a request forged by another site has nothing to impersonate. CSRF stays active for
  // every other route, should a route ever rely on a cookie.
  csrf: { exceptRoutes: ['/rpc'] },
  // Through the validated environment, never `process.env` directly; the key is
  // revealed only here, where it is used.
  secret: env.get('APP_KEY').release(),
})
