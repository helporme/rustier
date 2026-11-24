export interface NativePlatform {
  os: string
  cpu: string
  libc?: string
  suffix: string
}

export const nativePlatforms = {
  'aarch64-apple-darwin': { os: 'darwin', cpu: 'arm64', suffix: 'darwin-arm64' },
  'x86_64-apple-darwin': { os: 'darwin', cpu: 'x64', suffix: 'darwin-x64' },
  'x86_64-unknown-linux-gnu': { os: 'linux', cpu: 'x64', libc: 'glibc', suffix: 'linux-x64-gnu' },
  'aarch64-unknown-linux-gnu': { os: 'linux', cpu: 'arm64', libc: 'glibc', suffix: 'linux-arm64-gnu' },
  'x86_64-unknown-linux-musl': { os: 'linux', cpu: 'x64', libc: 'musl', suffix: 'linux-x64-musl' },
  'aarch64-unknown-linux-musl': { os: 'linux', cpu: 'arm64', libc: 'musl', suffix: 'linux-arm64-musl' },
  'x86_64-pc-windows-msvc': { os: 'win32', cpu: 'x64', suffix: 'win32-x64-msvc' },
  'aarch64-pc-windows-msvc': { os: 'win32', cpu: 'arm64', suffix: 'win32-arm64-msvc' },
} satisfies Record<string, NativePlatform>

export interface ResolvedTarget {
  target: string
  buildDriver: 'cargo' | 'auto' | 'zigbuild' | 'xwin'
  kind: 'node' | 'wasm32' | 'wasm64'
  loading: 'web' | 'node-js' | 'module'
  features: string[]
  env: Record<string, string>
  release: boolean
  platform?: NativePlatform
}

export type MergedTargetOptions = Pick<ResolvedTarget, 'target'>
  & Partial<Pick<ResolvedTarget, 'buildDriver' | 'loading' | 'features' | 'env'>>

export function resolveTarget(request: string | MergedTargetOptions,
  host: string, release: boolean, driver: ResolvedTarget['buildDriver'] = 'auto'): ResolvedTarget {
  const entry = typeof request === 'string' ? { target: request } : request
  const target = entry.target === 'host' ? host : entry.target
  const platform = Object.hasOwn(nativePlatforms, target) ? nativePlatforms[target as keyof typeof nativePlatforms] : undefined
  const kind = target === 'wasm32-unknown-unknown' ? 'wasm32'
    : target === 'wasm64-unknown-unknown' ? 'wasm64' : 'node'
  if (kind === 'node' && !platform) throw new Error(`Unsupported Rustier target: ${target}`)
  if (kind === 'node' && entry.loading) throw new Error(`loading applies only to Wasm: ${target}`)
  const loading = entry.loading ?? 'web'
  if (!['web', 'node-js', 'module'].includes(loading)) throw new Error(`Invalid Wasm loading mode: ${loading}`)
  if (typeof release !== 'boolean') throw new Error('release must be a boolean')
  return {
    target, kind, platform, loading, release,
    buildDriver: entry.buildDriver ?? driver,
    features: [...new Set(entry.features ?? [])].sort(),
    env: { ...entry.env },
  }
}
