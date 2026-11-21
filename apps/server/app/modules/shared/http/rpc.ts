import { ServerError, type ServerErrorName } from '@tessel/protocol'

/**
 * A refusal a point THROWS: ream's `RpcRouter` answers any error with a numeric `code`
 * as the JSON-RPC error `{ code, message, data }` (taken from nadeo). The message is the
 * stable name of `@tessel/protocol`'s `ServerError`; the client translates it.
 */
export class RpcFailure extends Error {
  readonly code: number
  readonly data?: unknown

  constructor(name: ServerErrorName, data?: unknown) {
    super(name)
    this.name = 'RpcFailure'
    this.code = ServerError[name]
    this.data = data
  }
}
