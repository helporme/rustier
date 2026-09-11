import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { styleText } from 'node:util'
import type { Plugin, PluginContext, RolldownError } from 'rolldown'
import {
  Server,
  type ServerDiagnostic,
  ServerDiagnosticSeverity,
  type ServerLog,
  ServerLogLevel,
  ServerPlatform,
  ServerWasmTarget,
} from 'rust-curse-native'

export type RustCurseOptions = {
  /**
   * Path to the Cargo manifest, relative to Rolldown's working directory.
   *
   * @default "Cargo.toml"
   */
  manifestPath?: string
  /**
   * Rust build profiles available to imported modules.
   */
  profiles: Record<string, RustCurseProfile>
  /**
   * Select the build profile for an imported `.rs` module.
   */
  activeProfile: (fileId: string) => string
  /**
   * Control progress logging. Compiler diagnostics are reported independently.
   */
  logging?: {
    /**
     * Whether the plugin should emit progress logs.
     *
     * @default true
     */
    enabled?: boolean
    /**
     * Include debug and trace messages in addition to normal logs.
     *
     * @default false
     */
    debug?: boolean
    /**
     * `"pretty"` uses colorful logs output with nice formatting.
     * `"plain"` emits only the original message, without added formatting.
     *
     * Structured fields remain available in log metadata in either mode.
     *
     * @default "pretty"
     */
    format?: 'pretty' | 'plain'
  }
}

export interface RustCurseProfile {
  target: RustCurseTarget
  /**
   * Whether release build is used.
   */
  release?: boolean
}

export type RustCurseTarget = 'node' | 'wasm' | 'wasm-node'

/**
 * Compile Rust modules and make `.rs` files importable by Rolldown.
 *
 * @example
 * ```ts
 * import { defineConfig } from 'rolldown'
 * import rustCurse from 'rust-curse/rolldown'
 *
 * export default defineConfig({
 *   platform: 'node',
 *   plugins: [
 *     rustCurse({
 *       profiles: {
 *         node: { target: 'node' },
 *         wasm: { target: 'wasm' },
 *       },
 *       activeProfile: (fileId) =>
 *         fileId.includes('.server.') ? 'node' : 'wasm',
 *     }),
 *   ],
 * })
 * ```
 */
export default function rustCurse(options: RustCurseOptions): Plugin {
  const profileNames = Object.keys(options.profiles)

  if (profileNames.length === 0) {
    throw new Error('rust-curse requires at least one build profile')
  }

  if (typeof options.activeProfile !== 'function') {
    throw new Error('rust-curse requires activeProfile')
  }

  let ctx: PluginContext = null!

  let configured = false;
  let manifestPath = path.resolve(options.manifestPath ?? './Cargo.toml')
  let rootDir = path.dirname(manifestPath)

  const serverResult = Server.create({
    rootDir,
    manifestPath,
  })

  const getServer = (ctx: PluginContext) => unwrap(ctx, serverResult)

  return {
    name: 'rust-curse',

    api: {
      get server() {
        return ctx ? getServer(ctx) : undefined
      },
    },

    options(inputOptions) {
      if (inputOptions.watch === false) return null

      const watch = inputOptions.watch ?? {}
      const exclude =
        watch.exclude == null
          ? []
          : Array.isArray(watch.exclude)
            ? watch.exclude
            : [watch.exclude]

      return {
        ...inputOptions,
        watch: {
          ...watch,
          // FIXME: .rust-curse constant is an internal knowledge of server
          exclude: [...exclude, /[/\\]\.rust-curse(?:[/\\]|$)/],
        },
      }
    },

    buildStart() {
      ctx = this
      const server = getServer(this)

      if (!configured) {
        configured = true;
        configureLogging(ctx, server, options.logging)
      }

      watchRustFiles(ctx, rootDir, manifestPath);
      reportDiagnostics(this, server.check())

      for (const profile of Object.values(options.profiles)) {
        const output = unwrap(
          this,
          server.build(
            toServerPlatform(profile.target),
            profile.release ?? false,
          ),
        )

        for (let binPath of output.binPaths) {
          this.emitFile({
            type: 'asset',
            fileName: path.basename(binPath),
            source: readFileSync(binPath),
          })
        }
      }
    },

    watchChange(filePath, change) {
      const server = getServer(this)
      const contents =
        change.event === 'delete' ? undefined : readFileSync(filePath, 'utf8')

      unwrap(this, server.change({ filePath, contents }))
    },

    resolveId(source) {
      if (source.startsWith('$rust')) {
        return `\0${source}`
      }
      return null
    },

    load(id) {
      const filePath = id.startsWith('\0$rust') ? id.slice(1) : id
      if (!filePath.endsWith('.rs') && !filePath.startsWith('$rust')) {
        return null
      }

      const profileName = options.activeProfile(filePath)
      const profile = options.profiles[profileName]
      if (!profile) {
        throw new Error(`Unknown Rust build profile: ${profileName}`)
      }

      const server = getServer(this)
      return server.load(toServerPlatform(profile.target), filePath)
    },

    closeBundle() {
      if (serverResult.type === 'Ok') serverResult.value.onLog(undefined)
    },

    closeWatcher() {
      if (serverResult.type === 'Ok') serverResult.value.onLog(undefined)
    },
  }
}

