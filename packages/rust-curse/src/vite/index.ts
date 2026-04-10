import { resolve, join } from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'
import { defaultOptions, RustCurseOptions } from './options'
import { Session, BuildTarget } from 'rust-curse-native'
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
      const preludeDir = resolve(
        config.root,
        join(options.tempDir, 'prelude'),
      )
      const jsSourceClient = normalizeSources(options.sources)
      const jsSourceServer = normalizeSources(options.ssr.sources)

      const session = new Session({
        jsSource: {
          include: [...jsSourceClient.include, ...jsSourceServer.include],
          exclude: [],
        },
        moduleDirRelPath: resolve(config.root, options.rust.moduleDir),
        target: BuildTarget.Prelude,
        cargoTomlRelPath: resolve(config.root, options.rust.manifestPath),
        outputDirRelPath: preludeDir,
        projectRootAbsPath: config.root,
      })

      session.build()

      // TODO: server build (napi) + client build (wasm) once later pipeline steps are implemented
    },
  }
}
