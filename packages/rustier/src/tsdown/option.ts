import { resolve } from 'node:path'
import { nativePlatforms, resolveTarget, type MergedTargetOptions, type ResolvedTarget } from '../targets'
import { resolveCrates, type WorkspaceOptions } from '../workspace'

/** Local crate paths are relative to `rootDir`; registry dependencies take a Cargo version requirement. */
export type CrateOptions = ({ path: string } | { version: string }) & {
  /** Crate-local feature names. Default features remain enabled. */
  features?: string[]
}

export type NativeTargetName = keyof typeof nativePlatforms
/** Only wasm32/wasm64-unknown-unknown are currently implemented. */
export type WasmTargetName = 'wasm32-unknown-unknown' | 'wasm64-unknown-unknown' | (`wasm${number}-${string}` & {})
export type TargetName = 'host' | NativeTargetName | WasmTargetName | (string & {})

/** Fetch Wasm by URL, read it from disk in Node, or import a bundler-provided WebAssembly.Module. */
export type Loading = 'web' | 'node-js' | 'module'
/** `auto` uses Zig for foreign Linux targets and xwin for non-Windows hosts targeting MSVC. */
export type BuildDriver = 'cargo' | 'auto' | 'zigbuild' | 'xwin'

export type TargetOptions = {
  /** Override the plugin's Cargo build driver for this target. */
  buildDriver?: BuildDriver
  /** Additional Cargo features, e.g. `['my_crate/simd']`. */
  features?: string[]
  /** Override variables in the Cargo process environment. */
  env?: Record<string, string>
} & ({
  target: TargetName
  loading?: never
} | {
  target: WasmTargetName
  /** Wasm loading strategy for single-target inline output. Defaults to `'web'`. */
  loading?: Loading
})

/** A Rust target triple or `'host'`, optionally accompanied by target-specific settings. */
export type Target = TargetName | TargetOptions

/**
 * Where runtime artifacts are distributed.
 * `'inline'` includes them in tsdown's output; `'npm'` generates target packages
 * and a router package, and patches the root manifest by default.
 * @example 'inline'
 * @example { type: 'npm', packageName: '@acme/library-bindings', outDir: 'npm' }
 */
export type Distribution = 'inline' | 'npm' | {
  /**
   * Generate standalone npm packages rather than bundling all artifacts into the root package.
   * @example 'npm'
   */
  type: 'npm'
  /**
   * Pin the generated router in root dependencies and add prepublishOnly if absent.
   * Set to false for setup warnings without editing package.json.
   * @default true
   */
  patchPackageJson?: boolean
  /**
   * Base name for generated packages, including an optional npm scope.
   * Target suffixes and `-unknown-unknown-unknown` for the router are appended.
   * Defaults to the root package name followed by `-bindings`.
   * @example '@acme/library-bindings'
   */
  packageName?: string
  /**
   * Generated package directory, relative to `rootDir`.
   * Must remain inside the project and separate from the root package's published files.
   * @default 'npm'
   * @example 'generated/native-packages'
   */
  outDir?: string
}

/**
 * Overrides used in watch mode. Omitted fields use the development defaults.
 * @example { target: 'wasm32-unknown-unknown', release: false }
 */
export interface DevOptions {
  /**
   * The one target to build. A configured target inherits its settings.
   * `'host'` can be used even when the release target list does not include it.
   * @default 'host'
   * @example { target: 'wasm32-unknown-unknown', loading: 'module' }
   */
  target?: Target
  /**
   * Development artifact layout.
   * @default 'inline'
   * @example 'inline'
   */
  distribution?: Distribution
  /**
   * Build optimized artifacts during development.
   * @default false
   * @example true
   */
  release?: boolean
}

/**
 * Configuration for the Rustier tsdown plugin.
 * @example { targets: 'recommended', distribution: 'npm' }
 */