function configureLogging(ctx: PluginContext, server: Server, options: RustCurseOptions['logging']) {
  if (options?.enabled === false) {
    server.onLog(undefined)
  } else {
    server.onLog((log) => {
      const verbose =
        log.level === ServerLogLevel.Debug ||
        log.level === ServerLogLevel.Trace
      if (verbose && !options?.debug) return

      const entry = {
        message:
          options?.format === 'plain'
            ? log.message
            : prettyLog(log),
        meta: { target: log.target, fields: log.fields, level: log.level },
      }
      if (
        log.level === ServerLogLevel.Error ||
        log.level === ServerLogLevel.Warn
      ) {
        // Trace events are informational; diagnostics control build failure.
        ctx.warn(entry)
      } else if (verbose) {
        ctx.debug(entry)
      } else {
        ctx.info(entry)
      }
    })
  }
}

const logStyles = {
  [ServerLogLevel.Error]: ['red', '[x]'],
  [ServerLogLevel.Warn]: ['yellow', '[!]'],
  [ServerLogLevel.Info]: ['cyan', '[i]'],
  [ServerLogLevel.Debug]: ['gray', '[d]'],
  [ServerLogLevel.Trace]: ['gray', '[t]'],
} as const

function prettyLog(log: ServerLog): string {
  const [color, marker] = logStyles[log.level]
  const stream = { stream: process.stderr }
  const duration = formatDuration(log.fields.elapsed_ms)
  const fields = Object.entries(log.fields)
    .filter(([key]) => key !== 'elapsed_ms' || duration === undefined)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => {
      if ((key === 'path' || key === 'cwd') && path.isAbsolute(value)) {
        const relative = path.relative(process.cwd(), value)
        if (
          relative !== '..' &&
          !relative.startsWith(`..${path.sep}`) &&
          !path.isAbsolute(relative)
        ) {
          value = relative || '.'
        }
      }
      return `${key}=${/\s|=/.test(value) ? JSON.stringify(value) : value}`
    })
    .join(' ')

  return [
    styleText(color, marker, stream),
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

function watchRustFiles(ctx: PluginContext, rootDir: string, manifestPath: string) {
  const srcDir = path.join(rootDir, 'src')

  // FIXME: replace it with enumerating files from the rust module graph
  // or run a separate file watcher since it's kind of crazy to explicitly register each path
  ctx.addWatchFile(srcDir)
  for (const entry of readdirSync(srcDir, {
    recursive: true,
    withFileTypes: true,
  })) {
    if (
      entry.isDirectory() ||
      (entry.isFile() && entry.name.endsWith('.rs'))
    ) {
      ctx.addWatchFile(path.join(entry.parentPath, entry.name))
    }
  }
  ctx.addWatchFile(manifestPath)
  ctx.addWatchFile(path.join(rootDir, 'Cargo.lock'))
}

function toServerPlatform(target: RustCurseTarget): ServerPlatform {
  switch (target) {
    case 'node':
      return { type: 'Node' }
    case 'wasm-node':
      return { type: 'Wasm', target: ServerWasmTarget.Node }
    case 'wasm':
      return { type: 'Wasm', target: ServerWasmTarget.Web }
  }
}

function reportDiagnostics(
  context: PluginContext,
  diagnostics: ServerDiagnostic[],
) {
  let failure: RolldownError | undefined
  for (const diagnostic of diagnostics) {
    const log = diagnosticLog(diagnostic)
    if (diagnostic.severity === ServerDiagnosticSeverity.Error) {
      if (failure) failure.message += `\n${log.message}`
      else failure = { ...log, stack: '' }
    } else if (diagnostic.severity === ServerDiagnosticSeverity.Warning)
      context.warn(log)
    else context.info(log)
  }
  if (failure) context.error(failure)
}

function unwrap<T>(
  context: PluginContext,
  result: { type: 'Ok'; value: T } | { type: 'Err'; error: ServerDiagnostic },
): T {
  if (result.type === 'Err') context.error(diagnosticLog(result.error))
  return result.value
}

function diagnosticLog(diagnostic: ServerDiagnostic): RolldownError {
  const file = diagnostic.loc?.file ?? diagnostic.file
  return {
    pluginCode: diagnostic.code,
    message: diagnostic.rendered,
    id: file,
    // rolldown 1.0's logger requires a filename when loc is present
    loc: diagnostic.loc && file ? { ...diagnostic.loc, file } : undefined,
    url: diagnostic.url,
    meta: { diagnostic },
    stack: '',
  }
}
