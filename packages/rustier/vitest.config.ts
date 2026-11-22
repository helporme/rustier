import { fileURLToPath, pathToFileURL } from 'node:url'
import { defineConfig, type Plugin } from 'vitest/config'
import { Server, type BackendOptions } from '../../crates/server/dist/rustier-server.js'

const crates = {
  'handy-crate': { path: fileURLToPath(new URL('./tests/handy-crate', import.meta.url)) },
}

const backends = {
  node: { type: 'node' },
  wasm32: { type: 'wasm', loading: 'node-js', mem64: false },
  wasm64: { type: 'wasm', loading: 'node-js', mem64: true },
} satisfies Record<string, BackendOptions>

function rustierTests(profile: string, backend: BackendOptions): Plugin {
  const runtime = `rustier:tests/${profile}/`
  const diags = `\0$build/diags/${profile}/`
  const server = new Server({ rootDir: process.cwd(), profiles: { [profile]: { backend, crates, procMacros: true } } })
  let revision = 0
  return {
    name: `rustier-tests-${profile}`,
    enforce: 'post',
    transform(code, id) {
      if (!id.endsWith('.test.ts') || !code.includes('rustier')) return
      const { edits } = server.transformModule(profile, id, code, runtime + ++revision)
      if (!edits.length) return
      for (const edit of edits.reverse()) {
        code = code.slice(0, edit.start) + edit.text + code.slice(edit.end)
      }
      return code
    },
    resolveId(id, importer) {
      if (id.startsWith(runtime)) return '\0' + id
      if (id === '$build/diags') return diags + importer
    },
    load(id) {
      if (id.startsWith(diags)) return `export const rustierDiags = ${JSON.stringify(server.check(profile))}`
      if (!id.startsWith('\0' + runtime)) return
      const { binFfiPath, diagnostics } = server.build(profile)
      if (!binFfiPath) this.error(diagnostics.join('\n') || 'No Rustier test binary')
      return server.renderJsFfi(profile, backend.type === 'node' ? binFfiPath : pathToFileURL(binFfiPath).href)
    },
  }
}

export default defineConfig({
  test: {
    maxWorkers: 1,
    fileParallelism: false,
    projects: [...Object.entries(backends).map(([name, backend]) => ({
      logLevel: 'error',
      plugins: [rustierTests(name, backend)],
      test: { name, include: ['tests/*.test.ts'], exclude: ['tests/js-macro-invalid.test.ts'] },
    })), {
      test: { name: 'invalid-js', include: ['tests/js-macro-invalid.test.ts'] },
    }],
  },
})
