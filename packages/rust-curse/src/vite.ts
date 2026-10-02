/*
 * Vite plugin, mostly genearted by AI and used to shape the api of rust server.
 */

// FIXME: should be rewritten and unified with rolldown plugin

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { styleText } from 'node:util'
import { normalizePath } from 'vite'
import type { Plugin, ResolvedConfig, ViteDevServer } from 'vite'
import {
  Server,
  type ServerDiagnostic,
  ServerDiagnosticSeverity,
  type ServerLog,
  ServerLogLevel,
} from 'rust-curse-native'
import { toServerPlatform, type RustCurseTarget } from './target'

export type { RustCurseTarget } from './target'

const ffiModuleIds = [
  '\0$rust/unstable!/node',
  '\0$rust/unstable!/wasm',
] as const

export interface RustCurseConfiguration {
  target?: RustCurseTarget
  /** Defaults to false in Vite dev and true for build/preview. */
  release?: boolean
  /** Additional Cargo features enabled on the source crate. */
  features?: string[]
}

export interface RustCurseOptions {
  /** Cargo manifest relative to Vite's root. Defaults to Cargo.toml. */
  manifestPath?: string
  /** Browser modules default to WebAssembly with fetch loading. */
  browser?: RustCurseConfiguration
  /** Server modules default to the native Node target. */
  ssr?: RustCurseConfiguration
  logging?: {
    enabled?: boolean
    debug?: boolean
    format?: 'pretty' | 'plain'
  }
}

// Structural logging context keeps this entry independent of a bundler package.
type Context = LogContext
interface LogEntry {
  message: string
  id?: string
  pluginCode?: string
  loc?: { file?: string; line: number; column: number }
  url?: string
  meta?: Record<string, unknown>
  stack?: string
}
interface LogContext {
  warn(log: LogEntry): void
  info(log: LogEntry): void
  debug(log: LogEntry): void
  error(log: LogEntry): never
}

/**
 * Rust vite plugin.
 */
