import { defineConfig, locks } from '@c9up/ream/scheduler/config'

/** Recurring tasks run on one instance at a time: the lock lives in KeyDB. */
export default defineConfig({
  lock: locks.redis({ connection: 'main' }),
})
