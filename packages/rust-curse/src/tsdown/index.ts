import { resolve } from 'node:path'
import { type Rolldown } from 'tsdown'
import { defaultOptions, RustCurseOptions } from './options'
import { RustCurse } from 'rust-curse-native'
import type { BuildTarget } from 'rust-curse-native'
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
        { type: 'Default', jsSource },
        options.platform === 'node'
          ? { type: 'Node', jsSource }
          : { type: 'Wasm', jsSource },
      ]

      const rustCurse = new RustCurse({
        targets,
        moduleDirRelPath: resolve(options.rootDir, options.rust.moduleDir),
        cargoTomlRelPath: resolve(options.rootDir, options.rust.manifestPath),
        outputDirRelPath: resolve(options.rootDir, options.tempDir),
        projectRootAbsPath: options.rootDir,
      })

      const result = rustCurse.build()
      for (const warning of result.warnings) {
        console.warn(`[rust-curse] ${warning}`)
      }
    },
  }
}
