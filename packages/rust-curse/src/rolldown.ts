import { readFileSync } from 'node:fs'
import path, { resolve } from 'node:path'
import { platform as profiles } from 'node:process'
import type { Plugin } from 'rolldown'
import { Server, ServerPlatform, ServerWasmTarget } from 'rust-curse-native'

export type RustCurseOptions = {
  /**
   * Path to the Cargo manifest, relative to Rolldown's working directory.
   *
   * @default "Cargo.toml"
   */
  manifestPath?: string
  /**
   * Rust build profiles available to imported modules.
   */
  profiles: Record<string, RustCurseProfile>
  /**
   * Select the build profile for an imported `.rs` module.
   */
  activeProfile: (fileId: string) => string
}

export interface RustCurseProfile {
  target: RustCurseTarget
  /**
   * Whether release build is used.
   */
  release?: boolean
}

export type RustCurseTarget = 'node' | 'wasm' | 'wasm-node'


/**
 * Compile Rust modules and make `.rs` files importable by Rolldown.
 *
 * @example
 * ```ts
 * import { defineConfig } from 'rolldown'
 * import rustCurse from 'rust-curse/rolldown'
 *
 * export default defineConfig({
 *   platform: 'node',
 *   plugins: [
 *     rustCurse({
 *       profiles: {
 *         node: { target: 'node' },
 *         wasm: { target: 'wasm' },
 *       },
 *       activeProfile: (fileId) =>
 *         fileId.includes('.server') ? 'node' : 'wasm',
 *     }),
 *   ],
 * })
 * ```
 */
export default function rustCurse(options: RustCurseOptions): Plugin {
  const profileNames = Object.keys(options.profiles)

  if (profileNames.length === 0) {
    throw new Error('rust-curse requires at least one build profile')
  }

  if (typeof options.activeProfile !== 'function') {
    throw new Error('rust-curse requires activeProfile')
  }

  let server = new Server({
    rootDir: process.cwd(),
    manifestPath: path.resolve(options.manifestPath ?? "./Cargo.toml")
  })

  return {
    name: 'rust-curse',
    watchChange(filePath, change) {
      const contents =
        change.event === 'delete' ? undefined : readFileSync(filePath, 'utf8')

      server.change({ filePath, contents })
    },

    buildStart() {
      for (let profile of Object.values(options.profiles)) {
        server.build(toServerPlatforn(profile.target), profile.release ?? false)
      }
    },
  }
}


function toServerPlatforn(target: RustCurseTarget): ServerPlatform {
  switch (target) {
    case 'node':
      return { type: 'Node' }
    case 'wasm-node':
      return { type: 'Wasm', target: ServerWasmTarget.Node }
    case 'wasm':
      return { type: 'Wasm', target: ServerWasmTarget.Web }
  }
}

// const profileNames = Object.keys(options.profiles)
//   if (profileNames.length === 0) {
//     throw new Error('rust-curse requires at least one build profile')
//   }
//   if (typeof options.activeProfile !== 'function') {
//     throw new Error('rust-curse requires activeProfile')
//   }

//   let root: string
//   let manifestPath: string
//   let outDir: string
//   let server: Server
//   let initialized = false

//   const initialize = (cwd: string) => {
//     root = cwd
//     manifestPath = resolve(root, options.manifestPath ?? 'Cargo.toml')
//     outDir = resolve(root, options.outDir ?? '.rust-curse')
//     server = new Server({
//       rootDir: root,
//       manifestPath
//     })

//     if (options.installDeps !== false) server.installDeps()
//     initialized = true
//   }


//   const build = (warn: (diagnostic: string) => void) => {
//     try {
//       const output = server.build(profileNames)
//       for (const profileName of profileNames) {
//         const profile = output.perProfile[profileName]
//         if (!profile) {
//           throw new Error(`profile \`${profileName}\` has no build output`)
//         }
//         for (const diagnostic of profile.diags) warn(diagnostic)
//       }
//     } catch (error) {
//       warn(errorMessage(error))
//     }
//   }

//   const rustInputPath = (rawFilePath: string) => {
//     const filePath = resolve(root, cleanModuleId(rawFilePath))
//     if (isPathInside(outDir, filePath)) return
//     if (!filePath.endsWith('.rs') && filePath !== manifestPath) return
//     return filePath
//   }

//   return {
//     name: 'rust-curse',

//     buildStart(inputOptions) {
//       if (!initialized) {
//         initialize(inputOptions.cwd ?? process.cwd())
//       }
//       build((diagnostic) => this.warn(diagnostic))
//     },

//     watchChange(id, change) {
//       const filePath = rustInputPath(id)
//       if (!filePath) return

//       const contents =
//         change.event === 'delete' ? undefined : readFileSync(filePath, 'utf8')
//       server.update([{ filePath, contents }])
//     },

//     load(id) {
//       const filePath = cleanModuleId(id)
//       if (!filePath.endsWith('.rs')) return null

//       try {
//         const profileName = options.activeProfile(filePath)
//         if (!options.profiles[profileName]) {
//           throw new Error(
//             `activeProfile returned unknown profile \`${profileName}\` for \`${filePath}\``,
//           )
//         }
//         return server.load(profileName, filePath)
//       } catch (error) {
//         const diagnostic = errorMessage(error)
//         this.warn(diagnostic)
//         return `throw new Error(${JSON.stringify(diagnostic)});`
//       }
//     },
//   }
