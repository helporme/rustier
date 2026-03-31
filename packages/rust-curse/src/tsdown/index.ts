import { readFileSync, readdirSync } from 'node:fs'
import { resolve, dirname, join } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { build, BuildTarget } from 'rust-curse-native'
import { buildNapi } from '../napi'
import { buildWasm } from '../wasm'
import { deepMerge, DeepRequired, normalizeSources } from '../utils'

const VIRTUAL_PREFIX = '\0rust-curse:'
type Plugin = Rolldown.Plugin

export default function rustCurse(options?: RustCurseOptions): Plugin {
  const _options = deepMerge(
    defaultOptions,
    options ?? {},
  ) as DeepRequired<RustCurseOptions>

  return rustCursePlugin(_options)
}

function rustCursePlugin(options: DeepRequired<RustCurseOptions>): Plugin {
  // Maps relative source key (e.g. ./src/routes/greeting.rs) to absolute glue file path
  let moduleMap: Map<string, string> = new Map()
  // Maps absolute filesystem path to the relative source key
  let absToKey: Map<string, string> = new Map()
  // Absolute path to the platform output directory
  let platformOutputDir: string = ''

  return {
    name: 'rust-curse',

    async buildStart() {
      const platformDir = resolve(
        options.rootDir,
        join(options.tempDir, options.platform),
      )
      const platformModulesDir = resolve(platformDir, 'modules')
      const devDir = resolve(options.rootDir, join(options.tempDir, 'dev'))
      const jsSource = normalizeSources(options.sources)

      // dev build
      build({
        jsSource,
        moduleDir: options.rust.moduleDir,
        target: BuildTarget.Dev,
        cargoPath: options.rust.manifestPath,
        outputDir: devDir,
        projectRoot: options.rootDir,
      })

      // platform build
      build({
        jsSource,
        moduleDir: options.rust.moduleDir,
        target:
          options.platform === 'browser' ? BuildTarget.Wasm : BuildTarget.Napi,
        cargoPath: options.rust.manifestPath,
        outputDir: platformDir,
        projectRoot: options.rootDir,
      })

      const release = !this.meta.watchMode
      platformOutputDir = join(platformDir, 'output')

      if (options.platform === 'browser') {
        buildWasm({
          release: release,
          manifestPath: join(platformDir, 'Cargo.toml'),
          outputDir: platformOutputDir,
        })
      } else {
        await buildNapi({
          release: release,
          manifestPath: join(platformDir, 'Cargo.toml'),
          outputDir: platformOutputDir,
        })
      }

      // Load the module map
      const modulesJsonPath = resolve(platformModulesDir, 'modules.json')
      const modulesJson: Record<string, string> = JSON.parse(
        readFileSync(modulesJsonPath, 'utf-8'),
      )

      moduleMap = new Map()
      absToKey = new Map()
      for (const [sourceKey, glueFile] of Object.entries(modulesJson)) {
        const gluePath = resolve(platformModulesDir, glueFile)
        moduleMap.set(sourceKey, gluePath)

        // Also map absolute path to key for direct resolution
        const absPath = resolve(options.rootDir, sourceKey)
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

    generateBundle() {
      if (!platformOutputDir) return

      if (options.platform === 'browser') {
        // Emit .wasm files from the wasm-bindgen pkg output
        const pkgDir = resolve(platformOutputDir, 'pkg')
        for (const file of readdirSync(pkgDir)) {
          if (!file.endsWith('.wasm')) continue
          this.emitFile({
            type: 'asset',
            fileName: file,
            source: readFileSync(resolve(pkgDir, file)),
          })
        }
      } else {
        // Emit .node native binaries (the JS loader is inlined by the bundler)
        for (const file of readdirSync(platformOutputDir)) {
          if (!file.endsWith('.node')) continue
          this.emitFile({
            type: 'asset',
            fileName: file,
            source: readFileSync(resolve(platformOutputDir, file)),
          })
        }
      }
    },
  }
}
