import { readFileSync } from 'node:fs'
import { basename, join, resolve as resolvePath } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import {
  BuildOutput,
  ProfileOpts,
  RustCurse,
  WasmProfileArtifact,
} from 'rust-curse-native'
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
  let build: BuildOutput | null = null

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

      const curse = new RustCurse(
        {
          projectRoot: resolvePath(options.rootDir),
          outDir: options.outDir,
          rust: { modulesRootDir: options.rust.moduleDir },
          js: {
            filter: normalizeFilter(options.js.filter),
            typescript: options.js.typescript ?? true,
          },
          profiles: [profile],
        },
        buildNapi,
      )

      build = await curse.build([profile.name])
      for (const warn of build.warns) {
        this.warn({
          code: warn.code,
          message: warn.rendered ?? warn.message,
        })
      }
      const artifact = build.artifacts[profile.name]

      if (artifact.type === 'wasm') {
        // wasm-bindgen is called from rust
        this.emitFile({
          type: 'asset',
          fileName: basename(artifact.wasmAbsPath),
          source: readFileSync(artifact.wasmAbsPath),
        })
      } else {
        for (const nodePath of artifact.nodePaths) {
          this.emitFile({
            type: 'asset',
            fileName: basename(nodePath),
            source: readFileSync(nodePath),
          })
        }
      }
    },

    load(id) {
      if (!id.endsWith('.rs')) return null
      if (!build) return null
      return build.resolve(options.platform, id)
    },
  }
}
