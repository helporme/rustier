export interface NativePlatform {
  os: string
  cpu: string
  libc?: string
  suffix: string
}

/**
 * Native Node target triples with built-in platform-package mappings.
 * @example 'x86_64-unknown-linux-gnu'
 */
export type NativeTargetName =
  | 'aarch64-apple-darwin'
  | 'x86_64-apple-darwin'
  | 'x86_64-unknown-linux-gnu'
  | 'aarch64-unknown-linux-gnu'
  | 'x86_64-unknown-linux-musl'
  | 'aarch64-unknown-linux-musl'
  | 'x86_64-pc-windows-msvc'
  | 'aarch64-pc-windows-msvc'

/**
 * Wasm target names. Only wasm32/wasm64-unknown-unknown are currently implemented.
 * @example 'wasm32-unknown-unknown'
 */
export type WasmTargetName =
  | 'wasm32-unknown-unknown'
  | 'wasm64-unknown-unknown'
  | (`wasm${number}-${string}` & {})

/**
 * Known targets provide autocomplete; other strings remain assignable.
 * @example 'aarch64-apple-darwin'
 */
export type TargetName = 'host' | NativeTargetName | WasmTargetName | (string & {})

export const nativePlatforms: Record<NativeTargetName, NativePlatform> = {
  'aarch64-apple-darwin': { os: 'darwin', cpu: 'arm64', suffix: 'darwin-arm64' },
  'x86_64-apple-darwin': { os: 'darwin', cpu: 'x64', suffix: 'darwin-x64' },
  'x86_64-unknown-linux-gnu': { os: 'linux', cpu: 'x64', libc: 'glibc', suffix: 'linux-x64-gnu' },
  'aarch64-unknown-linux-gnu': { os: 'linux', cpu: 'arm64', libc: 'glibc', suffix: 'linux-arm64-gnu' },
  'x86_64-unknown-linux-musl': { os: 'linux', cpu: 'x64', libc: 'musl', suffix: 'linux-x64-musl' },
  'aarch64-unknown-linux-musl': { os: 'linux', cpu: 'arm64', libc: 'musl', suffix: 'linux-arm64-musl' },
  'x86_64-pc-windows-msvc': { os: 'win32', cpu: 'x64', suffix: 'win32-x64-msvc' },
  'aarch64-pc-windows-msvc': { os: 'win32', cpu: 'arm64', suffix: 'win32-arm64-msvc' },
}

/**
 * How a Wasm runtime loads its artifact.
 * - `'web'`: fetch the binary relative to the runtime module.
 * - `'node-js'`: read it from disk using Node.js.
 * - `'module'`: import a bundler-provided `WebAssembly.Module`, as in Workers.
 * @example 'module'
 */
export type Loading = 'web' | 'node-js' | 'module'

/**
 * A compilation target with target-specific build settings.
 * `loading` is accepted only when `target` is a Wasm target name.
 * @example { target: 'wasm32-unknown-unknown', loading: 'module' }
 * @example { target: 'aarch64-apple-darwin', env: { MACOSX_DEPLOYMENT_TARGET: '11.0' } }
 */
export type TargetOptions = {
  /**
   * Additional Cargo features for this target. Features declared on `crates` remain enabled.
   * @example ['my_crate/simd']
   */
  features?: string[]
  /**
   * Overrides matching keys in the inherited Cargo process environment.
   * Other inherited keys remain available; the parent environment is not modified.
   * @example { MACOSX_DEPLOYMENT_TARGET: '11.0' }
   */
  env?: Record<string, string>
} & ({
  /**
   * Rust target triple, or `'host'` for the current native Node target.
   * @example 'x86_64-unknown-linux-gnu'
   */
  target: TargetName
  loading?: never
} | {
  /**
   * Wasm target triple.
   * @example 'wasm32-unknown-unknown'
   */
  target: WasmTargetName
  /**
   * Wasm loading strategy for single-target inline output. Invalid for native targets.
   * Routed distributions provide separate loading adapters for each environment.
   * @default 'web'
   * @example 'module'
   */
  loading?: Loading
})

/**
 * A Rust target triple or `'host'`, optionally accompanied by target-specific settings.
 * @example 'host'
 * @example { target: 'wasm32-unknown-unknown', loading: 'node-js' }
 */
export type Target = TargetName | TargetOptions

export const recommendedTargets: readonly Target[] = [
  ...Object.keys(nativePlatforms),
  'wasm32-unknown-unknown',
]

export interface ResolvedTarget {
  target: string
  kind: 'node' | 'wasm32' | 'wasm64'
  loading: Loading
  features: string[]
  env: Record<string, string>
  release: boolean
  platform?: NativePlatform
}
