import { createHash } from 'node:crypto'
import { posix } from 'node:path'
import type { EffectiveOptions, Loading } from '../option'
import type { ResolvedTarget } from '../../targets'
import { renderRouter, type RouterPlan } from './router'

export const binaryNames = { node: 'bridge.node', wasm32: 'bridge.wasm', wasm64: 'bridge.wasm' } as const

export interface GeneratedFile {
  path: string
  contents: string | Uint8Array
}

export interface RuntimeArtifact {
  target: ResolvedTarget
  binary: Uint8Array
  exports: string[]
  runtimes: Record<Loading, string>
}

export interface PackageMetadata {
  name: string
  version: string
  license?: unknown
  repository?: unknown
  engines?: unknown
  publishConfig?: { access?: string, registry?: string, tag?: string }
}

export interface GeneratedPackage {
  directory: string
  name: string
  files: GeneratedFile[]
}

export interface DistributionPlan {
  entry: { type: 'file' | 'package', specifier: string }
  files: GeneratedFile[]
  packages: GeneratedPackage[]
  packageDir?: string
}

export function planDistribution(
  options: EffectiveOptions,
  artifacts: RuntimeArtifact[],
  metadata?: PackageMetadata,
): DistributionPlan {
  if (!artifacts.length) throw new Error('Cannot distribute an empty runtime')

  const expected = JSON.stringify([...artifacts[0].exports].sort())
  for (const artifact of artifacts) {
    if (JSON.stringify([...artifact.exports].sort()) !== expected) {
      throw new Error(`Runtime export mismatch for ${artifact.target.target}`)
    }
  }

  if (artifacts.length !== options.targets.length
    || options.targets.some(target => !artifacts.some(artifact => artifact.target.target === target.target))) {
    throw new Error('Missing target artifacts')
  }

  const npm = options.distribution !== 'inline'
  const config = typeof options.distribution === 'object' ? options.distribution : undefined
  const base = config?.packageName ?? (metadata && `${metadata.name}-bindings`)
  const packageDir = posix.normalize((config?.outDir ?? 'npm').replaceAll('\\', '/'))

  if (npm) {
    if (!metadata?.version || !base || !/^(?:@[a-z0-9._-]+\/)?[a-z0-9][a-z0-9._-]*$/.test(base)) {
      throw new Error('npm distribution requires a package name and version')
    }

    if (!/^[a-zA-Z0-9_./-]+$/.test(packageDir) || packageDir === '.' || packageDir.startsWith('/')
      || packageDir.split('/').includes('..') || ['.git', 'node_modules'].includes(packageDir.split('/')[0])) {
      throw new Error('distribution.outDir must be a relative package directory without traversal')
    }
  }

  const files: GeneratedFile[] = []
  const packages: GeneratedPackage[] = []
  const routes: RouterPlan = { exports: artifacts[0].exports, native: [] }
  const nativeDependencies: Record<string, string> = {}
  const wasmDependencies: Record<string, string> = {}
  let directRuntime = ''

  const identity = createHash('sha256')
  for (const artifact of artifacts) {
    identity.update(artifact.target.target).update(artifact.binary)
    for (const runtime of Object.values(artifact.runtimes)) identity.update(runtime)
  }
  const prefix = `rustier-${identity.digest('hex').slice(0, 16)}`

  for (const artifact of artifacts) {
    const target = artifact.target
    const suffix = target.platform?.suffix ?? target.target
    const name = `${base}-${suffix}`
    const directory = npm ? suffix : `${prefix}/${suffix}`
    const binary = binaryNames[target.kind]

    const content: GeneratedFile[] = [{ path: binary, contents: artifact.binary }]
    const modes: Loading[] = target.kind === 'node' ? ['node-js']
      : !npm && artifacts.length === 1 ? [target.loading] : ['module', 'web', 'node-js']
    for (const mode of modes) content.push({ path: `${mode}.mjs`, contents: artifact.runtimes[mode] })

    const specifier = (mode: Loading) => npm ? `${name}/${mode}` : `./${suffix}/${mode}.mjs`
    directRuntime = `${directory}/${target.kind === 'node' ? 'node-js' : target.loading}.mjs`

    if (target.platform) {
      routes.native.push({ ...target.platform, module: specifier('node-js') })
      if (npm) nativeDependencies[name] = metadata!.version
    } else {
      // Prefer wasm32 when both pointer widths are distributed.
      if (!routes.wasm || target.kind === 'wasm32') {
        routes.wasm = { module: specifier('module'), web: specifier('web'), node: specifier('node-js') }
      }
      if (npm) wasmDependencies[name] = metadata!.version
    }

    if (npm) {
      const manifest = {
        name,
        version: metadata!.version,
        type: 'module',
        license: metadata!.license,
        repository: metadata!.repository,
        engines: metadata!.engines,
        publishConfig: metadata!.publishConfig,

        os: target.platform && [target.platform.os],
        cpu: target.platform && [target.platform.cpu],
        libc: target.platform?.libc && [target.platform.libc],

        exports: Object.fromEntries(modes.map(mode => [`./${mode}`, `./${mode}.mjs`])),
        files: [binary, ...modes.map(mode => `${mode}.mjs`)],
        rustierGenerated: true,
      }
      packages.push({ directory, name, files: [...content, { path: 'package.json', contents: JSON.stringify(manifest, null, 2) + '\n' }] })
    } else {
      files.push(...content.map(file => ({ path: `${directory}/${file.path}`, contents: file.contents })))
    }
  }

  if (!npm && artifacts.length === 1) return { entry: { type: 'file', specifier: directRuntime }, files, packages }

  const routerFiles = Object.entries(renderRouter(routes)).map(([path, contents]) => ({ path, contents }))
  if (!npm) {
    files.push(...routerFiles.map(file => ({ ...file, path: `${prefix}/${file.path}` })))
    return { entry: { type: 'file', specifier: `${prefix}/fallback.mjs` }, files, packages }
  }
  const routerName = `${base}-unknown-unknown-unknown`
  const manifest = {
    name: routerName,
    version: metadata!.version,
    type: 'module',
    license: metadata!.license,
    repository: metadata!.repository,
    engines: metadata!.engines,
    publishConfig: metadata!.publishConfig,

    exports: {
      '.': {
        ...(routes.wasm ? {
          workerd: './wasm-module.mjs',
          deno: './wasm-node.mjs',
          browser: './wasm-web.mjs',
        } : {}),
        bun: routes.native.length ? './node.mjs' : './wasm-node.mjs',
        ...(routes.native.length ? { 'node-addons': './node.mjs' } : {}),
        node: routes.wasm ? './wasm-node.mjs' : './node.mjs',
        ...(routes.wasm ? { worker: './wasm-web.mjs' } : {}),
        default: './fallback.mjs',
      },
    },
    dependencies: wasmDependencies,
    optionalDependencies: nativeDependencies,
    files: routerFiles.map(file => file.path),
    rustierGenerated: true,
  }
  packages.push({
    directory: 'unknown-unknown-unknown', name: routerName,
    files: [...routerFiles, { path: 'package.json', contents: JSON.stringify(manifest, null, 2) + '\n' }],
  })
  return { entry: { type: 'package', specifier: routerName }, files, packages, packageDir }
}
