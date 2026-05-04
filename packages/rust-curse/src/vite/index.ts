import { resolve } from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'
import { defaultOptions, RustCurseOptions } from './options'
import { BuildTarget, RustCurse } from 'rust-curse-native'
import type { SourceOptions } from 'rust-curse-native'
import { deepMerge, DeepRequired, normalizeSources } from '../utils'

export default function rustCurse(options?: RustCurseOptions): Plugin {
  const _options = deepMerge(
    defaultOptions,
    options ?? {},
  ) as DeepRequired<RustCurseOptions>

  return rustCursePlugin(_options)
}

function rustCursePlugin(options: DeepRequired<RustCurseOptions>): Plugin {
  let config: ResolvedConfig

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

    buildStart() {
      const clientSource = normalizeSources(options.sources)

      const targets: BuildTarget[] = [BuildTarget.Wasm]
      let jsSource: SourceOptions = clientSource

      if (config.command === 'build') {
        targets.push(BuildTarget.Node)
        const ssrSource = normalizeSources(options.ssr.sources)
        jsSource = {
          include: [...clientSource.include, ...ssrSource.include],
          exclude: [...clientSource.exclude, ...ssrSource.exclude],
        }
      }

      const rustCurse = new RustCurse({
        targets,
        jsSource,
        moduleDirRelPath: resolve(config.root, options.rust.moduleDir),
        cargoTomlRelPath: resolve(config.root, options.rust.manifestPath),
        outputDirRelPath: resolve(config.root, options.tempDir),
        projectRootAbsPath: config.root,
      })

      rustCurse.build()
    },
  }
}
