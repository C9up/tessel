// The real-time streams (relay): each module that pushes declares its channels in its
// http layer; `start/` only registers them, before the server listens.
import { registerDeliveryStreams } from './delivery.js'

registerDeliveryStreams()
