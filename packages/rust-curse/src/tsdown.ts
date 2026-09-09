import rolldown, { type RustCurseTarget } from './rolldown'

export type { RustCurseTarget } from './rolldown'

export interface RustCurseOptions {
  /**
   * The runtime Rust should be compiled for.
   */
  target: RustCurseTarget
  /**
   * Compile an optimized Cargo release build.
   *
   * @default false
   */
  release?: boolean
  /**
   * Path to the Cargo manifest, relative to tsdown's working directory.
   *
   * @default "Cargo.toml"
   */
  manifestPath?: string
  /**
   * Directory for generated crates, bindings, and build artifacts.
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
   * Ensure the target's Rust build tools are installed before the first build.
   *
   * @default true
   */
  installDeps?: boolean
}

/**
 * Compile Rust modules and make `.rs` files importable by tsdown.
 *
 * @example
 * ```ts
 * import { defineConfig } from 'tsdown'
 * import rustCurse from 'rust-curse/tsdown'
 *
 * export default defineConfig({
 *   plugins: [rustCurse({ target: 'node' })],
 * })
 * ```
 */
export default function rustCurse({
  target,
  release,
  ...options
}: RustCurseOptions) {
  return rolldown({
    ...options,
    profiles: {
      default: { target, release },
    },
    activeProfile: () => 'default',
  })
}
