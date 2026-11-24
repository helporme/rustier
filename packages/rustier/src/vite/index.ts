import { createHash, randomInt } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { basename, posix, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import type { Environment, Plugin, ViteDevServer } from 'vite'
import { buildBatch, checkDrivers, Compilation, createBatch, hostTarget, removeIteration } from '../compiler'
import { applyEdits, relativeImport, rewriteImport } from '../edits'
import { normalizeOptions, type ViteOptions } from './option'
import { syntaxEdits } from './syntax'

export type { EnvironmentOptions, ViteOptions, CrateOptions } from './option'

const runtime = 'virtual:rustier/runtime'
const cargoInput = /(?:\.rs|\/Cargo\.(?:toml|lock)|\/\.cargo\/config(?:\.toml)?|\/rust-toolchain(?:\.toml)?)$/

// Vite serves worker sources through the client environment in development.
function workerScope(id: string) {
  if (/[?&]worker_file(?:&|$)/.test(id)) {
    return createHash('sha256').update(id.split('?')[0]).digest('hex').slice(0, 16)
  }
  return /[?&]rustier-worker=([a-f0-9]{16})(?:&|$)/.exec(id)?.[1] ?? ''
}

class Graph {
  compilation?: Compilation
  readonly effective
  readonly directory: string
  readonly sources = new Map<string, string>()
  readonly crateDirs: string[]
  readonly watchFiles = new Set<string>()
  readonly iterations = new Set<number>()
  readonly devRuntime: string
  output?: { prefix: string, binaryName: string, binary: Buffer, js: string }
  served = false

  constructor(env: Environment, input: string, options: ViteOptions) {
    const effective = normalizeOptions(options, env, hostTarget())
    checkDrivers(effective.targets)
    const scope = createHash('sha256').update(JSON.stringify([env.mode, env.name, input, effective])).digest('hex').slice(0, 16)
    this.directory = resolve(effective.root, '.rustier/vite', scope)
    this.devRuntime = resolve(this.directory, 'runtime.mjs').replaceAll('\\', '/')
    this.effective = effective
    this.crateDirs = Object.values(effective.crates).flatMap(crate => 'path' in crate ? [crate.path.replaceAll('\\', '/')] : [])
  }

  build(ids: Iterable<string>, warn: (message: string) => void, watch?: (path: string) => void) {
    const compilation = this.compilation ??= new Compilation(this.effective.targets, this.effective, resolve(this.directory, 'workspace'))
    if (watch) {
      for (const dir of this.crateDirs) {
        for (const name of ['Cargo.toml', 'Cargo.lock', '.cargo/config.toml', '.cargo/config']) {
          const file = posix.join(dir, name)
          if (existsSync(file)) watch(file)
        }
      }
    }
    const included = new Set(ids)
    compilation.setProgram([...this.sources].filter(([id]) => included.has(id)))
    const batch = createBatch()
    compilation.prepare(batch, this.directory)
    const iteration = randomInt(0x100000000)
    const [result] = buildBatch(batch, iteration, 1)
    for (const warning of result.warnings) warn(warning)
    if (result.error) throw new Error(`${result.target}: ${result.error}`)
    this.iterations.add(iteration)
    if (watch) {
      // Cargo's dep-info also covers include! and build-script rerun-if-changed inputs.
      const rule = readFileSync(result.dep_info!, 'utf8')
      const deps = rule.slice(rule.indexOf(': ') + 2)
      this.watchFiles.clear()
      for (const word of deps.match(/(?:\\ |[^\s])+/g) ?? []) {
        const file = word.replaceAll('\\ ', ' ').replaceAll('\\', '/')
        if (!file.includes('/.rustier/') && this.crateDirs.some(dir => file.startsWith(`${dir}/`))) {
          this.watchFiles.add(file)
          watch(file)
        }
      }
    }
    return { binary: result.binary_path!, compilation, target: this.effective.targets[0] }
  }

  close(warn: (message: string) => void) {
    for (const iteration of this.iterations) {
      try {
        removeIteration(this.directory, iteration)
        this.iterations.delete(iteration)
      } catch (error) {
        warn(`Could not remove Rustier artifact ${iteration}: ${String(error)}`)
      }
    }
  }
}

/** Compile imported `rs` templates for the Vite environment that uses them. */
export default function rustier(options: ViteOptions = {}): Plugin {
  const environments = new WeakMap<Environment, Map<string, Graph>>()
  const inputs = new WeakMap<Environment, string>()
  let devServer: ViteDevServer | undefined

  return {
    name: 'rustier:vite',
    enforce: 'post',
    perEnvironmentStartEndDuringDev: true,
    config() {
      return {
        optimizeDeps: { exclude: ['rustier'] },
        server: { watch: { ignored: ['**/.rustier/**', '**/target/**'] } },
      }
    },
    buildStart({ input }) {
      inputs.set(this.environment, JSON.stringify(input))
    },
    configureServer(server) {
      devServer = server
    },
    transform(code, id) {
      const env = this.environment
      if (env.mode !== 'dev' && env.mode !== 'build') return
      const scope = env.mode === 'dev' ? workerScope(id) : ''
      let graphs = environments.get(env)
      let graph = graphs?.get(scope)
      if (id.includes('/.rustier/') || id.includes('/node_modules/') || /\.d\.[cm]?ts(?:\?|$)/.test(id)
        || /[?&](?:raw|url|worker|sharedworker)(?:[=&]|$)/.test(id)) return
      const edits = code.includes('rustier') && code.includes('`') ? syntaxEdits(id, code, runtime) : []
      if (!edits.length) {
        graph?.sources.delete(id)
        return
      }
      if (!graph) {
        graph = new Graph(env, scope || inputs.get(env) || '', options)
        if (!graphs) environments.set(env, graphs = new Map())
        graphs.set(scope, graph)
      }
      const changed = graph.sources.get(id) !== code
      graph.sources.set(id, code)
      if (env.mode === 'dev' && changed && graph.served) {
        // A lazy import introduced new bindings. Old handles cannot cross runtimes.
        graph.served = false
        env.moduleGraph.invalidateAll()
        env.hot.send({ type: 'full-reload' })
      }
      return applyEdits(code, id, edits)
    },
    resolveId: {
      order: 'pre',
      async handler(id, importer, opts) {
        if (this.environment.mode !== 'dev') {
          if (id === runtime) return { id, external: true, moduleSideEffects: false }
          return
        }
        const scope = importer ? workerScope(importer) : ''
        if (id === runtime) return environments.get(this.environment)?.get(scope)?.devRuntime
        if (!scope || !importer || id.startsWith('\0')
          || /[?&](?:raw|url|worker|sharedworker|rustier-worker)(?:[=&]|$)/.test(id)) return
        const resolved = await this.resolve(id, importer, { ...opts, skipSelf: true })
        if (resolved && !resolved.external && !resolved.id.includes('/node_modules/')
          && /\.(?:[cm]?[jt]sx?|svelte|vue)(?:\?|$)/.test(resolved.id)) {
          return { ...resolved, id: `${resolved.id}${resolved.id.includes('?') ? '&' : '?'}rustier-worker=${scope}` }
        }
        return resolved
      },
    },
    load(id) {
      const env = this.environment
      if (env.mode !== 'dev') return
      const graph = [...environments.get(env)?.values() ?? []].find(graph => id === graph.devRuntime)
      if (!graph) return
      const { binary, compilation, target } = graph.build(graph.sources.keys(), message => this.warn(message), dir => this.addWatchFile(dir))
      const path = binary.replaceAll('\\', '/')
      if (target.kind !== 'node' && target.loading !== 'node-js') {
        // Frameworks can restrict fs.allow to src/. Expose the Wasm file, not the Cargo workspace.
        const allow = env.config.server.fs.allow
        if (!allow.includes(path)) allow.push(path)
      }
      let url = target.kind === 'node' ? binary : target.loading === 'node-js' ? pathToFileURL(binary).href
        : `${env.config.base}@fs/${path}`
      if (env.config.consumer === 'server' && target.kind !== 'node' && target.loading === 'web') {
        const origin = devServer?.resolvedUrls?.local[0]
        if (!origin) this.error('Rustier SSR web loading needs a listening dev server; use node-js loading for Node')
        url = new URL(url, origin).href
      }
      graph.served = true
      return compilation.renderRuntime(url, target.loading)
    },
    hotUpdate({ file, modules, server }) {
      const env = this.environment
      const graphs = environments.get(env)
      if (!graphs) return
      const affected = new Set(modules)
      for (const module of affected) for (const dependency of module.importedModules) affected.add(dependency)
      const dirty = [...graphs.values()].some(graph =>
        graph.watchFiles.has(file) || cargoInput.test(file) && [env.config.root, ...graph.crateDirs].some(dir => file.startsWith(`${dir}/`))
        || [...affected].some(module => module.id && graph.sources.has(module.id))
        || [...graph.sources.keys()].some(id => id.split('?')[0] === file))
      if (!dirty) return
      for (const graph of graphs.values()) graph.close(message => this.warn(message))
      environments.delete(env)
      env.moduleGraph.invalidateAll()
      env.hot.send({ type: 'full-reload' })
      if (env.config.consumer === 'server') server.environments.client?.hot.send({ type: 'full-reload' })
      return []
    },
    watchChange(id) {
      const graph = environments.get(this.environment)?.get('')
      if (graph && (cargoInput.test(id) || graph.watchFiles.has(id))) graph.compilation = undefined
    },
    renderStart() {
      const graph = environments.get(this.environment)?.get('')
      if (graph) graph.output = undefined
    },
    renderChunk(code, chunk, output, { chunks }) {
      const graph = environments.get(this.environment)?.get('')
      if (!graph || !chunk.imports.includes(runtime)) return
      if (output.format !== 'es') this.error('Rustier requires ES module output (use worker.format: "es" for workers)')
      if (!graph.output) {
        const included = Object.values(chunks).flatMap(chunk => chunk.moduleIds)
        const { binary: path, compilation, target } = graph.build(included, message => this.warn(message),
          this.meta.watchMode ? file => this.addWatchFile(file) : undefined)
        const binary = readFileSync(path)
        const binaryName = basename(path)
        const js = compilation.renderRuntime(`./${binaryName}`, target.loading)
        const hash = createHash('sha256').update(js).update(binary).digest('hex').slice(0, 16)
        const prefix = posix.join(this.environment.config.build.assetsDir, `rustier-${hash}`)
        graph.output = { prefix, binaryName, binary, js }
      }
      return rewriteImport(code, chunk.fileName, runtime, relativeImport(chunk.fileName, `${graph.output.prefix}/runtime.mjs`))
    },
    generateBundle: {
      // SSR normally strips client assets. Emit only this environment's required files afterwards.
      order: 'post',
      handler() {
        const output = environments.get(this.environment)?.get('')?.output
        if (!output) return
        this.emitFile({ type: 'asset', fileName: `${output.prefix}/runtime.mjs`, source: output.js })
        this.emitFile({ type: 'asset', fileName: `${output.prefix}/${output.binaryName}`, source: output.binary })
      },
    },
    closeBundle() {
      for (const graph of environments.get(this.environment)?.values() ?? []) graph.close(message => this.warn(message))
    },
  }
}
