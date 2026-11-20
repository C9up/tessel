import { test } from '@c9up/helix'

test('the health probe answers', async ({ client }) => {
  await client.get('/health').assertOk()
})
