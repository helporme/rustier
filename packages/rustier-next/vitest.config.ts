import { defineConfig, type Plugin } from 'vitest/config'
import rustier, { type RustierProfile } from 'rustier-next/rolldown'

const profiles = {
  node: { platform: { type: 'node' } },
  wasm32: { platform: { type: 'wasm', loading: 'node-js', mem64: false } },
  wasm64: { platform: { type: 'wasm', loading: 'node-js', mem64: true } },
} satisfies Record<string, RustierProfile>

let activeProfile = 'node'
const plugin = rustier({ profiles, activeProfile: () => activeProfile })
const transform = (plugin as Plugin).transform!
const transformHandler = typeof transform === 'function' ? transform : transform.handler

function diagnosticsPlugin(profile: string): Plugin {
  const prefix = `\0$build/diags/${profile}/`

  return {
    name: 'rustier-test-diagnostics',
    resolveId(source, importer) {
      if (source === '$build/diags') return prefix + encodeURIComponent(importer ?? '')
    },
    load(id) {
      if (id.startsWith(prefix)) {
        return `export const rustierDiags = ${JSON.stringify(plugin.api.check(profile))};`
      }
    },
  }
}

export default defineConfig({
  test: {
    maxWorkers: 1,
    fileParallelism: false,
    projects: Object.keys(profiles).map(name => ({
      logLevel: 'error',
      plugins: [{
        ...plugin,
        transform(code, id, options) {
          activeProfile = name
          return transformHandler.call(this, code, id, options)
        },
      }, diagnosticsPlugin(name)],
      test: {
        name,
        include: ['tests/*.test.ts'],
      },
    })),
  },
})

