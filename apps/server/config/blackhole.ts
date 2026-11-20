import { defineConfig } from '@c9up/blackhole/config'
import env from '#start/env.js'

export default defineConfig({
  xss: true,
  // `/rpc` est exempté : l'API ne connaît aucun cookie, chaque appel est authentifié par
  // la clé de l'appareil (story 03-02). Sans cookie que le navigateur joindrait tout seul,
  // une requête forgée par un autre site n'a rien à usurper. Le CSRF reste actif pour
  // toute autre route, si un jour une route s'appuie sur un cookie.
  csrf: { exceptRoutes: ['/rpc'] },
  // Par l'environnement validé, jamais `process.env` directement ; la clé n'est
  // révélée qu'ici, où elle sert.
  secret: env.get('APP_KEY').release(),
})
