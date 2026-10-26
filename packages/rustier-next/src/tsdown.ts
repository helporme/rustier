import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { posix } from 'node:path'
import { RolldownMagicString, type Plugin } from 'rolldown'
import { parseSync } from 'rolldown/utils'
import { Server } from 'server-next'

export type Platform = {
  type: 'wasm'
  loading: 'web' | 'node-js'
  mem64?: boolean
}

export interface RustierOptions {
  release?: boolean
  platform: Platform
  crates?: Record<string, { path: string }>
}

export default function rustier(options: RustierOptions): Plugin {
  const serverOptions = {
    rootDir: process.cwd(),
    release: options.release ?? false,
    mem64: options.platform.mem64 ?? false,
    wasmImport: options.platform.loading === 'node-js' ? 'node-js' : 'fetch',
    crates: Object.fromEntries(
      Object.entries(options.crates ?? {}).map(([name, crate]) => [name, crate.path]),
    ),
  }
  const ffiJsModPath = 'rustier:runtime'
  const binFilePath = './bridge.wasm'
  let server = new Server(serverOptions)
  let needsRuntime = false
  let runtimeFileName: string | null = null

  return {
    name: 'rustier-next',
    buildStart() {
      server = new Server(serverOptions)
      needsRuntime = false
      runtimeFileName = null
    },
    resolveId(id) {
      if (id !== ffiJsModPath) return
      needsRuntime = true
      // Do not load the runtime until all macros have been collected.
      return { id, external: true }
    },
    transform(code, id) {
      if (id.startsWith('\0') || id.includes('/node_modules/')) return
      if (/\.d\.[cm]?ts(?:[?#]|$)/.test(id)) return
      if (!/\.[cm]?[jt]sx?(?:[?#]|$)/.test(id)) return
      return { code: server.transform(id, code, ffiJsModPath), map: null }
    },
    buildEnd(error) {
      if (error || !needsRuntime) return
      const result = server.build()
      if (!result.binFfiPath) this.error(result.diagnostics.join('\n') || 'Rustier build failed')
      for (const diagnostic of result.diagnostics) this.warn(diagnostic)
      const binary = readFileSync(result.binFfiPath)
      const runtime = server.renderJsFfi(binFilePath)
      // A changed binary must also change the runtime URL, including in watch mode.
      const hash = createHash('sha256').update(runtime).update(binary).digest('hex').slice(0, 16)
      runtimeFileName = `runtime-${hash}.mjs`
      this.emitFile({ type: 'asset', fileName: 'bridge.wasm', source: binary })
      this.emitFile({ type: 'asset', fileName: runtimeFileName, source: runtime })
    },
    renderStart(output) {
      if (runtimeFileName != null && output.format !== 'es') {
        this.error('Rustier runtime assets require ES module output')
      }
    },
    renderChunk(code, chunk) {
      if (runtimeFileName == null || !chunk.imports.includes(ffiJsModPath)) return
      const relative = posix.relative(posix.dirname(chunk.fileName), runtimeFileName)
      const path = relative.startsWith('.') ? relative : `./${relative}`
      const result = new RolldownMagicString(code)
      // Rewrite import specifiers, never strings in application code.
      const parsed = parseSync(chunk.fileName, code)
      for (const statement of parsed.program.body) {
        if (statement.type === 'ImportDeclaration' && statement.source.value === ffiJsModPath) {
          result.overwrite(statement.source.start, statement.source.end, JSON.stringify(path))
        }
      }
      return { code: result.toString(), map: result.generateMap({ hires: true }).toString() }
    },
  }
}
