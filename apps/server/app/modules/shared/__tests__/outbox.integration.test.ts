import { describe, expect, test } from '@c9up/helix'
import { claimOutboxBatch, deleteOutbox, enqueueOutbox, type OutboxDb } from '#modules/shared/infrastructure/outbox.js'
import { connectDirectDb, connectTestDb } from './support/db.js'

describe('the outbox, through BlackGate', () => {
  test('the application reads back what it kept, the database holds only ciphertext', async () => {
    const db = await connectTestDb()
    const direct = await connectDirectDb()
    const outbox = db as unknown as OutboxDb
    const id = crypto.randomUUID()
    const payload = { eventId: id, marker: `clear-${id}` }
    try {
      await enqueueOutbox(outbox, { id, eventType: 'IntegrationProbe', payload, delaySeconds: 0 })

      const claimed = (await claimOutboxBatch(outbox, { limit: 50, leaseSeconds: 60 })).find((r) => r.id === id)
      expect(claimed?.event_type).toBe('IntegrationProbe')
      expect(JSON.parse(String(claimed?.payload))).toEqual(payload)

      const rows = await direct.query('SELECT event_type, payload FROM outbox WHERE id = $1::uuid', [id])
      expect(rows.length).toBe(1)
      const raw = rows[0] ?? {}
      expect(typeof raw.event_type === 'string' && raw.event_type.length > 0).toBe(true)
      expect(typeof raw.payload === 'string' && raw.payload.length > 0).toBe(true)
      expect(String(raw.event_type)).not.toContain('IntegrationProbe')
      expect(String(raw.payload)).not.toContain(payload.marker)
    } finally {
      await deleteOutbox(outbox, id)
    }
  })
})
