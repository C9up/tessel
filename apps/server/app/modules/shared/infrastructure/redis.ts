import type { QuasarConnection } from '@c9up/quasar'
import quasar from '@c9up/quasar/services/main'

/**
 * The shared KeyDB connection, served by `@c9up/quasar`, as a lazy accessor:
 * importing this file opens no socket, the connection is resolved only when a
 * method is called (taken from nadeo).
 */
export const redis: QuasarConnection = new Proxy(Object.create(null), {
  get(_target, prop) {
    const connection = quasar.connection()
    const value = Reflect.get(connection, prop, connection)
    return typeof value === 'function' ? value.bind(connection) : value
  },
})
