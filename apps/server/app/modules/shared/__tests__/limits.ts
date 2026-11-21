import limiter from '@c9up/blackhole/services/limiter'

/**
 * Forgets every rate-limit counter, between tests. Through a limiter of the default
 * store rather than `limiter.clear()`: that one clears only the limiters this process
 * has built so far, and the counters live in KeyDB (story 04-06), where an earlier run or
 * another instance left theirs. The store's own clear deletes every key it holds.
 */
export function clearLimits(): Promise<void> {
  return limiter.use({ requests: 1, duration: '1 minute' }).clear()
}