export default function rustCurse(options: RustCurseOptions = {}): Plugin {
  let config: ResolvedConfig
  let defaultRelease = false
  let dev: ViteDevServer | undefined
  let native: Server | undefined
  let manifestPath: string
  let rootDir: string

  let serverBinaries: string[] = []
  const moduleBinaries = new Map<string, string>()
  let lastChange:
    | { file: string; timestamp: number; affected: boolean }
    | undefined

  function getServer(ctx: Context): Server {
    if (!native) {
      native = unwrap(
        ctx,
        Server.create({
          rootDir,
          manifestPath,
          ts: true,
        }),
      )
      configureLogging(ctx, native, options.logging)
    }
    return native
  }

  function applyChange(ctx: Context, file: string, deleted: boolean) {
    let contents: string | undefined
    if (!deleted) {
      try {
        contents = readFileSync(file, 'utf8')
      } catch (error) {
        // A queued create/change event may arrive after the file was removed.
        if (!(error instanceof Error) || !('code' in error) || error.code !== 'ENOENT') {
          throw error
        }
      }
    }
    return unwrap(
      ctx,
      getServer(ctx).change({
        filePath: file,
        contents,
      }),
    )
  }

  function buildTarget(
    ctx: Context,
    target: RustCurseTarget,
    release: boolean | undefined,
    features?: string[],
  ) {
    const server = getServer(ctx)
    reportDiagnostics(ctx, server.check())
    const platform = toServerPlatform(target)
    const output = unwrap(
      ctx,
      server.build({
        platform,
        release: release ?? defaultRelease,
        features,
      }),
    )
    if (dev) {
      // Frameworks may restrict fs.allow to source/static directories.
      // Permit compiled binaries without exposing other artifacts.
      for (const binary of output.binPaths) {
        const file = normalizePath(binary)
        if (!dev.config.server.fs.allow.includes(file)) {
          dev.config.server.fs.allow.push(file)
        }
      }
    }
    return output
  }

  const plugin: Plugin = {
    name: 'rust-curse',
    enforce: 'pre',
    perEnvironmentStartEndDuringDev: true,
    api: {
      get server() {
        return native
      },
    },

    config(_config, { command, isPreview }) {
      defaultRelease = command === 'build' || isPreview === true
      return {
        // SSR normally suppresses assets; Rust bindings require sibling binaries.
        build: { emitAssets: true, ssrEmitAssets: true },
        server: {
          watch: {
            ignored: [
              '**/.rust-curse/**',
              (file: string) => {
                const targetDir = normalizePath(path.join(rootDir, 'target'))
                const normalized = normalizePath(file)
                return normalized === targetDir || normalized.startsWith(`${targetDir}/`)
              },
            ],
          },
        },
        // Rust facades and their generated imports are handled by this plugin,
        // not by the dependency scanner.
        optimizeDeps: { exclude: ['$rust'], extensions: [] },
      }
    },

    configResolved(resolved) {
      config = resolved
      manifestPath = path.resolve(
        config.root,
        options.manifestPath ?? 'Cargo.toml',
      )
      rootDir = path.dirname(manifestPath)
    },

    configureServer(server) {
      dev = server
      server.watcher.add([
        path.join(rootDir, 'src'),
        manifestPath,
        path.join(rootDir, 'Cargo.lock'),
        path.join(rootDir, 'build.rs'),
      ])
      server.httpServer?.once('close', () => {
        native?.onLog(undefined)
      })
    },

    buildStart() {
      if (!dev) {
        serverBinaries = []
        moduleBinaries.clear()
      }
      watchRustFiles(this, rootDir, manifestPath)
      const ssr = this.environment.config.consumer === 'server'
      const settings = ssr ? options.ssr : options.browser
      const output = buildTarget(
        this,
        settings?.target ?? (ssr ? 'node' : 'wasm'),
        settings?.release,
        settings?.features,
      )
      if (!dev && ssr) serverBinaries = output.binPaths
    },

    watchChange(file, change) {
      // Dev changes arrive through hotUpdate once per environment. Build-watch
      // changes use the same immediate native update without HMR bookkeeping.
      if (!dev) applyChange(this, file, change.event === 'delete')
    },

    hotUpdate(context) {
      if (
        lastChange?.file !== context.file ||
        lastChange.timestamp !== context.timestamp
      ) {
        lastChange = {
          file: context.file,
          timestamp: context.timestamp,
          affected: applyChange(this, context.file, context.type === 'delete'),
        }
      }
      if (!lastChange.affected) return
      const ssr = this.environment.config.consumer === 'server'
      const settings = ssr ? options.ssr : options.browser

      buildTarget(
        this,
        settings?.target ?? (ssr ? 'node' : 'wasm'),
        settings?.release,
        settings?.features,
      )

      const graph = this.environment.moduleGraph
      const modules = ffiModuleIds
        .map((id) => graph.getModuleById(id))
        .filter((module) => module !== undefined)

      const invalidated = new Set<(typeof modules)[number]>()
      for (const module of modules)
        graph.invalidateModule(module, invalidated, context.timestamp, true)

      return modules
    },

    resolveId(source, importer) {
      if (source === '$rust' || source.startsWith('$rust/'))
        return `\0${source}`
      // Module loading must reach the downstream runtime as a WASM import,
      // not Vite's instantiated exports or an asset URL.
      if (importer === ffiModuleIds[1] && source.endsWith('.wasm')) {
        if (dev) return { id: source, external: true }
        const id = `./${path.basename(source)}`
        moduleBinaries.set(id, source)
        return { id, external: true }
      }
      return null
    },

    load(id, loadOptions) {
      const filePath = id.startsWith('\0$rust') ? id.slice(1) : cleanId(id)
      if (!filePath.endsWith('.rs') && !filePath.startsWith('$rust'))
        return null

      const ssr =
        this.environment?.config.consumer === 'server' ||
        loadOptions?.ssr === true
      const settings = ssr ? options.ssr : options.browser
      const target = settings?.target ?? (ssr ? 'node' : 'wasm')
      const server = getServer(this)
      const code = server.load(toServerPlatform(target), filePath)
      if (code == null) return this.error(`rust-curse could not load ${filePath}`)
      return { code, map: null }
    },

    generateBundle(_options, bundle) {
      const emitted = new Set<string>()
      for (const chunk of Object.values(bundle)) {
        if (chunk.type !== 'chunk') continue
        const directories = new Set(
          Object.keys(chunk.modules).map((id) => path.dirname(id)),
        )
        const binaries = new Set<string>()
        // External WASM belongs beside its importing chunk, not its glue chunk.
        for (const id of chunk.imports) {
          const binary = moduleBinaries.get(id)
          if (binary) binaries.add(binary)
        }
        for (const binary of serverBinaries) {
          if (
            directories.has(path.dirname(binary)) &&
            !moduleBinaries.has(`./${path.basename(binary)}`)
          ) {
            binaries.add(binary)
          }
        }
        for (const binary of binaries) {
          const fileName = path.posix.join(
            path.posix.dirname(chunk.fileName),
            path.basename(binary),
          )
          if (emitted.has(fileName)) continue
          emitted.add(fileName)
          this.emitFile({
            type: 'asset',
            fileName,
            source: readFileSync(binary),
          })
        }
      }
    },

    closeBundle() {
      if (!dev) native?.onLog(undefined)
    },
    closeWatcher() {
      native?.onLog(undefined)
    },
  }
  return plugin
}

