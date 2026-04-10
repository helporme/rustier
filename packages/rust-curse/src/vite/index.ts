import { resolve } from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'
import { defaultOptions, RustCurseOptions } from './options'
import { Session, BuildTarget } from 'rust-curse-native'
import type { TargetConfig } from 'rust-curse-native'
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
      const targets: TargetConfig[] = [
        {
          target: BuildTarget.Wasm,
          jsSource: normalizeSources(options.sources),
        },
      ]

      if (config.command === 'build') {
        targets.push({
          target: BuildTarget.Napi,
          jsSource: normalizeSources(options.ssr.sources),
        })
      }

      const session = new Session({
        targets,
        moduleDirRelPath: resolve(config.root, options.rust.moduleDir),
        cargoTomlRelPath: resolve(config.root, options.rust.manifestPath),
        outputDirRelPath: resolve(config.root, options.tempDir),
        projectRootAbsPath: config.root,
      })

      const result = session.build()
      for (const warning of result.warnings) {
        config.logger.warn(`[rust-curse] ${warning}`)
      }
    },
  }
}
