import { init } from 'rustier/cli'
import { test } from 'vitest'
import { snapshot } from '../snapshot'

test.each(['node', 'wasm', 'wasm-node'])('JS: %s', async (plan) => {
  const snap = await snapshot(plan, { variants: ['js'] })

  await init({ dir: snap.dir })

  await snap.assert()
})

test.each(['node', 'wasm', 'wasm-node'])('TS: %s', async (plan) => {
  const snap = await snapshot(plan, { variants: ['ts'] })

  await init({ dir: snap.dir })

  await snap.assert()
})

test.each(['node', 'wasm', 'wasm-node'])('JS with Cargo: %s', async (plan) => {
  const snap = await snapshot(plan, { variants: ['js', 'cargo'] })

  await init({ dir: snap.dir })

  await snap.assert()
})

test.each(['node', 'wasm', 'wasm-node'])('TS with Cargo: %s', async (plan) => {
  const snap = await snapshot(plan, { variants: ['ts', 'cargo'] })

  await init({ dir: snap.dir })

  await snap.assert()
})

test('repeated initialization', async () => {
  const snap = await snapshot('repeat-node', { variants: ['js', 'cargo'] })

  await init({ dir: snap.dir })
  await init({ dir: snap.dir })

  await snap.assert()
})
