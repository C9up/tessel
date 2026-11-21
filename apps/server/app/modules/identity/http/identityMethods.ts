import { clientKey, ThrottleException } from '@c9up/blackhole'
import limiter from '@c9up/blackhole/services/limiter'
import type { HttpContext } from '@c9up/ream'
import { rules, schema } from '@c9up/rune'
import {
  type AccountCreated,
  type Challenge,
  type ClaimedKeyPackages,
  type ClaimKeyPackagesParams,
  type CreateAccountParams,
  type DeviceLinked,
  type DeviceList,
  type DeviceRevoked,
  fromBase64Url,
  type KeyPackageStock,
  type LinkGrantTaken,
  type ProfileSet,
  type SealedProfile,
  type Session,
  type SignedIn,
  type SignedOut,
  type SignInParams,
  toBase64Url,
} from '@tessel/protocol'
import { RpcFailure } from '#modules/shared/http/rpc.js'
import type { AccountDevices } from '../application/AccountDevices.js'
import type { AccountProfiles } from '../application/AccountProfiles.js'
import type { Authenticate } from '../application/Authenticate.js'
import type { CreateAccount } from '../application/CreateAccount.js'
import type { KeyPackageDirectory } from '../application/KeyPackageDirectory.js'
import type { LinkDevice } from '../application/LinkDevice.js'
import type { SignIn } from '../application/SignIn.js'
import { IdentityError } from '../domain/account.js'
import { type KeyPackageUpload, LOW_STOCK, MAX_BATCH } from '../domain/keyPackages.js'
import { MAX_PROFILE_BLOB } from '../domain/profile.js'
import { isDeviceKind, type SignedInDevice } from '../domain/session.js'

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

/**
 * KeyPackage claims by one device, per minute: enough to start a large group, not to
 * drain everyone's stock. Counted per claiming device: claims need a session.
 */
export const CLAIMS = { requests: 60, duration: '1 minute' } as const

/** Uploads by one device, per minute: a device renews its pool rarely. */
export const UPLOADS = { requests: 10, duration: '1 minute' } as const

/** Devices one device links, per hour. */
export const LINKS = { requests: 10, duration: '1 hour' } as const

/**
 * Grant polls from one network, per minute: the new device asks every few seconds while
 * the QR code is on screen. The link identifier is 128 random bits: guessing one is
 * hopeless, this only bounds the work.
 */
export const GRANT_POLLS = { requests: 60, duration: '1 minute' } as const

/** Revocations by one device, per hour. */
export const REVOCATIONS = { requests: 10, duration: '1 hour' } as const

/** Profile changes by one device, per hour. */
export const PROFILE_CHANGES = { requests: 20, duration: '1 hour' } as const

/** Profile fetches by one device, per minute: a contact list's worth. */
export const PROFILE_FETCHES = { requests: 120, duration: '1 minute' } as const

/** A sealed grant: a nonce, a certificate, a tag. Well under this. */
const MAX_GRANT = 1024

/** A KeyPackage of Tessel's suite is well under this many bytes. */
const MAX_KEY_PACKAGE = 4096

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/** The bytes of a base64url string of at most `max` bytes; `undefined` otherwise. */
function decoded(value: unknown, max: number): Uint8Array | undefined {
  if (typeof value !== 'string' || value.length > Math.ceil((max * 4) / 3)) return undefined
  return fromBase64Url(value)
}

/** Every entry decoded; `undefined` when one is not base64url of at most `max` bytes. */
function decodedAll(values: unknown, max: number): Uint8Array[] | undefined {
  if (!Array.isArray(values)) return undefined
  const out: Uint8Array[] = []
  for (const value of values) {
    const bytes = decoded(value, max)
    if (bytes === undefined) return undefined
    out.push(bytes)
  }
  return out
}

/** The upload of `identity.publishKeyPackages`, decoded; `undefined` when it is not one. */
function upload(params: unknown): KeyPackageUpload | undefined {
  if (!isRecord(params)) return undefined
  const publish = decodedAll(params.publish, MAX_KEY_PACKAGE)
  const withdraw = decodedAll(params.withdraw, 64)
  const lastResort = params.lastResort === undefined ? undefined : decoded(params.lastResort, MAX_KEY_PACKAGE)
  if (publish === undefined || withdraw === undefined || (params.lastResort !== undefined && lastResort === undefined)) return undefined
  if (publish.length + withdraw.length > MAX_BATCH) return undefined
  return lastResort === undefined ? { publish, withdraw } : { publish, withdraw, lastResort }
}

