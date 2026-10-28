import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import type { Plugin } from 'rolldown'
import type { Plugin as VitePlugin, ViteDevServer } from 'vite'
import { Server } from 'server-next'
import { buildRuntime, rewriteRuntimeImports } from './bundle'
import { toProfileOptions, type Platform, type RustierConfiguration } from './options'

export type { Platform } from './options'

export interface RustierProfile extends RustierConfiguration {
  platform: Platform
}

export interface RustierOptions {
  rootDir?: string
  profiles: Record<string, RustierProfile>
  activeProfile: (fileId: string) => string
}

type RustierPlugin = Plugin & Pick<VitePlugin, 'config' | 'configureServer' | 'enforce'>

const runtimePrefix = 'rustier:runtime/'

export default function rustier(options: RustierOptions): Plugin {
  const names = Object.keys(options.profiles)

  if (names.length === 0) {
    throw new Error('rustier requires at least one build profile')
  }

  if (typeof options.activeProfile !== 'function') {
    throw new Error('rustier requires activeProfile')
  }

  const serverOptions = {
    rootDir: resolve(options.rootDir ?? process.cwd()),
    profiles: Object.fromEntries(names.map(name => {
      const profile = options.profiles[name]

      return [name, toProfileOptions(profile, profile.platform, false)]
    })),
  }

  let server: Server | undefined
  let dev: ViteDevServer | undefined

  const profiles = new Map(names.map(name => [name, {
    sources: new Map<string, string>(),
    needsRuntime: false,
    runtimeFileName: null as string | null,
  }]))

  const servedRuntimes = new Map<string, string>()
  const getServer = () => server ??= new Server(serverOptions)

  const plugin: RustierPlugin = {
    name: 'rustier-next',
    enforce: 'post',
    config() {
      return { optimizeDeps: { exclude: ['rustier', 'rustier-next'] } }
    },
    configureServer(vite) {
      dev = vite
    },
    buildStart() {
      for (const [name, state] of profiles) {
        getServer().reset(name)

        for (const [id, code] of state.sources) {
          getServer().transformModule(name, id, code, `${runtimePrefix}${name}`)
        }

        state.needsRuntime = false
        state.runtimeFileName = null
      }
    },
    transform(code, id) {
      if (id.startsWith(`\0${runtimePrefix}`) || id.includes('/node_modules/')) return

      if (
        !code.includes('rustier')
        && ![...profiles.values()].some(state => state.sources.has(id))
      ) return

      const name = options.activeProfile(id)
      const state = profiles.get(name)

      if (!state) {
        this.error(`Unknown Rust build profile: ${name}`)
      }

      if (!dev) {
        for (const [other, previous] of profiles) {
          if (other !== name && previous.sources.delete(id)) {
            getServer().removeModule(other, id)
          }
        }
      }

      let runtimeId = `${runtimePrefix}${name}`

      if (dev) {
        const hash = createHash('sha256').update(id).update(code).digest('hex').slice(0, 16)
        runtimeId += `/${hash}`
        servedRuntimes.set(runtimeId, name)
      }

      const transformed = getServer().transformModule(name, id, code, runtimeId)
      state.sources.set(id, code)

      return { code: transformed, map: null }
    },
    resolveId(id) {
      if (!id.startsWith(runtimePrefix)) return

      if (dev) {
        if (!servedRuntimes.has(id)) {
          this.error(`Unknown Rust runtime: ${id}`)
        }

        return `\0${id}`
      }

      const state = profiles.get(id.slice(runtimePrefix.length))

      if (!state) {
        this.error(`Unknown Rust runtime: ${id}`)
      }

      state.needsRuntime = true

      return { id, external: true }
    },
    load(id) {
      if (!dev || !id.startsWith(`\0${runtimePrefix}`)) return

      const name = servedRuntimes.get(id.slice(1))

      if (!name) {
        this.error(`Unknown Rust runtime: ${id}`)
      }

      const result = getServer().build(name)

      if (!result.binFfiPath) {
        this.error(result.diagnostics.join('\n') || 'Rustier build failed')
      }

      for (const diagnostic of result.diagnostics) {
        this.warn(diagnostic)
      }

      const platform = options.profiles[name].platform
      let binary = result.binFfiPath

      if (platform.type === 'wasm') {
        if (platform.loading === 'node-js') {
          binary = pathToFileURL(binary).href
        } else {
          const path = binary.replaceAll('\\', '/')

          if (!dev.config.server.fs.allow.includes(path)) {
            dev.config.server.fs.allow.push(path)
          }

          binary = `${dev.config.base}@fs/${path}`
        }
      }

      return getServer().renderJsFfi(name, binary)
    },
    buildEnd(error) {
      if (dev || error) return

      const modules = new Set(this.getModuleIds())

      for (const [name, state] of profiles) {
        for (const id of state.sources.keys()) {
          if (modules.has(id)) continue

          getServer().removeModule(name, id)
          state.sources.delete(id)
        }

        if (!state.needsRuntime) continue

        const extension = options.profiles[name].platform.type === 'node' ? 'node' : 'wasm'
        const binaryName = `bridge-${name}.${extension}`
        const result = buildRuntime(getServer(), name, binaryName)

        for (const diagnostic of result.diagnostics) {
          this.warn(diagnostic)
        }

        state.runtimeFileName = result.runtimeFileName

        this.emitFile({ type: 'asset', fileName: binaryName, source: result.binary })
        this.emitFile({ type: 'asset', fileName: result.runtimeFileName, source: result.runtime })
      }
    },
    renderStart(output) {
      const hasRuntime = [...profiles.values()].some(state => state.runtimeFileName !== null)

      if (output.format !== 'es' && hasRuntime) {
        this.error('Rustier runtime assets require ES module output')
      }
    },
    renderChunk(code, chunk) {
      const runtimes = new Map<string, string>()

      for (const [name, state] of profiles) {
        const id = `${runtimePrefix}${name}`

        if (!state.runtimeFileName || !chunk.imports.includes(id)) continue

        runtimes.set(id, state.runtimeFileName)
      }

      if (runtimes.size) return rewriteRuntimeImports(code, chunk.fileName, runtimes)
    },
    closeBundle() {
      server = undefined
    },
    closeWatcher() {
      server = undefined
    },
  }

  return plugin
}
