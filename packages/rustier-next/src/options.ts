import type { ProfileOptions } from 'server-next'

export type Platform = {
  type: 'wasm'
  loading: 'web' | 'node-js'
  mem64?: boolean
} | { type: 'node' }

export interface RustierConfiguration {
  release?: boolean
  platform?: Platform
  crates?: Record<string, { path: string }>
}

export function toProfileOptions(
  options: RustierConfiguration,
  platform: Platform,
  release: boolean,
): ProfileOptions {
  return {
    release: options.release ?? release,
    backend: platform.type === 'node' ? { type: 'node' } : {
      type: 'wasm',
      loading: platform.loading === 'web' ? 'fetch' : 'node-js',
      mem64: platform.mem64 ?? false,
    },
    crates: Object.fromEntries(
      Object.entries(options.crates ?? {}).map(([name, crate]) => [name, crate.path]),
    ),
  }
}
