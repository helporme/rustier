import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import type { Plugin } from 'rolldown'
import type { Plugin as VitePlugin, ViteDevServer } from 'vite'
import { Server } from 'server-next'
import { buildRuntime, reportDiagnostics, rewriteRuntimeImports } from './bundle'
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

export interface RustierPluginApi {
  check(profile: string): string[]
}

export type RustierPlugin = Omit<Plugin, 'api'> & Pick<VitePlugin, 'config' | 'configureServer' | 'enforce'> & {
  api: RustierPluginApi
}

const runtimePrefix = 'rustier:runtime/'

export default function rustier(options: RustierOptions): RustierPlugin {
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
    sources: new Map<string, { code: string, diagnostics: string[] }>(),
    reportedDiagnostics: new Set<string>(),
    needsRuntime: false,
    runtimeFileName: null as string | null,
  }]))

  const servedRuntimes = new Map<string, string>()
  const getServer = () => server ??= new Server(serverOptions)

  const plugin: RustierPlugin = {
    name: 'rustier-next',
    api: {
      check(profile) {
        return getServer().check(profile)
      },
    },
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
        state.reportedDiagnostics.clear()

        for (const [id, source] of state.sources) {
          const result = getServer().transformModule(name, id, source.code, `${runtimePrefix}${name}`)
          source.diagnostics = result.diagnostics
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
      if (dev && state.sources.get(id)?.code !== code) state.reportedDiagnostics.clear()
      state.sources.set(id, { code, diagnostics: transformed.diagnostics })
      reportDiagnostics(state.reportedDiagnostics, transformed.diagnostics, message => this.warn(message))

      return { code: transformed.code, map: null }
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

      reportDiagnostics(profiles.get(name)!.reportedDiagnostics, result.diagnostics, message => this.warn(message))

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

        for (const source of state.sources.values()) {
          reportDiagnostics(state.reportedDiagnostics, source.diagnostics, message => this.warn(message))
        }

        if (!state.needsRuntime) continue

        const extension = options.profiles[name].platform.type === 'node' ? 'node' : 'wasm'
        const binaryName = `bridge-${name}.${extension}`
        const result = buildRuntime(getServer(), name, binaryName)

        reportDiagnostics(state.reportedDiagnostics, result.diagnostics, message => this.warn(message))

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
