import { clientKey, ThrottleException } from '@c9up/blackhole'
import limiter from '@c9up/blackhole/services/limiter'
import type { HttpContext } from '@c9up/ream'
import { rules, schema } from '@c9up/rune'
import {
  type AccountCreated,
  type Challenge,
  type CreateAccountParams,
  fromBase64Url,
  type Session,
  type SignedIn,
  type SignedOut,
  type SignInParams,
  toBase64Url,
} from '@tessel/protocol'
import { RpcFailure } from '#modules/shared/http/rpc.js'
import type { Authenticate } from '../application/Authenticate.js'
import type { CreateAccount } from '../application/CreateAccount.js'
import type { SignIn } from '../application/SignIn.js'
import { IdentityError } from '../domain/account.js'
import type { SignedInDevice } from '../domain/session.js'

type Handler = (ctx: HttpContext, params: unknown) => Promise<unknown>

/** A certificate is a few hundred bytes; anything far larger is not one. */
const MAX_CERTIFICATE = 2048

/** Account creations from one network (an IPv4 address, an IPv6 /64), per hour. */
export const ACCOUNT_CREATIONS = { requests: 5, duration: '1 hour' } as const

/** Challenges one network draws, per minute: a device needs one per sign-in. */
export const CHALLENGES = { requests: 30, duration: '1 minute' } as const

/**
 * Sign-ins from one network, per minute. Counted per network, not per device: a count
 * per device would let anyone who knows a device's identifier (its contacts do) lock it
 * out. A signature cannot be guessed anyway; this bounds the work an attacker costs.
 */
export const SIGN_INS = { requests: 10, duration: '1 minute' } as const

const base64url = (value: unknown) => typeof value === 'string' && fromBase64Url(value) !== undefined

const CreateAccountValidator = schema<CreateAccountParams>({
  certificate: rules.string().max(MAX_CERTIFICATE).custom('base64url', base64url, 'not base64url'),
})

const SignInValidator = schema<SignInParams>({
  device: rules.string().max(32).custom('base64url', base64url, 'not base64url'),
  challenge: rules.string().max(64).custom('base64url', base64url, 'not base64url'),
  proof: rules.string().max(128).custom('base64url', base64url, 'not base64url'),
})

function refused(error: unknown): never {
  if (error instanceof IdentityError) {
    if (error.code === 'nameTaken') throw new RpcFailure('E_IDENTITY_NAME_TAKEN')
    if (error.code === 'wrongDomain') throw new RpcFailure('E_IDENTITY_WRONG_DOMAIN')
    if (error.code === 'signInRefused') throw new RpcFailure('E_IDENTITY_SIGN_IN_REFUSED')
    if (error.code === 'unauthenticated') throw new RpcFailure('E_UNAUTHENTICATED')
    throw new RpcFailure('E_IDENTITY_INVALID_CERTIFICATE')
  }
  if (error instanceof ThrottleException) throw new RpcFailure('E_TOO_MANY_REQUESTS', { retryIn: error.response.availableIn })
  throw error
}

/** The token of `Authorization: Bearer <token>`; `''` when there is none. */
export function bearerToken(ctx: HttpContext): string {
  const [scheme, token] = (ctx.request.header('authorization') ?? '').split(' ')
  return scheme?.toLowerCase() === 'bearer' && token ? token : ''
}

/** What the identity module's points need. */
export interface IdentityDeps {
  createAccount: () => Promise<CreateAccount>
  signIn: () => Promise<SignIn>
  authenticate: () => Promise<Authenticate>
}

/**
 * The device behind the call's session, for every point that needs one: this module's
 * and the others'. Throws `E_UNAUTHENTICATED` otherwise.
 */
export function sessionGuard(deps: Pick<IdentityDeps, 'authenticate'>): (ctx: HttpContext) => Promise<SignedInDevice> {
  return async (ctx) => {
    try {
      return await (await deps.authenticate()).run(bearerToken(ctx))
    } catch (error) {
      refused(error)
    }
  }
}

/**
 * The identity module's points. `identity.createAccount`, `identity.challenge` and
 * `identity.signIn` are public (there is no session yet); the network they come from is
 * counted, for as long as the window lasts, and kept nowhere else.
 */
export function identityMethods(deps: IdentityDeps): Record<'createAccount' | 'challenge' | 'signIn' | 'session' | 'signOut', Handler> {
  const guard = sessionGuard(deps)
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

    challenge: async (ctx) => {
      try {
        await limiter.use(CHALLENGES).consume(`identity_challenge_${clientKey(ctx.request.ip())}`)
        return { challenge: toBase64Url(await (await deps.signIn()).challenge()) } satisfies Challenge
      } catch (error) {
        refused(error)
      }
    },

    signIn: async (ctx, params) => {
      try {
        await limiter.use(SIGN_INS).consume(`identity_sign_in_${clientKey(ctx.request.ip())}`)
        const parsed = SignInValidator.validateResult(params ?? {})
        if (!parsed.valid) throw new RpcFailure('E_INVALID_PARAMS', parsed.errors)
        const deviceId = fromBase64Url(parsed.data.device)
        const challenge = fromBase64Url(parsed.data.challenge)
        const proof = fromBase64Url(parsed.data.proof)
        if (deviceId?.length !== 16 || challenge?.length !== 32 || proof?.length !== 64) throw new RpcFailure('E_INVALID_PARAMS')
        const token = await (await deps.signIn()).run({ deviceId, challenge, proof })
        return { token } satisfies SignedIn
      } catch (error) {
        refused(error)
      }
    },

    session: async (ctx) => {
      const device = await guard(ctx)
      return { account: device.account, device: toBase64Url(device.deviceId) } satisfies Session
    },

    signOut: async (ctx) => {
      try {
        await (await deps.authenticate()).signOut(bearerToken(ctx))
        return { signedOut: true } satisfies SignedOut
      } catch (error) {
        refused(error)
      }
    },
  }
}
