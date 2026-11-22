import { resolve } from 'node:path'
import { nativePlatforms, recommendedTargets, type BuildDriver, type NativeTargetName, type Target, type TargetOptions, type ResolvedTarget } from './targets'

export type { BuildDriver, Target, TargetName, NativeTargetName, WasmTargetName, TargetOptions, Loading } from './targets'

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
 * A Cargo dependency from a local directory or crates.io.
 * @example { version: '1', features: ['derive'] }
 * @example { path: './rust', features: ['logging'] }
 */
export type CrateOptions = ({
  /** Local crate directory, relative to `rootDir` or absolute. */
  path: string
} | {
  /** Cargo version requirement for a crates.io dependency. */
  version: string
}) & {
  /**
   * Features enabled for this dependency on every target. Default features remain enabled.
   * Names are local to the crate, without a `crate/` prefix.
   * @example ['derive']
   */
  features?: string[]
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

export interface EffectiveOptions {
  root: string
  crates: NonNullable<RustierOptions['crates']>
  targets: ResolvedTarget[]
  distribution: Distribution
  concurrency: number
  development: boolean
}

export function normalizeOptions(options: RustierOptions, watch: boolean, host: string): EffectiveOptions {
  if (options.release !== undefined && typeof options.release !== 'boolean') {
    throw new Error('release must be a boolean')
  }
  const requested = options.targets === 'recommended' ? recommendedTargets : options.targets ?? ['host']
  if (!requested.length) throw new Error('Rustier requires at least one target')
  const root = resolve(options.rootDir ?? process.cwd())
  const development = watch && options.dev !== false
  const dev = development ? options.dev || {} : undefined
  const configured: (Omit<TargetOptions, 'target'> & { target: string })[] =
    requested.map(target => typeof target === 'string' ? { target } : target)
  const names = configured.map(target => target.target)
  if (new Set(names).size !== names.length) throw new Error('Duplicate Rustier targets')

  let selected = configured
  if (dev) {
    const override: Omit<TargetOptions, 'target'> & { target: string } =
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

  const targets = selected.map((entry): ResolvedTarget => {
    const target = entry.target === 'host' ? host : entry.target
    const platform = Object.hasOwn(nativePlatforms, target) ? nativePlatforms[target as NativeTargetName] : undefined
    const kind = target === 'wasm32-unknown-unknown' ? 'wasm32'
      : target === 'wasm64-unknown-unknown' ? 'wasm64' : 'node'
    if (kind === 'node' && !platform) throw new Error(`Unsupported Rustier target: ${target}`)
    if (kind === 'node' && entry.loading) throw new Error(`loading applies only to Wasm: ${target}`)
    const loading = entry.loading ?? 'web'
    if (!['web', 'node-js', 'module'].includes(loading)) throw new Error(`Invalid Wasm loading mode: ${loading}`)
    return {
      target, kind, platform, loading,
      buildDriver: entry.buildDriver ?? options.buildDriver ?? 'auto',
      features: [...new Set(entry.features ?? [])].sort(),
      env: { ...entry.env },
      release: dev ? dev.release ?? false : options.release ?? true,
    }
  })
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
    crates: Object.fromEntries(Object.entries(options.crates ?? {}).map(([name, crate]) => [
      name, 'path' in crate ? { ...crate, path: resolve(root, crate.path) } : crate,
    ])),
  }
}
