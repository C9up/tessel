import { defineConfig } from '@c9up/ream'

export default defineConfig({
  providers: [
    () => import('@c9up/spectrum/provider'),
    () => import('@c9up/atlas/provider'),
    () => import('@c9up/ream/events/provider'),
    () => import('@c9up/ream/rpc/provider'),
    () => import('@c9up/quasar/provider'),
    () => import('@c9up/blackhole/provider'),
    () => import('@c9up/relay/provider'),
    () => import('#providers/AppProvider.js'),
  ],
  // Commands come from the packages, never from the global binary (absent in CI).
  commands: [() => import('@c9up/atlas/commands')],
  preloads: [() => import('#start/kernel.js'), () => import('#start/events.js'), () => import('#start/rpc.js'), () => import('#start/realtime.js')],
  modules: {
    path: './app/modules',
    // A module keeps its routes in its http layer (nadeo model).
    autoload: ['http/routes'],
  },
  tests: {
    suites: [{ name: 'app', files: ['app/modules/**/__tests__/**/*.test.ts'] }],
  },
})
