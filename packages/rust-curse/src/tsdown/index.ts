import { readFileSync } from 'node:fs'
import { basename, join, resolve as resolvePath } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { ProfileOpts, RustCurse, WasmProfileArtifact } from 'rust-curse-native'
import { deepMerge, DeepRequired, normalizeFilter } from '../utils'
import { buildNapi } from '../napi'

type Plugin = Rolldown.Plugin

export default function rustCurse(options?: RustCurseOptions): Plugin {
  const _options = deepMerge(
    defaultOptions,
    options ?? {},
  ) as DeepRequired<RustCurseOptions>

  return _rustCurse(_options)
}

function _rustCurse(options: DeepRequired<RustCurseOptions>): Plugin {
  let curse: RustCurse | null = null

  return {
    name: 'rust-curse',

    async buildStart() {
      const profile: ProfileOpts = {
        name: options.platform,
        rust: {
          target: {
            type: options.platform,
          },
        },
      }

      curse = new RustCurse({
        projectRoot: resolvePath(options.rootDir),
        outDir: options.outDir,
        rust: { modulesRootDir: options.rust.moduleDir },
        js: {
          filter: normalizeFilter(options.js.filter),
          typescript: options.js.typescript ?? true,
        },
        profiles: [profile],
      })

      const artifact = curse.build([profile.name]).artifacts[profile.name]

      if (profile.name === 'wasm') {
        // wasm-bindgen is called from rust
        this.emitFile({
          type: 'asset',
          fileName: basename(artifact.wasmAbsPath),
          source: readFileSync(artifact.wasmAbsPath),
        })
      } else {
        // napi-cli is called by js
        const output = await buildNapi({
          outputDir: join(options.rootDir, options.outDir, 'output/node'),
          manifestPath: join(options.rootDir, 'Cargo.toml'),
          release: false,
        })
        for (const artifact of output) {
          if (artifact.kind === 'node') {
            this.emitFile({
              type: 'asset',
              fileName: basename(artifact.path),
              source: readFileSync(artifact.path),
            })
          }
        }
      }
    },

    load(id) {
      if (!id.endsWith('.rs')) return null
      if (!curse) return null
      return curse.resolve(options.platform, id)
    },
  }
}
