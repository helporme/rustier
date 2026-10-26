import type { Plugin } from 'rolldown'
import { Server } from 'server-next'
import { buildRuntime, rewriteRuntimeImports } from './bundle'
import { toProfileOptions, type Platform } from './options'

export type { Platform } from './options'

export interface RustierOptions {
  release?: boolean
  platform: Platform
  crates?: Record<string, { path: string }>
}

export default function rustier(options: RustierOptions): Plugin {
  const serverOptions = {
    rootDir: process.cwd(),
    profiles: { default: toProfileOptions(options, options.platform, false) },
  }
  const ffiJsModPath = 'rustier:runtime'
  const binFileName = options.platform.type === 'node' ? 'bridge.node' : 'bridge.wasm'
  const server = new Server(serverOptions)
  const sources = new Map<string, string>()
  let needsRuntime = false
  let runtimeFileName: string | null = null

  return {
    name: 'rustier-next',
    buildStart() {
      server.reset('default')
      for (const [id, code] of sources) server.transformModule('default', id, code, ffiJsModPath)
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
      const transformed = server.transformModule('default', id, code, ffiJsModPath)
      sources.set(id, code)
      return { code: transformed, map: null }
    },
    buildEnd(error) {
      if (error) return
      const modules = new Set(this.getModuleIds())
      for (const id of sources.keys()) {
        if (modules.has(id)) continue
        server.removeModule('default', id)
        sources.delete(id)
      }
      if (!needsRuntime && !modules.has(ffiJsModPath)) return
      const result = buildRuntime(server, 'default', binFileName)
      for (const diagnostic of result.diagnostics) this.warn(diagnostic)
      runtimeFileName = result.runtimeFileName
      this.emitFile({ type: 'asset', fileName: binFileName, source: result.binary })
      this.emitFile({ type: 'asset', fileName: runtimeFileName, source: result.runtime })
    },
    renderStart(output) {
      if (runtimeFileName != null && output.format !== 'es') {
        this.error('Rustier runtime assets require ES module output')
      }
    },
    renderChunk(code, chunk) {
      if (runtimeFileName == null || !chunk.imports.includes(ffiJsModPath)) return
      return rewriteRuntimeImports(code, chunk.fileName, ffiJsModPath, runtimeFileName)
    },
  }
}