const ClaimValidator = schema<ClaimKeyPackagesParams>({
  account: rules.string().max(320),
})

const deviceKey = (device: SignedInDevice) => Buffer.from(device.deviceId).toString('hex')

const base64url = (value: unknown) => typeof value === 'string' && fromBase64Url(value) !== undefined

const CreateAccountValidator = schema<CreateAccountParams>({
  certificate: rules.string().max(MAX_CERTIFICATE).custom('base64url', base64url, 'not base64url'),
})

const SignInValidator = schema<SignInParams>({
  device: rules.string().max(32).custom('base64url', base64url, 'not base64url'),
  challenge: rules.string().max(64).custom('base64url', base64url, 'not base64url'),
  proof: rules.string().max(128).custom('base64url', base64url, 'not base64url'),
  kind: rules.string().custom('kind', isDeviceKind, 'web, android or ios'),
})

function refused(error: unknown): never {
  if (error instanceof IdentityError) {
    if (error.code === 'nameTaken') throw new RpcFailure('E_IDENTITY_NAME_TAKEN')
    if (error.code === 'wrongDomain') throw new RpcFailure('E_IDENTITY_WRONG_DOMAIN')
    if (error.code === 'signInRefused') throw new RpcFailure('E_IDENTITY_SIGN_IN_REFUSED')
    if (error.code === 'unauthenticated') throw new RpcFailure('E_UNAUTHENTICATED')
    if (error.code === 'invalidKeyPackage') throw new RpcFailure('E_IDENTITY_INVALID_KEY_PACKAGE')
    if (error.code === 'stockFull') throw new RpcFailure('E_IDENTITY_KEY_PACKAGE_STOCK_FULL')
    if (error.code === 'tooManyDevices') throw new RpcFailure('E_IDENTITY_TOO_MANY_DEVICES')
    if (error.code === 'linkUsed') throw new RpcFailure('E_IDENTITY_LINK_USED')
    if (error.code === 'unknownDevice') throw new RpcFailure('E_IDENTITY_UNKNOWN_DEVICE')
    if (error.code === 'invalidRevocation') throw new RpcFailure('E_IDENTITY_INVALID_REVOCATION')
    if (error.code === 'invalidProfile') throw new RpcFailure('E_IDENTITY_INVALID_PROFILE')
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
  directory: () => Promise<KeyPackageDirectory>
  link: () => Promise<LinkDevice>
  devices: () => Promise<AccountDevices>
  profiles: () => Promise<AccountProfiles>
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
export function identityMethods(
  deps: IdentityDeps,
): Record<
  | 'createAccount'
  | 'challenge'
  | 'signIn'
  | 'session'
  | 'signOut'
  | 'publishKeyPackages'
  | 'keyPackageStock'
  | 'claimKeyPackages'
  | 'linkDevice'
  | 'takeLinkGrant'
  | 'devices'
  | 'revokeDevice'
  | 'setProfile'
  | 'profile',
  Handler
> {
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
        const kind = parsed.data.kind
        if (!isDeviceKind(kind)) throw new RpcFailure('E_INVALID_PARAMS')
        const token = await (await deps.signIn()).run({ deviceId, challenge, proof, kind })
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

    publishKeyPackages: async (ctx, params) => {
      const device = await guard(ctx)
      try {
        await limiter.use(UPLOADS).consume(`identity_upload_${deviceKey(device)}`)
        const parsed = upload(params)
        if (parsed === undefined) throw new RpcFailure('E_INVALID_PARAMS')
        const stock = await (await deps.directory()).publish(device, parsed)
        return { stock, low: stock < LOW_STOCK } satisfies KeyPackageStock
      } catch (error) {
        refused(error)
      }
    },

    keyPackageStock: async (ctx) => {
      const device = await guard(ctx)
      return (await (await deps.directory()).stock(device)) satisfies KeyPackageStock
    },

    claimKeyPackages: async (ctx, params) => {
      const device = await guard(ctx)
      try {
        await limiter.use(CLAIMS).consume(`identity_claim_${deviceKey(device)}`)
        const parsed = ClaimValidator.validateResult(params ?? {})
        if (!parsed.valid) throw new RpcFailure('E_INVALID_PARAMS', parsed.errors)
        const claimed = await (await deps.directory()).claim(device, parsed.data.account)
        return {
          keyPackages: claimed.map((entry) => ({ device: toBase64Url(entry.deviceId), keyPackage: toBase64Url(entry.keyPackage) })),
        } satisfies ClaimedKeyPackages
      } catch (error) {
        refused(error)
      }
    },

    linkDevice: async (ctx, params) => {
      const device = await guard(ctx)
      try {
        await limiter.use(LINKS).consume(`identity_link_${deviceKey(device)}`)
        if (!isRecord(params)) throw new RpcFailure('E_INVALID_PARAMS')
        const certificate = decoded(params.certificate, 2048)
        const link = decoded(params.link, 16)
        const grant = decoded(params.grant, MAX_GRANT)
        if (certificate === undefined || link?.length !== 16 || grant === undefined || grant.length === 0) throw new RpcFailure('E_INVALID_PARAMS')
        await (await deps.link()).run(device, certificate, link, grant)
        return { linked: true } satisfies DeviceLinked
      } catch (error) {
        refused(error)
      }
    },

    takeLinkGrant: async (ctx, params) => {
      try {
        await limiter.use(GRANT_POLLS).consume(`identity_grant_${clientKey(ctx.request.ip())}`)
        const link = isRecord(params) ? decoded(params.link, 16) : undefined
        if (link?.length !== 16) throw new RpcFailure('E_INVALID_PARAMS')
        const grant = await (await deps.link()).takeGrant(link)
        return (grant === null ? {} : { grant: toBase64Url(grant) }) satisfies LinkGrantTaken
      } catch (error) {
        refused(error)
      }
    },

    devices: async (ctx) => {
      const device = await guard(ctx)
      const current = deviceKey(device)
      const devices = await (await deps.devices()).list(device)
      return {
        devices: devices.map((entry) => ({
          device: toBase64Url(entry.deviceId),
          createdOn: entry.createdOn,
          current: Buffer.from(entry.deviceId).toString('hex') === current,
          ...(entry.kind === null ? {} : { kind: entry.kind }),
          ...(entry.lastActiveOn === null ? {} : { lastActiveOn: entry.lastActiveOn }),
        })),
      } satisfies DeviceList
    },

    revokeDevice: async (ctx, params) => {
      const device = await guard(ctx)
      try {
        await limiter.use(REVOCATIONS).consume(`identity_revoke_${deviceKey(device)}`)
        if (!isRecord(params)) throw new RpcFailure('E_INVALID_PARAMS')
        const target = decoded(params.device, 16)
        const revocation = params.revocation === undefined ? undefined : decoded(params.revocation, 64)
        if (target?.length !== 16 || (params.revocation !== undefined && revocation === undefined)) throw new RpcFailure('E_INVALID_PARAMS')
        await (await deps.devices()).revoke(device, target, revocation)
        return { revoked: true } satisfies DeviceRevoked
      } catch (error) {
        refused(error)
      }
    },

    setProfile: async (ctx, params) => {
      const device = await guard(ctx)
      try {
        await limiter.use(PROFILE_CHANGES).consume(`identity_profile_set_${deviceKey(device)}`)
        const blob = isRecord(params) ? decoded(params.blob, MAX_PROFILE_BLOB) : undefined
        if (blob === undefined) throw new RpcFailure('E_INVALID_PARAMS')
        await (await deps.profiles()).set(device, blob)
        return { saved: true } satisfies ProfileSet
      } catch (error) {
        refused(error)
      }
    },

    profile: async (ctx, params) => {
      const device = await guard(ctx)
      try {
        await limiter.use(PROFILE_FETCHES).consume(`identity_profile_get_${deviceKey(device)}`)
        const account = isRecord(params) ? params.account : undefined
        if (typeof account !== 'string' || account.length > 320) throw new RpcFailure('E_INVALID_PARAMS')
        const blob = await (await deps.profiles()).get(account)
        return (blob === null ? {} : { blob: toBase64Url(blob) }) satisfies SealedProfile
      } catch (error) {
        refused(error)
      }
    },
  }
}
