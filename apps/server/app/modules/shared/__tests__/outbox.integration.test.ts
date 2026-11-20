import { createNapiConnection } from '@c9up/atlas'
import { describe, expect, test } from '@c9up/helix'
import { claimOutboxBatch, deleteOutbox, enqueueOutbox, type OutboxDb } from '#modules/shared/infrastructure/outbox.js'
import env from '#start/env.js'
import { connectTestDb, weakenedUrl } from './support/db.js'

describe('the database, over mutual TLS', () => {
  test('the application reads back what it kept in the outbox', async () => {
    const db = await connectTestDb()
    const outbox = db as unknown as OutboxDb
    const id = crypto.randomUUID()
    const payload = { eventId: id, marker: `probe-${id}` }
    try {
      await enqueueOutbox(outbox, { id, eventType: 'IntegrationProbe', payload, delaySeconds: 0 })

      const claimed = (await claimOutboxBatch(outbox, { limit: 50, leaseSeconds: 60 })).find((r) => r.id === id)
      expect(claimed?.event_type).toBe('IntegrationProbe')
      expect(JSON.parse(String(claimed?.payload))).toEqual(payload)
    } finally {
      await deleteOutbox(outbox, id)
    }
  })

  test('the same URL with mutual TLS connects (control for the two refusals below)', async () => {
    const url = weakenedUrl({
      sslmode: 'verify-full',
      sslrootcert: env.get('DB_SSL_ROOT_CERT'),
      sslcert: env.get('DB_SSL_CERT'),
      sslkey: env.get('DB_SSL_KEY'),
    })
    const db = await createNapiConnection(url, 1, 1)
    expect((await db.query('SELECT 1 AS one')).length).toBe(1)
  })

  test('a connection without TLS is refused', async () => {
    await expect(createNapiConnection(weakenedUrl({ sslmode: 'disable' }), 1, 1)).rejects.toThrow()
  })

  test('a TLS connection without a client certificate is refused', async () => {
    const url = weakenedUrl({ sslmode: 'verify-full', sslrootcert: env.get('DB_SSL_ROOT_CERT') })
    await expect(createNapiConnection(url, 1, 1)).rejects.toThrow()
  })
})
