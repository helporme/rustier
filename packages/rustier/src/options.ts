import type { ProfileOptions } from '#server'

export type Platform = {
  type: 'wasm'
  loading: 'web' | 'node-js'
  mem64?: boolean
} | { type: 'node' }

export interface RustierConfiguration {
  release?: boolean
  procMacros?: boolean
  platform?: Platform
  crates?: Record<string, ({ path: string } | { version: string }) & { features?: string[] }>
}

export function toProfileOptions(
  options: RustierConfiguration,
  platform: Platform,
  release: boolean,
): ProfileOptions {
  return {
    release: options.release ?? release,
    procMacros: options.procMacros ?? false,
    backend: platform.type === 'node' ? { type: 'node' } : {
      type: 'wasm',
      loading: platform.loading === 'web' ? 'fetch' : 'node-js',
      mem64: platform.mem64 ?? false,
    },
    crates: options.crates,
  }
}
