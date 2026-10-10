import { init } from 'rustier/cli'
import { test } from 'vitest'
import { snapshot } from '../snapshot'

test.each([
  'cli-node',
  'cli-wasm',
  'cli-wasm-node',
  'library-node',
  'library-wasm',
  'library-wasm-node',
  'sveltekit-node',
  'sveltekit-wasm',
  'sveltekit-wasm-node',
])('%s', async (plan) => {
  const snap = await snapshot(plan)

  await init({ dir: snap.dir })

  await snap.assert()
})
