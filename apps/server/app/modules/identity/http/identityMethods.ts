import { clientKey, ThrottleException } from '@c9up/blackhole'
import limiter from '@c9up/blackhole/services/limiter'
import type { HttpContext } from '@c9up/ream'
import { rules, schema } from '@c9up/rune'
import { type AccountCreated, type CreateAccountParams, fromBase64Url } from '@tessel/protocol'
import { RpcFailure } from '#modules/shared/http/rpc.js'
import type { CreateAccount } from '../application/CreateAccount.js'
import { IdentityError } from '../domain/account.js'

type Handler = (ctx: HttpContext, params: unknown) => Promise<unknown>

/** A certificate is a few hundred bytes; anything far larger is not one. */
const MAX_CERTIFICATE = 2048

/** Account creations from one network (an IPv4 address, an IPv6 /64), per hour. */
export const ACCOUNT_CREATIONS = { requests: 5, duration: '1 hour' } as const

const CreateAccountValidator = schema<CreateAccountParams>({
  certificate: rules
    .string()
    .max(MAX_CERTIFICATE)
    .custom('base64url', (value) => typeof value === 'string' && fromBase64Url(value) !== undefined, 'not base64url'),
})

function refused(error: unknown): never {
  if (error instanceof IdentityError) {
    if (error.code === 'nameTaken') throw new RpcFailure('E_IDENTITY_NAME_TAKEN')
    if (error.code === 'wrongDomain') throw new RpcFailure('E_IDENTITY_WRONG_DOMAIN')
    throw new RpcFailure('E_IDENTITY_INVALID_CERTIFICATE')
  }
  if (error instanceof ThrottleException) throw new RpcFailure('E_TOO_MANY_REQUESTS', { retryIn: error.response.availableIn })
  throw error
}

/**
 * The identity module's points. `identity.createAccount` is public (there is no account
 * yet to sign with); the network it comes from is counted, for as long as the window
 * lasts, and kept nowhere else.
 */
export function identityMethods(deps: { createAccount: () => Promise<CreateAccount> }): { createAccount: Handler } {
  return {
    createAccount: async (ctx, params) => {
      try {
        await limiter.use(ACCOUNT_CREATIONS).consume(`identity_create_${clientKey(ctx.request.ip())}`)
        const parsed = CreateAccountValidator.validateResult(params ?? {})
        if (!parsed.valid) throw new RpcFailure('E_INVALID_PARAMS', parsed.errors)
        const certificate = fromBase64Url(parsed.data.certificate)
        if (certificate === undefined) throw new RpcFailure('E_INVALID_PARAMS')
        const account = await (await deps.createAccount()).run(certificate)
        return { account } satisfies AccountCreated
      } catch (error) {
        refused(error)
      }
    },
  }
}
