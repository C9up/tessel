import { randomBytes } from 'node:crypto'
import { redis } from '#modules/shared/infrastructure/redis.js'
import type { Challenges } from '../domain/ports.js'

/** Seconds a challenge stays good: long enough to sign, too short to keep. */
const CHALLENGE_TTL_SEC = 60

const key = (challenge: Uint8Array) => `identity:challenge:${Buffer.from(challenge).toString('hex')}`

/**
 * The challenges, in KeyDB and nowhere else: a random value, for a minute, naming no
 * device and no network. Deleting it is what spends it, so two attempts with the same
 * challenge cannot both pass.
 */
export const keyDbChallenges: Challenges = {
  async issue() {
    const challenge = randomBytes(32)
    await redis.set(key(challenge), '1', 'EX', CHALLENGE_TTL_SEC)
    return new Uint8Array(challenge)
  },
  async take(challenge) {
    if (challenge.length !== 32) return false
    return (await redis.del(key(challenge))) === 1
  },
}
