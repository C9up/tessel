// The recurring tasks, on ream's scheduler (web processes only: not in tests nor in a
// command). Each module declares its task in its http layer; `start/` registers it.
import type { Scheduler } from '@c9up/ream'
import app from '@c9up/ream/services/app'
import { registerDeliverySchedules } from './delivery.js'

registerDeliverySchedules(await app.container.make<Scheduler>('scheduler'))
