import { readFileSync } from 'node:fs'
import { resolve, relative, dirname } from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'
import { RustCurseOptions } from './options'
import { build, BuildTarget } from 'rust-curse-native'
import { buildNapi } from '../napi'

const VIRTUAL_PREFIX = '\0rust-curse:'

export default function rustCurse(options: RustCurseOptions = {}): Plugin {
  let config: ResolvedConfig
  // Maps relative source key (e.g. ./src/routes/greeting.rs) to absolute glue file path
  let moduleMap: Map<string, string> = new Map()
  // Maps absolute filesystem path to the relative source key
  let absToKey: Map<string, string> = new Map()

  return {
    name: 'rust-curse',
    enforce: 'pre',

    configResolved(resolved) {
      config = resolved
    },

    async buildStart() {
      const ssrDir = resolve(config.root, '.rust-curse/ssr')
      const modulesDir = resolve(ssrDir, 'modules')

      // dev build
      build({
        jsSource: {
          include: ['src/**/*.ts', 'src/**/*.svelte'],
          exclude: [],
        },
        moduleDir: 'src/',
        target: BuildTarget.Dev,
        cargoPath: 'Cargo.toml',
        outputDir: '.rust-curse/dev/',
        projectRoot: config.root,
      })

      // napi build
      build({
        jsSource: {
          include: ['src/**/*.ts', 'src/**/*.svelte'],
          exclude: [],
        },
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

      // Load the module map
      const modulesJsonPath = resolve(modulesDir, 'modules.json')
      const modulesJson: Record<string, string> = JSON.parse(
        readFileSync(modulesJsonPath, 'utf-8'),
      )

      moduleMap = new Map()
      absToKey = new Map()
      for (const [sourceKey, glueFile] of Object.entries(modulesJson)) {
        const gluePath = resolve(modulesDir, glueFile)
        moduleMap.set(sourceKey, gluePath)

        // Also map absolute path to key for direct resolution
        const absPath = resolve(config.root, sourceKey)
        absToKey.set(absPath, sourceKey)
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
      const gluePath = moduleMap.get(sourceKey)
      if (!gluePath) return null

      return readFileSync(gluePath, 'utf-8')
    },
  }
}
