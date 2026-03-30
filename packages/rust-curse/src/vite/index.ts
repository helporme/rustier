import { readFileSync } from 'node:fs'
import { resolve, relative, dirname } from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'
import { RustCurseOptions } from './options'
import { build, BuildTarget } from 'rust-curse-native'
import { buildNapi } from '../napi'
import { buildWasm } from '../wasm'

const VIRTUAL_PREFIX = '\0rust-curse:'

export default function rustCurse(options: RustCurseOptions = {}): Plugin {
  let config: ResolvedConfig
  // Maps relative source key to absolute glue file path, per target
  let ssrModuleMap: Map<string, string> = new Map()
  let wasmModuleMap: Map<string, string> = new Map()
  // Maps absolute filesystem path to the relative source key (shared)
  let absToKey: Map<string, string> = new Map()

  return {
    name: 'rust-curse',
    enforce: 'pre',

    config() {
      return {
        server: {
          fs: {
            allow: ['.rust-curse'],
          },
        },
      }
    },

    configResolved(resolved) {
      config = resolved
    },

    async buildStart() {
      const ssrDir = resolve(config.root, '.rust-curse/ssr')
      const ssrModulesDir = resolve(ssrDir, 'modules')
      const wasmDir = resolve(config.root, '.rust-curse/wasm')
      const wasmModulesDir = resolve(wasmDir, 'modules')

      const jsSource = {
        include: ['src/**/*.ts', 'src/**/*.svelte'],
        exclude: [],
      }

      // dev build
      build({
        jsSource,
        moduleDir: 'src/',
        target: BuildTarget.Dev,
        cargoPath: 'Cargo.toml',
        outputDir: '.rust-curse/dev/',
        projectRoot: config.root,
      })

      // napi build (SSR)
      build({
        jsSource,
        moduleDir: 'src/',
        target: BuildTarget.Napi,
        cargoPath: 'Cargo.toml',
        outputDir: '.rust-curse/ssr/',
        projectRoot: config.root,
      })

      await buildNapi({
        release: false,
        manifestPath: '.rust-curse/ssr/Cargo.toml',
        outputDir: '.rust-curse/ssr/output',
      })

      // wasm build (browser)
      build({
        jsSource,
        moduleDir: 'src/',
        target: BuildTarget.Wasm,
        cargoPath: 'Cargo.toml',
        outputDir: '.rust-curse/wasm/',
        projectRoot: config.root,
      })

      buildWasm({
        release: false,
        manifestPath: '.rust-curse/wasm/Cargo.toml',
        outputDir: '.rust-curse/wasm/output',
      })

      // Load SSR module map
      ssrModuleMap = new Map()
      absToKey = new Map()
      const ssrModulesJson: Record<string, string> = JSON.parse(
        readFileSync(resolve(ssrModulesDir, 'modules.json'), 'utf-8'),
      )
      for (const [sourceKey, glueFile] of Object.entries(ssrModulesJson)) {
        ssrModuleMap.set(sourceKey, resolve(ssrModulesDir, glueFile))
        absToKey.set(resolve(config.root, sourceKey), sourceKey)
      }

      // Load Wasm module map
      wasmModuleMap = new Map()
      const wasmModulesJson: Record<string, string> = JSON.parse(
        readFileSync(resolve(wasmModulesDir, 'modules.json'), 'utf-8'),
      )
      for (const [sourceKey, glueFile] of Object.entries(wasmModulesJson)) {
        wasmModuleMap.set(sourceKey, resolve(wasmModulesDir, glueFile))
        // absToKey is shared — keys are the same for both targets
      }
    },

    resolveId(source, importer) {
      if (!importer) return null

      // Only consider .rs imports or extensionless relative imports
      if (!source.endsWith('.rs') && !source.startsWith('.')) return null

      // Normalize importer to absolute path
      // If it starts with config.root, it's already absolute.
      // If it starts with '/' but not config.root, it's root-relative (e.g. /src/routes/+page.svelte).
      let absoluteImporter: string
      if (importer.startsWith(config.root)) {
        absoluteImporter = importer
      } else if (importer.startsWith('/')) {
        absoluteImporter = resolve(config.root, importer.slice(1))
      } else {
        absoluteImporter = importer
      }
      const resolved = resolve(dirname(absoluteImporter), source)

      // Try exact absolute path
      let key = absToKey.get(resolved)
      if (key) return VIRTUAL_PREFIX + key

      // Try with .rs appended
      key = absToKey.get(resolved + '.rs')
      if (key) return VIRTUAL_PREFIX + key

      return null
    },

    load(id) {
      if (!id.startsWith(VIRTUAL_PREFIX)) return null

      const sourceKey = id.slice(VIRTUAL_PREFIX.length)
      const isSSR = this.environment?.name === 'ssr'
      const map = isSSR ? ssrModuleMap : wasmModuleMap
      const gluePath = map.get(sourceKey)
      if (!gluePath) return null

      return readFileSync(gluePath, 'utf-8')
    },
  }
}
