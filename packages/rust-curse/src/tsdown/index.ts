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

      curse.build([profile.name])
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
      if (!curse) return null
      return curse.resolve(options.platform, id)
    },
  }
}
