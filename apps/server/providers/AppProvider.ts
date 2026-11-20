import { Provider } from '@c9up/ream'
import { setOutboxActive } from '#modules/shared/infrastructure/events.js'
import { outboxRelay } from '#modules/shared/infrastructure/outboxRelay.js'

export default class AppProvider extends Provider {
  register() {}
  async boot() {}
  async start() {}
  async ready() {}

  /** Arrêter le relais et l'écriture dans l'outbox. Leur démarrage vit dans `bin/server.ts`. */
  async shutdown() {
    setOutboxActive(false)
    outboxRelay.stop()
  }
}
