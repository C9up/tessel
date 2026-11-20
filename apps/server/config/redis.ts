import { defineConfig } from '@c9up/quasar'
import env from '#start/env.js'

export default defineConfig({
  connection: 'main',
  connections: {
    main: {
      url: env.get('REDIS_URL'),
      maxRetriesPerRequest: null,
      // Connects on the first command: building the client opens no socket.
      lazyConnect: true,
    },
  },
})
