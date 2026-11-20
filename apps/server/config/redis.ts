import { defineConfig } from '@c9up/quasar'
import env from '#start/env.js'

export default defineConfig({
  connection: 'main',
  connections: {
    main: {
      url: env.get('REDIS_URL'),
      maxRetriesPerRequest: null,
      // Connexion à la première commande : bâtir le client n'ouvre aucun socket.
      lazyConnect: true,
    },
  },
})
