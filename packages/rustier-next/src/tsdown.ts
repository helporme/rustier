import { readFileSync } from 'node:fs'
import type { Plugin } from 'rolldown'
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

  let server = new Server(serverOptions)
  let binaryReference: string | null = null
  let binary: Uint8Array | null = null

  return {
    name: 'rustier-next',
    buildStart() {
      server = new Server(serverOptions)
      binaryReference = null
      binary = null
    },
    resolveId(id) {
      if (id === server.binFfiModId || server.jsFfiModule(id) != null) return id
    },
    load(id) {
      if (id === server.binFfiModId) {
        // Rolldown requires an asset source at emission time. A fixed filename
        // lets us fill the reserved asset later without invalidating its URL/hash.
        binaryReference ??= this.emitFile({
          type: 'asset',
          fileName: 'bridge.wasm',
          source: new Uint8Array(),
        })
        return `export default import.meta.ROLLUP_FILE_URL_${binaryReference};`
      }
      return server.jsFfiModule(id)
    },
    transform(code, id) {
      if (id.startsWith('\0') || id.includes('/node_modules/')) return
      if (id === server.binFfiModId || server.jsFfiModule(id) != null) return
      if (/\.d\.[cm]?ts(?:[?#]|$)/.test(id)) return
      if (!/\.[cm]?[jt]sx?(?:[?#]|$)/.test(id)) return
      return { code: server.transform(id, code), map: null }
    },
    buildEnd(error) {
      if (error || binaryReference == null) return
      const result = server.build()
      if (!result.binFfiPath) this.error(result.diagnostics.join('\n') || 'Rustier build failed')
      for (const diagnostic of result.diagnostics) this.warn(diagnostic)
      binary = readFileSync(result.binFfiPath)
    },
    generateBundle(_, bundle) {
      if (binaryReference == null) return
      if (binary == null) this.error('Rustier binary was not built')
      const asset = bundle[this.getFileName(binaryReference)]
      if (!asset || asset.type !== 'asset') this.error('Rustier binary asset is missing')
      asset.source = binary
    },
  }
}
