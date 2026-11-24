import type { Environment } from 'vite'
import { type nativePlatforms, resolveTarget } from '../targets'
import { resolveCrates } from '../workspace'

/** Local crate paths are relative to Vite's root; registry dependencies take a Cargo version requirement. */
export type CrateOptions = ({ path: string } | { version: string }) & {
  /** Crate-local feature names. Default features remain enabled. */
  features?: string[]
}

type WasmTargetName = 'wasm32-unknown-unknown' | 'wasm64-unknown-unknown' | (`wasm${number}-${string}` & {})
type TargetName = 'host' | keyof typeof nativePlatforms | WasmTargetName | (string & {})

export type EnvironmentOptions = {
  crates?: Record<string, CrateOptions>
  /** Defaults to debug in serve and release in build. */
  release?: boolean
  buildDriver?: 'cargo' | 'auto' | 'zigbuild' | 'xwin'
  /** Additional Cargo features, e.g. `['my_crate/simd']`. */
  features?: string[]
  /** Override variables in the Cargo process environment. */
  env?: Record<string, string>
} & ({
  /** Defaults to wasm32 for browsers and workers and `'host'` for SSR. */
  target?: TargetName
  loading?: never
} | {
  target?: WasmTargetName
  /** Wasm only. Defaults to `'web'` for browsers and workers and `'node-js'` for Wasm SSR. */
  loading?: 'web' | 'node-js' | 'module'
})

export interface ViteOptions {
  browser?: EnvironmentOptions
  server?: EnvironmentOptions
}

export function normalizeOptions(options: ViteOptions, env: Environment, host: string) {
  const server = env.config.consumer === 'server' && !env.config.isWorker
  const profile = server ? options.server : options.browser
  const root = env.config.root
  const target = resolveTarget({
    ...profile,
    target: profile?.target ?? (server ? 'host' : 'wasm32-unknown-unknown'),
  }, host, profile?.release ?? env.mode === 'build')
  if (!server && (target.kind === 'node' || target.loading === 'node-js')) {
    throw new Error('Rustier browser and worker graphs require a Wasm target without Node.js loading')
  }
  if (target.kind === 'node' && target.target !== host) {
    throw new Error(`Rustier Vite native SSR requires the build host (${host}); use Wasm or build on the deployment platform`)
  }
  if (server && target.kind !== 'node' && !profile?.loading) target.loading = 'node-js'
  return { root, crates: resolveCrates(root, profile?.crates), targets: [target] }
}
