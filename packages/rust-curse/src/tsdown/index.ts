import { resolve, join } from 'node:path'
import { mkdirSync } from 'node:fs'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { Session, BuildTarget } from 'rust-curse-native'
import { deepMerge, DeepRequired, normalizeSources } from '../utils'

type Plugin = Rolldown.Plugin

export default function rustCurse(options?: RustCurseOptions): Plugin {
  const _options = deepMerge(
    defaultOptions,
    options ?? {},
  ) as DeepRequired<RustCurseOptions>

  return rustCursePlugin(_options)
}

function rustCursePlugin(options: DeepRequired<RustCurseOptions>): Plugin {
  return {
    name: 'rust-curse',

    buildStart() {
      const jsSource = normalizeSources(options.sources)

      const session = new Session({
        jsSource,
        moduleDir: resolve(options.rootDir, options.rust.moduleDir),
        target: BuildTarget.Prelude,
        cargoPath: resolve(options.rootDir, options.rust.manifestPath),
        outputDir: resolve(options.rootDir, join(options.tempDir, 'prelude')),
        projectRoot: options.rootDir,
      })

      session.build()

      // TODO: platform build (napi/wasm) once later pipeline steps are implemented
    },
  }
}