export interface RustierOptions {
  /**
   * Release target list.
   *
   * `'recommended'` includes:
   * - `aarch64-apple-darwin`,
   * - `x86_64-apple-darwin`
   * - `x86_64-unknown-linux-gnu`,
   * - `aarch64-unknown-linux-gnu`
   * - `x86_64-unknown-linux-musl`,
   * - `aarch64-unknown-linux-musl`
   * - `x86_64-pc-windows-msvc`,
   * - `aarch64-pc-windows-msvc`
   *
   * To build Wasm, include its target explicitly.
   *
   * @default ['host']
   * @example 'recommended'
   * @example
   * [
   *   'aarch64-apple-darwin',
   *   'x86_64-unknown-linux-gnu',
   *   { target: 'wasm32-unknown-unknown', loading: 'module' },
   * ]
   */
  targets?: Target[] | 'recommended'
  /**
   * Command used to compile generated glue.
   * @default 'auto'
   * @example 'cargo'
   */
  buildDriver?: BuildDriver
  /**
   * Artifact layout for regular builds.
   * @default 'inline'
   * @example { type: 'npm', packageName: '@acme/library-bindings' }
   */
  distribution?: Distribution
  /**
   * Watch-mode (`tsdown watch`) overrides.
   *
   * Defaults to the host target, inline distribution, and debug builds.
   * Set `false` to use the regular-build configuration unchanged.
   *
   * @example { target: 'wasm32-unknown-unknown', distribution: 'inline' }
   * @example false
   */
  dev?: false | DevOptions
  /**
   * Project root.
   * @default process.cwd()
   * @example './packages/my-library'
   */
  rootDir?: string
  /**
   * Rust dependencies keyed by crate name:
   *
   * @example { my_crate: { path: './rust', features: ['logging'] }, serde: { version: '1', features: ['derive'] } }
   */
  crates?: Record<string, CrateOptions>
  /**
   * Maximum simultaneous target builds. Cargo may also parallelize within each build.
   * Must be a positive integer.
   * @default 2
   * @example 4
   */
  concurrency?: number
  /**
   * Build for release?
   *
   * Development still defaults to debug unless `dev.release` overrides it.
   * @default true
   * @example false
   */
  release?: boolean
}

export interface EffectiveOptions extends WorkspaceOptions {
  targets: ResolvedTarget[]
  distribution: Distribution
  concurrency: number
  development: boolean
}

export function normalizeOptions(options: RustierOptions, watch: boolean, host: string): EffectiveOptions {
  if (options.release !== undefined && typeof options.release !== 'boolean') {
    throw new Error('release must be a boolean')
  }
  const requested = options.targets === 'recommended' ? Object.keys(nativePlatforms) : options.targets ?? ['host']
  if (!requested.length) throw new Error('Rustier requires at least one target')
  const root = resolve(options.rootDir ?? process.cwd())
  const development = watch && options.dev !== false
  const dev = development ? options.dev || {} : undefined
  const configured: TargetOptions[] = requested.map(target => typeof target === 'string' ? { target } : target)
  const names = configured.map(target => target.target)
  if (new Set(names).size !== names.length) throw new Error('Duplicate Rustier targets')

  let selected: MergedTargetOptions[] = configured
  if (dev) {
    const override: TargetOptions =
      typeof dev.target === 'string' ? { target: dev.target } : dev.target ?? { target: 'host' }
    const name = override.target === 'host' ? host : override.target
    const base = configured.find(target => (target.target === 'host' ? host : target.target) === name)
    if (!base && override.target !== 'host') throw new Error(`Dev target ${name} is not in targets`)
    selected = [{
      ...base, ...override, target: name,
      features: [...(base?.features ?? []), ...(override.features ?? [])],
      env: { ...base?.env, ...override.env },
    }]
  }

  const release = dev ? dev.release ?? false : options.release ?? true
  const targets = selected.map(entry => resolveTarget(entry, host, release, options.buildDriver))
  if (new Set(targets.map(target => target.target)).size !== targets.length) {
    throw new Error('Targets resolve to the same Rust target')
  }
  const concurrency = options.concurrency ?? 2
  if (!Number.isSafeInteger(concurrency) || concurrency < 1 || concurrency > 0xffffffff) {
    throw new Error('concurrency must be a positive u32 integer')
  }
  const distribution = (dev ? dev.distribution : options.distribution) ?? 'inline'
  if (distribution !== 'inline' && distribution !== 'npm' && distribution.type !== 'npm') {
    throw new Error('Invalid Rustier distribution')
  }
  return {
    root, development, distribution, concurrency, targets,
    crates: resolveCrates(root, options.crates),
  }
}
