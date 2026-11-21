import { redis } from '#modules/shared/infrastructure/redis.js'
import type { LinkMailbox } from '../domain/ports.js'

/** Seconds a grant waits for its device: the time to scan and confirm, no more. */
const GRANT_TTL_SEC = 300

const key = (link: Uint8Array) => `identity:link:${Buffer.from(link).toString('hex')}`

/**
 * The link mailboxes, in KeyDB and nowhere else: a sealed grant the server cannot open,
 * under a random link identifier, for five minutes. `SET NX` keeps the first grant;
 * `GETDEL` hands it out once.
 */
export const keyDbLinkMailbox: LinkMailbox = {
  async post(link, grant) {
    return (await redis.set(key(link), Buffer.from(grant).toString('base64'), 'EX', GRANT_TTL_SEC, 'NX')) === 'OK'
  },
  async take(link) {
    const grant = await redis.getdel(key(link))
    return grant === null ? null : new Uint8Array(Buffer.from(grant, 'base64'))
  },
}
