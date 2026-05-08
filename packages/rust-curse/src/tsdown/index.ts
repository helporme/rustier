import { dirname, resolve as resolvePath } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { ProfileOpts, RustCurse } from 'rust-curse-native'
import {
  deepMerge,
  DeepRequired,
  normalizeSources as normalizeFilter,
} from '../utils'

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
  // The profile resolved against when a JS file imports a `.rs` module.
  // When the user configured multiple platforms we currently pick the first
  // one; refining this (e.g. routing per-output-condition) is future work.
  let resolveProfileName: string | null = null

  return {
    name: 'rust-curse',

    buildStart() {
      const platforms =
        typeof options.platform === 'string'
          ? [options.platform]
          : options.platform

      const profiles: ProfileOpts[] = platforms.map((platform) => ({
        name: platform,
        rust: {
          target: {
            type: platform,
          },
        },
      }))

      resolveProfileName = profiles[0]?.name ?? null

      curse = new RustCurse({
        projectRoot: resolvePath(options.rootDir),
        outDir: options.outDir,
        rust: { modulesRootDir: options.rust.moduleDir },
        js: {
          filter: normalizeFilter(options.js.filter),
          typescript: options.js.typescript ?? true,
        },
        profiles,
      })

      curse.build(profiles.map((p) => p.name))
    },

    resolveId(source, importer) {
      if (!source.endsWith('.rs')) return null
      if (importer && (source.startsWith('./') || source.startsWith('../'))) {
        return resolvePath(dirname(importer), source)
      }
      return null
    },

    load(id) {
      if (!id.endsWith('.rs')) return null
      if (!curse || !resolveProfileName) return null
      return curse.resolve(resolveProfileName, id)
    },
  }
}
