import { pathToFileURL } from 'node:url'
import { normalizePath, type Plugin, type ResolvedConfig, type ViteDevServer } from 'vite'
import { Server } from 'server-next'
import { buildRuntime, rewriteRuntimeImports } from './bundle'
import { toProfileOptions, type RustierConfiguration } from './options'

export type { Platform, RustierConfiguration } from './options'

export interface RustierOptions {
  browser?: RustierConfiguration
  ssr?: RustierConfiguration
}

type ProfileName = 'browser' | 'ssr'

const runtimePrefix = 'rustier:runtime/'

export default function rustier(options: RustierOptions = {}): Plugin {
  let config: ResolvedConfig
  let server: Server
  let dev: ViteDevServer | undefined
  let closed = false
  const profiles = {
    browser: { sources: new Map<string, string>(), needsRuntime: false, runtimeFileName: null as string | null, updating: false },
    ssr: { sources: new Map<string, string>(), needsRuntime: false, runtimeFileName: null as string | null, updating: false },
  }
  const timers = new Map<string, ReturnType<typeof setTimeout>>()

  function close() {
    closed = true
    for (const timer of timers.values()) clearTimeout(timer)
    timers.clear()
    server?.close()
  }

  return {
    name: 'rustier-next',
    enforce: 'post',
    perEnvironmentStartEndDuringDev: true,
    config() {
      return {
        build: { emitAssets: true },
        server: { watch: { ignored: ['**/.rustier/**', '**/target/**'] } },
        optimizeDeps: { exclude: ['rustier', 'rustier-next'] },
      }
    },
    configResolved(resolved) {
      config = resolved
      closed = false
      if (options.browser?.platform?.type === 'node'
        || (options.browser?.platform?.type === 'wasm' && options.browser.platform.loading !== 'web')) {
        throw new Error('Rustier browser profile requires Wasm with web loading')
      }
      server = new Server({
        rootDir: config.root,
        profiles: {
          browser: toProfileOptions(options.browser ?? {}, options.browser?.platform ?? { type: 'wasm', loading: 'web' }, config.command === 'build'),
          ssr: toProfileOptions(options.ssr ?? {}, options.ssr?.platform ?? { type: 'wasm', loading: 'node-js' }, config.command === 'build'),
        },
      })
    },
    configureServer(vite) {
      dev = vite
      server.watch(({ profile, error }) => {
        if (closed) return
        if (error) {
          vite.config.logger.error(error)
          return
        }
        clearTimeout(timers.get(profile))
        timers.set(profile, setTimeout(() => {
          timers.delete(profile)
          for (const environment of Object.values(vite.environments)) {
            const name = environment.config.consumer === 'server' ? 'ssr' : 'browser'
            if (name !== profile) continue
            const runtime = environment.moduleGraph.getModuleById(`\0${runtimePrefix}${name}`)
            if (runtime) {
              void environment.reloadModule(runtime).catch(error => vite.config.logger.error(String(error)))
            }
          }
        }, 50))
      })
      vite.httpServer?.once('close', close)
    },
    buildStart() {
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      server.reset(profile)
      for (const [id, code] of profiles[profile].sources) {
        server.transformModule(profile, id, code, `${runtimePrefix}${profile}`)
      }
      profiles[profile].needsRuntime = false
      profiles[profile].runtimeFileName = null
    },
    resolveId(id) {
      if (!id.startsWith(runtimePrefix)) return
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      if (id !== `${runtimePrefix}${profile}`) return
      profiles[profile].needsRuntime = true
      return dev ? `\0${id}` : { id, external: true }
    },
    load(id) {
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      if (!dev || id !== `\0${runtimePrefix}${profile}`) return
      const result = server.build(profile)
      if (!result.binFfiPath) this.error(result.diagnostics.join('\n') || 'Rustier build failed')
      for (const diagnostic of result.diagnostics) this.warn(diagnostic)
      const path = normalizePath(result.binFfiPath)
      if (!dev.config.server.fs.allow.includes(path)) dev.config.server.fs.allow.push(path)
      const platform = options[profile]?.platform ?? { type: 'wasm', loading: profile === 'browser' ? 'web' : 'node-js' }
      let binary = path
      if (platform.type === 'wasm') {
        if (platform.loading === 'node-js') {
          binary = pathToFileURL(path).href
        } else {
          binary = `${config.base}@fs/${path}`
          if (profile === 'ssr') {
            const origin = dev.resolvedUrls?.local[0]
            if (!origin) this.error('Rustier SSR web loading requires a listening Vite HTTP server')
            binary = new URL(binary, origin).href
          }
        }
      }
      return server.renderJsFfi(profile, binary)
    },
    transform(code, id) {
      if (id.includes(runtimePrefix) || id.includes('/node_modules/')) return
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      const state = profiles[profile]
      if (!code.includes('rustier') && !state.sources.has(id)) return
      const changed = state.sources.get(id) !== code
      const transformed = server.transformModule(profile, id, code, `${runtimePrefix}${profile}`)
      state.sources.set(id, code)
      if (dev && changed && this.environment.mode === 'dev') {
        const environment = this.environment
        const runtime = environment.moduleGraph.getModuleById(`\0${runtimePrefix}${profile}`)
        if (runtime?.transformResult) {
          environment.moduleGraph.invalidateModule(runtime, new Set(), Date.now(), true)
          if (!state.updating) {
            queueMicrotask(() => {
              if (closed) return
              void environment.reloadModule(runtime).catch(error => dev?.config.logger.error(String(error)))
            })
          }
        }
      }
      return { code: transformed, map: null }
    },
    async hotUpdate(context) {
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      const state = profiles[profile]
      if (context.modules.length === 0 && ![...state.sources.keys()].some(id => id.split('?')[0] === context.file)) return
      state.updating = true
      try {
        for (const id of state.sources.keys()) {
          if (id.split('?')[0] !== context.file || context.type !== 'delete') continue
          server.removeModule(profile, id)
          state.sources.delete(id)
        }
        if (context.type !== 'delete') {
          // Compile framework output before the runtime can be requested by HMR.
          for (const module of context.modules) {
            this.environment.moduleGraph.invalidateModule(module)
            await this.environment.transformRequest(module.url)
          }
        }
        const runtime = this.environment.moduleGraph.getModuleById(`\0${runtimePrefix}${profile}`)
        if (!runtime) return context.modules
        this.environment.moduleGraph.invalidateModule(runtime, new Set(), context.timestamp, true)
        return [...context.modules, runtime]
      } finally {
        state.updating = false
      }
    },
    buildEnd(error) {
      if (dev || error) return
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      const state = profiles[profile]
      const modules = new Set(this.getModuleIds())
      for (const id of state.sources.keys()) {
        if (modules.has(id)) continue
        server.removeModule(profile, id)
        state.sources.delete(id)
      }
      if (!state.needsRuntime && !modules.has(`${runtimePrefix}${profile}`)) return
      const platform = options[profile]?.platform
      const binaryName = `bridge-${profile}.${platform?.type === 'node' ? 'node' : 'wasm'}`
      const result = buildRuntime(server, profile, binaryName)
      for (const diagnostic of result.diagnostics) this.warn(diagnostic)
      state.runtimeFileName = result.runtimeFileName
      this.emitFile({ type: 'asset', fileName: binaryName, source: result.binary })
      this.emitFile({ type: 'asset', fileName: result.runtimeFileName, source: result.runtime })
    },
    renderChunk(code, chunk) {
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      const runtime = profiles[profile].runtimeFileName
      const id = `${runtimePrefix}${profile}`
      if (!runtime || !chunk.imports.includes(id)) return
      return rewriteRuntimeImports(code, chunk.fileName, id, runtime)
    },
    renderStart(output) {
      const profile: ProfileName = this.environment.config.consumer === 'server' ? 'ssr' : 'browser'
      if (profiles[profile].runtimeFileName && output.format !== 'es') {
        this.error('Rustier runtime assets require ES module output')
      }
    },
    closeBundle: close,
    closeWatcher: close,
  }
}
