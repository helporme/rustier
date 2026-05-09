import { readFileSync } from 'node:fs'
import { basename, resolve as resolvePath } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { ProfileOpts, RustCurse, WasmProfileArtifact } from 'rust-curse-native'
import { deepMerge, DeepRequired, normalizeFilter } from '../utils'

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

    buildStart() {
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

      const output = curse.build([profile.name])

      for (const [profileName, artifact] of Object.entries(output.artifacts)) {
        if (profileName !== options.platform) continue

        this.emitFile({
          type: 'asset',
          fileName: basename(artifact.wasmAbsPath),
          source: readFileSync(artifact.wasmAbsPath),
        })
      }
    },

    load(id) {
      if (!id.endsWith('.rs')) return null
      if (!curse) return null
      return curse.resolve(options.platform, id)
    },
  }
}
