import { resolve } from 'node:path'
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
  return {
    name: 'rust-curse',

    buildStart() {
      const profile: ProfileOpts = {
        name: options.platform,
        rust: {
          target:
            options.platform === 'node'
              ? { type: 'node-api' }
              : { type: 'wasm' },
        },
      }

      const rustCurse = new RustCurse({
        projectRoot: resolve(options.rootDir),
        outDir: options.outDir,
        rust: { modulesRootDir: options.rust.moduleDir },
        js: {
          filter: normalizeFilter(options.js.filter),
          typescript: options.js.typescript ?? true,
        },
        profiles: [profile],
      })

      rustCurse.build([profile.name])
    },
  }
}
