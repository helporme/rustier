import { readFileSync } from 'node:fs'
import { isAbsolute, relative, resolve } from 'node:path'
import type { Plugin } from 'rolldown'
import { Server, WasmTarget, type Target } from 'rust-curse-native'

export type RustCurseTarget = 'node' | 'wasm' | 'wasm-node'

export interface RustCurseProfile {
  target: RustCurseTarget
  release?: boolean
}

export interface RustCurseOptions {
  /**
   * Rust build profiles available to imported modules.
   */
  profiles: Record<string, RustCurseProfile>
  /**
   * Select the build profile for an imported `.rs` module.
   */
  activeProfile: (fileId: string) => string
  /**
   * Path to the Cargo manifest, relative to Rolldown's working directory.
   *
   * @default "Cargo.toml"
   */
  manifestPath?: string
  /**
   * Directory for generated crates, bindings, and build artifacts, relative
   * to Rolldown's working directory.
   *
   * @default ".rust-curse"
   */
  outDir?: string
  /**
   * Workspace file patterns passed to rust-curse.
   *
   * @default ["*.rs"]
   */
  included?: string[]
  /**
   * Workspace paths excluded from rust-curse.
   *
   * @default [".git", ".hg", ".svn", "node_modules", "target"]
   */
  excluded?: string[]
  /**
   * Dependency crates that should also receive generated bindings.
   */
  includeDepCrates?: string[]
  /**
   * Dependency crates that should not receive generated bindings.
   */
  excludeDepCrates?: string[]
  /**
   * Ensure the profiles' Rust build tools are installed before the first build.
   *
   * @default true
   */
  installDeps?: boolean
}

const toNativeTarget = (target: RustCurseTarget): Target => {
  switch (target) {
    case 'node':
      return { type: 'Node' }
    case 'wasm-node':
      return { type: 'Wasm', target: WasmTarget.NodeJs }
    case 'wasm':
      return { type: 'Wasm', target: WasmTarget.Web }
  }
}

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error)

const cleanModuleId = (id: string) => id.replace(/[?#].*$/, '')

const isPathInside = (parent: string, child: string) => {
  const childFromParent = relative(parent, child)
  return (
    childFromParent === '' ||
    (!childFromParent.startsWith('..') && !isAbsolute(childFromParent))
  )
}

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

  let root: string
  let manifestPath: string
  let outDir: string
  let server: Server
  let initialized = false

  const initialize = (cwd: string) => {
    root = cwd
    manifestPath = resolve(root, options.manifestPath ?? 'Cargo.toml')
    outDir = resolve(root, options.outDir ?? '.rust-curse')
    server = new Server({
      manifestPath,
      outDir,
      included: options.included ?? ['*.rs'],
      excluded: options.excluded ?? [
        '.git',
        '.hg',
        '.svn',
        'node_modules',
        'target',
      ],
      includeDepCrates: options.includeDepCrates ?? [],
      excludeDepCrates: options.excludeDepCrates ?? [],
      profiles: Object.fromEntries(
        Object.entries(options.profiles).map(([name, profile]) => [
          name,
          {
            target: toNativeTarget(profile.target),
            release: profile.release ?? false,
          },
        ]),
      ),
    })

    if (options.installDeps !== false) server.installDeps()
    initialized = true
  }

  const build = (warn: (diagnostic: string) => void) => {
    try {
      const output = server.build(profileNames)
      for (const profileName of profileNames) {
        const profile = output.perProfile[profileName]
        if (!profile) {
          throw new Error(`profile \`${profileName}\` has no build output`)
        }
        for (const diagnostic of profile.diags) warn(diagnostic)
      }
    } catch (error) {
      warn(errorMessage(error))
    }
  }

  const rustInputPath = (rawFilePath: string) => {
    const filePath = resolve(root, cleanModuleId(rawFilePath))
    if (isPathInside(outDir, filePath)) return
    if (!filePath.endsWith('.rs') && filePath !== manifestPath) return
    return filePath
  }

  return {
    name: 'rust-curse',

    buildStart(inputOptions) {
      if (!initialized) {
        initialize(inputOptions.cwd ?? process.cwd())
      }
      build((diagnostic) => this.warn(diagnostic))
    },

    watchChange(id, change) {
      const filePath = rustInputPath(id)
      if (!filePath) return

      const contents =
        change.event === 'delete' ? undefined : readFileSync(filePath, 'utf8')
      server.update([{ filePath, contents }])
    },

    load(id) {
      const filePath = cleanModuleId(id)
      if (!filePath.endsWith('.rs')) return null

      try {
        const profileName = options.activeProfile(filePath)
        if (!options.profiles[profileName]) {
          throw new Error(
            `activeProfile returned unknown profile \`${profileName}\` for \`${filePath}\``,
          )
        }
        return server.load(profileName, filePath)
      } catch (error) {
        const diagnostic = errorMessage(error)
        this.warn(diagnostic)
        return `throw new Error(${JSON.stringify(diagnostic)});`
      }
    },
  }
}
