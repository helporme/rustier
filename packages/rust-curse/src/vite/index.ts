import { readFileSync, readdirSync } from 'node:fs'
import { resolve, join, dirname, relative, posix } from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'
import { defaultOptions, RustCurseOptions } from './options'
import { build, BuildTarget } from 'rust-curse-native'
import { buildNapi } from '../napi'
import { buildWasm } from '../wasm'
import { deepMerge, DeepRequired, normalizeSources } from '../utils'

const VIRTUAL_PREFIX = '\0rust-curse:'
const NAPI_EXTERNAL_ID = 'rust-curse:napi-loader'
const NAPI_OUTPUT_DIR = '_rust_curse'

export default function rustCurse(options?: RustCurseOptions): Plugin {
  const _options = deepMerge(
    defaultOptions,
    options ?? {},
  ) as DeepRequired<RustCurseOptions>

  return rustCursePlugin(_options)
}

function rustCursePlugin(options: DeepRequired<RustCurseOptions>): Plugin {
  let config: ResolvedConfig

  // Maps relative source key to absolute glue file path, per target
  let ssrModuleMap: Map<string, string> = new Map()
  let wasmModuleMap: Map<string, string> = new Map()

  // Maps absolute filesystem path to the relative source key (shared)
  let absToKey: Map<string, string> = new Map()

  // Absolute path to the NAPI index.js loader — marked external so Rollup
  // doesn't bundle it and native .node files resolve correctly at runtime
  let napiIndexPath: string = ''

  return {
    name: 'rust-curse',
    enforce: 'pre',

    config() {
      return {
        server: {
          fs: {
            allow: [options.tempDir],
          },
        },
      }
    },

    configResolved(resolved) {
      config = resolved
    },

    async buildStart() {
      const serverDir = resolve(config.root, join(options.tempDir, 'node'))
      const serverModulesDir = resolve(serverDir, 'modules')

      const clientDir = resolve(config.root, join(options.tempDir, 'browser'))
      const clientModulesDir = resolve(clientDir, 'modules')

      const devDir = resolve(config.root, join(options.tempDir, 'dev'))

      const jsSourceClient = normalizeSources(options.sources)
      const jsSourceServer = normalizeSources(options.ssr.sources)

      const isReleaseBuild = config.command === 'build'
      const isSSR = !!config.build.ssr

      // dev build
      build({
        jsSource: {
          // it's questionable
          include: [...jsSourceClient.include, ...jsSourceServer.include],
          exclude: [],
        },
        moduleDir: options.rust.moduleDir,
        target: BuildTarget.Dev,
        cargoPath: options.rust.manifestPath,
        outputDir: devDir,
        projectRoot: config.root,
      })

      if (!isReleaseBuild || isSSR) {
        // server build
        build({
          jsSource: jsSourceServer,
          moduleDir: options.rust.moduleDir,
          target: BuildTarget.Napi,
          cargoPath: options.rust.manifestPath,
          outputDir: serverDir,
          projectRoot: config.root,
        })

        napiIndexPath = resolve(serverDir, 'output', 'index.js')
        await buildNapi({
          release: isReleaseBuild,
          manifestPath: join(serverDir, 'Cargo.toml'),
          outputDir: join(serverDir, 'output'),
        })

        // Load server module map
        ssrModuleMap = new Map()
        absToKey = new Map()
        const ssrModulesJson: Record<string, string> = JSON.parse(
          readFileSync(resolve(serverModulesDir, 'modules.json'), 'utf-8'),
        )
        for (const [sourceKey, glueFile] of Object.entries(ssrModulesJson)) {
          ssrModuleMap.set(sourceKey, resolve(serverModulesDir, glueFile))
          absToKey.set(resolve(config.root, sourceKey), sourceKey)
        }
      }

      if (!isReleaseBuild || !isSSR) {
        // client build
        build({
          jsSource: jsSourceClient,
          moduleDir: options.rust.moduleDir,
          target: BuildTarget.Wasm,
          cargoPath: options.rust.manifestPath,
          outputDir: clientDir,
          projectRoot: config.root,
        })
        buildWasm({
          release: isReleaseBuild,
          manifestPath: join(clientDir, 'Cargo.toml'),
          outputDir: join(clientDir, 'output'),
        })

        // Load client module map
        wasmModuleMap = new Map()
        const wasmModulesJson: Record<string, string> = JSON.parse(
          readFileSync(resolve(clientModulesDir, 'modules.json'), 'utf-8'),
        )
        for (const [sourceKey, glueFile] of Object.entries(wasmModulesJson)) {
          wasmModuleMap.set(sourceKey, resolve(clientModulesDir, glueFile))
          absToKey.set(resolve(config.root, sourceKey), sourceKey)
        }
      }
    },

    resolveId(source, importer) {
      // Externalize the NAPI loader during builds — we emit it into _rust_curse/
      // in generateBundle and fix up the import path in renderChunk.
      // In dev mode, resolve to the actual file so Vite's dev server can serve it.
      if (napiIndexPath && source === napiIndexPath) {
        if (config.command === 'build') {
          return { id: NAPI_EXTERNAL_ID, external: true }
        }
        return napiIndexPath
      }

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

    renderChunk(code, chunk) {
      if (!napiIndexPath) return null
      if (!code.includes(NAPI_EXTERNAL_ID)) return null

      // Compute the relative path from this chunk to _rust_curse/index.js
      const chunkDir = dirname(chunk.fileName)
      let rel = posix.relative(chunkDir, NAPI_OUTPUT_DIR + '/index.js')
      if (!rel.startsWith('.')) rel = './' + rel

      return code.replaceAll(NAPI_EXTERNAL_ID, rel)
    },

    generateBundle() {
      if (!napiIndexPath) return
      const isSSR = !!config.build.ssr
      if (!isSSR) return

      const napiDir = dirname(napiIndexPath)

      // Emit the NAPI loader
      this.emitFile({
        type: 'asset',
        fileName: NAPI_OUTPUT_DIR + '/index.js',
        source: readFileSync(napiIndexPath, 'utf-8'),
      })

      // Emit all .node files alongside it
      for (const file of readdirSync(napiDir)) {
        if (!file.endsWith('.node')) continue
        this.emitFile({
          type: 'asset',
          fileName: NAPI_OUTPUT_DIR + '/' + file,
          source: readFileSync(resolve(napiDir, file)),
        })
      }
    },
  }
}
