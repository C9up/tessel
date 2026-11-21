import type { Mailbox } from '../application/Mailbox.js'

/**
 * The purge task (story 04-04), run by ream's scheduler on one instance at a time: what
 * was kept longer than the retention goes, acknowledged or not.
 */
export function purgeTask(mailbox: () => Promise<Mailbox>, retentionDays: number): () => Promise<void> {
  return async () => {
    await (await mailbox()).purge(retentionDays)
  }
}
