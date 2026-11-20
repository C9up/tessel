import router from '@c9up/ream/services/router'

// Liveness probe, for the host and for clients.
router.get('/health', async ({ response }) => {
  response.status(200).json({ name: 'tessel', status: 'running' })
})
