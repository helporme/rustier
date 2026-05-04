import { resolve } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { BuildTarget, RustCurse } from 'rust-curse-native'
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

      const targets: BuildTarget[] = [
        BuildTarget.Ts,
        options.platform === 'node' ? BuildTarget.Node : BuildTarget.Wasm,
      ]

      const rustCurse = new RustCurse({
        targets,
        jsSource,
        moduleDirRelPath: resolve(options.rootDir, options.rust.moduleDir),
        cargoTomlRelPath: resolve(options.rootDir, options.rust.manifestPath),
        outputDirRelPath: resolve(options.rootDir, options.tempDir),
        projectRootAbsPath: options.rootDir,
      })

      rustCurse.build()
    },
  }
}
