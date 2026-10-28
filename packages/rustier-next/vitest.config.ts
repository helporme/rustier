import { defineConfig, type Plugin } from 'vitest/config'
import rustier, { type RustierProfile } from 'rustier-next/rolldown'

const profiles = {
  node: { platform: { type: 'node' } },
  wasm32: { platform: { type: 'wasm', loading: 'node-js', mem64: false } },
  wasm64: { platform: { type: 'wasm', loading: 'node-js', mem64: true } },
} satisfies Record<string, RustierProfile>

let activeProfile = 'node'
const plugin = rustier({ profiles, activeProfile: () => activeProfile }) as Plugin
const transform = plugin.transform!
const transformHandler = typeof transform === 'function' ? transform : transform.handler

export default defineConfig({
  test: {
    maxWorkers: 1,
    fileParallelism: false,
    projects: Object.keys(profiles).map(name => ({
      plugins: [{
        ...plugin,
        transform(code, id, options) {
          activeProfile = name
          return transformHandler.call(this, code, id, options)
        },
      }],
      test: {
        name,
        include: ['tests/*.test.ts'],
      },
    })),
  },
})

