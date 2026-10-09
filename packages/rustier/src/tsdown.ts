import rolldown, {
  type RustierOptions as RolldownOptions,
  type RustierTarget,
} from './rolldown'

export type { RustierTarget } from './rolldown'

export interface RustierOptions {
  /**
   * The runtime Rust should be compiled for.
   */
  target: RustierTarget
  /**
   * Compile an optimized Cargo release build.
   *
   * @default false
   */
  release?: boolean
  /** Additional Cargo features enabled on the source crate. */
  features?: string[]
  /**
   * Path to the Cargo manifest, relative to tsdown's working directory.
   *
   * @default "Cargo.toml"
   */
  manifestPath?: string
  /** Generate TypeScript declarations for the Rust API. Defaults to true. */
  ts?: boolean
  /**
   * Progress logging settings. Enabled by default, with pretty formatting
   * and debug/trace messages hidden. Does not suppress compiler diagnostics.
   */
  logging?: RolldownOptions['logging']
  /**
   * Directory for generated crates, bindings, and build artifacts.
   *
   * @default ".rustier"
   */
  outDir?: string
  /**
   * Workspace file patterns passed to rustier.
   *
   * @default ["*.rs"]
   */
  included?: string[]
  /**
   * Workspace paths excluded from rustier.
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
 * import rustier from 'rustier/tsdown'
 *
 * export default defineConfig({
 *   plugins: [rustier({ target: 'node' })],
 * })
 * ```
 */
export default function rustier({
  target,
  release,
  features,
  ...options
}: RustierOptions) {
  return rolldown({
    ...options,
    profiles: {
      default: { target, release, features },
    },
    activeProfile: () => 'default',
  })
}