function cleanId(id: string) {
  return id.split('?')[0]
}
function watchRustFiles(
  ctx: { addWatchFile(file: string): void },
  root: string,
  manifest: string,
) {
  const src = path.join(root, 'src')
  ctx.addWatchFile(src)
  if (existsSync(src)) {
    for (const entry of readdirSync(src, {
      recursive: true,
      withFileTypes: true,
    })) {
      ctx.addWatchFile(path.join(entry.parentPath, entry.name))
    }
  }
  for (const file of [
    manifest,
    path.join(root, 'Cargo.lock'),
    path.join(root, 'build.rs'),
  ])
    ctx.addWatchFile(file)
}

function configureLogging(
  ctx: Context,
  server: Server,
  options: RustCurseOptions['logging'],
) {
  server.onLog(
    options?.enabled === false
      ? undefined
      : (log) => {
          const verbose =
            log.level === ServerLogLevel.Debug ||
            log.level === ServerLogLevel.Trace
          if (verbose && !options?.debug) return
          const entry = {
            message: options?.format === 'plain' ? log.message : prettyLog(log),
            meta: { target: log.target, fields: log.fields, level: log.level },
          }
          if (
            log.level === ServerLogLevel.Error ||
            log.level === ServerLogLevel.Warn
          )
            ctx.warn(entry)
          else if (verbose) ctx.debug(entry)
          else ctx.info(entry)
        },
  )
}

function prettyLog(log: ServerLog): string {
  const stream = { stream: process.stderr }
  const duration = formatDuration(log.fields.elapsed_ms)
  const fields = Object.entries(log.fields)
    .filter(([key]) => key !== 'elapsed_ms' || duration === undefined)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, original]) => {
      let value = original
      if ((key === 'path' || key === 'cwd') && path.isAbsolute(value)) {
        const relative = path.relative(process.cwd(), value)
        if (
          relative !== '..' &&
          !relative.startsWith(`..${path.sep}`) &&
          !path.isAbsolute(relative)
        )
          value = relative || '.'
      }
      return `${key}=${/\s|=/.test(value) ? JSON.stringify(value) : value}`
    })
    .join(' ')
  return [
    log.message,
    fields ? styleText('dim', fields, stream) : '',
    duration ? styleText('dim', `(${duration})`, stream) : '',
  ]
    .filter(Boolean)
    .join(' ')
}

function formatDuration(value: string | undefined): string | undefined {
  if (value === undefined) return undefined
  const milliseconds = Number(value)
  if (!Number.isFinite(milliseconds) || milliseconds < 0) return undefined
  if (milliseconds < 1000) return `${Math.round(milliseconds)} ms`
  if (milliseconds < 60_000) return `${(milliseconds / 1000).toFixed(2)} s`
  return `${Math.floor(milliseconds / 60_000)} min ${Math.floor((milliseconds % 60_000) / 1000)} s`
}

function diagnosticLog(diagnostic: ServerDiagnostic): LogEntry {
  const file = diagnostic.loc?.file ?? diagnostic.file
  return {
    pluginCode: diagnostic.code,
    message: diagnostic.rendered,
    id: file,
    loc: diagnostic.loc && file ? { ...diagnostic.loc, file } : undefined,
    url: diagnostic.url,
    meta: { diagnostic },
    stack: '',
  }
}

function reportDiagnostics(ctx: Context, diagnostics: ServerDiagnostic[]) {
  let failure: LogEntry | undefined
  for (const diagnostic of diagnostics) {
    const entry = diagnosticLog(diagnostic)
    if (diagnostic.severity === ServerDiagnosticSeverity.Error) {
      if (failure) failure.message += `\n${entry.message}`
      else failure = entry
    } else if (diagnostic.severity === ServerDiagnosticSeverity.Warning)
      ctx.warn(entry)
    else ctx.info(entry)
  }
  if (failure) ctx.error(failure)
}

function unwrap<T>(
  ctx: Context,
  result: { type: 'Ok'; value: T } | { type: 'Err'; error: ServerDiagnostic },
): T {
  if (result.type === 'Err') ctx.error(diagnosticLog(result.error))
  return result.value
}
