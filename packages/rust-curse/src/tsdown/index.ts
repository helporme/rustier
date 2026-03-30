import { readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { type Rolldown } from 'tsdown'
import { RustCurseOptions } from './options'
import { build, BuildTarget } from 'rust-curse-native'
import { buildNapi } from '../napi'

const VIRTUAL_PREFIX = '\0rust-curse:'

export default function rustCurse(
  options: RustCurseOptions = {},
): Rolldown.Plugin {
  const root = process.cwd()
  // Maps relative source key (e.g. ./src/routes/greeting.rs) to absolute glue file path
  let moduleMap: Map<string, string> = new Map()
  // Maps absolute filesystem path to the relative source key
  let absToKey: Map<string, string> = new Map()

  return {
    name: 'rust-curse',

    async buildStart() {
      const ssrDir = resolve(root, '.rust-curse/napi')
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
        projectRoot: root,
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
        outputDir: '.rust-curse/napi/',
        projectRoot: root,
      })

      const release = !this.meta.watchMode

      await buildNapi({
        release: release,
        manifestPath: '.rust-curse/napi/Cargo.toml',
        outputDir: '.rust-curse/napi/output',
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
        const absPath = resolve(root, sourceKey)
        absToKey.set(absPath, sourceKey)
      }
    },

    resolveId(source, importer) {
      if (!importer) return null

      // Only consider .rs imports or extensionless relative imports
      if (!source.endsWith('.rs') && !source.startsWith('.')) return null

      const resolved = resolve(dirname(importer), source)

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
