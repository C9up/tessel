import { Duration } from '@c9up/chronos'
import env from '#start/env.js'

/**
 * Delivery (stories 04-01 to 04-04). An envelope goes as soon as every recipient has
 * acknowledged it; one never acknowledged goes after `retention`, as does what an
 * acknowledged one leaves behind (its deduplication key).
 */
export default {
  retention: Duration.fromObject({ days: env.get('DELIVERY_RETENTION_DAYS') ?? 30 }),
  // The purge runs every hour, at minute 17 (not on the hour, when everything else runs).
  purgeSchedule: '17 * * * *',
}
