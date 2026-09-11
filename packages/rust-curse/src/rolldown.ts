import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import type { Plugin, PluginContext, RolldownError } from 'rolldown'
import { Server, type ServerDiagnostic, ServerDiagnosticSeverity, ServerPlatform, ServerWasmTarget } from 'rust-curse-native'

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

  let manifestPath = path.resolve(options.manifestPath ?? "./Cargo.toml")
  let rootDir = path.dirname(manifestPath);

  const serverResult = Server.create({
    rootDir,
    manifestPath
  })

  const getServer = (context: PluginContext) =>
    unwrap(context, serverResult)

  return {
    name: 'rust-curse',

    options(inputOptions) {
      if (inputOptions.watch === false) return null

      const watch = inputOptions.watch ?? {}
      const exclude = watch.exclude == null
        ? []
        : Array.isArray(watch.exclude) ? watch.exclude : [watch.exclude]

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
      const server = getServer(this);
      const srcDir = path.join(rootDir, 'src')

      // FIXME: replace it with enumerating files from the rust module graph
      // or run a separate file watcher since it's kind of crazy to explicitly register each path
      this.addWatchFile(srcDir)
      for (const entry of readdirSync(srcDir, { recursive: true, withFileTypes: true })) {
        if (entry.isDirectory() || (entry.isFile() && entry.name.endsWith('.rs'))) {
          this.addWatchFile(path.join(entry.parentPath, entry.name))
        }
      }
      this.addWatchFile(manifestPath)
      this.addWatchFile(path.join(rootDir, 'Cargo.lock'))

      reportDiagnostics(this, server.check())

      for (const profile of Object.values(options.profiles)) {
        const output = unwrap(this, server.build(
          toServerPlatform(profile.target),
          profile.release ?? false,
        ))

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
      const server = getServer(this);
      const contents =
        change.event === 'delete'
          ? undefined
          : readFileSync(filePath, 'utf8')

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

      const server = getServer(this);
      return server.load(toServerPlatform(profile.target), filePath)
    },
  }
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

function reportDiagnostics(context: PluginContext, diagnostics: ServerDiagnostic[]) {
  let failure: RolldownError | undefined
  for (const diagnostic of diagnostics) {
    const log = diagnosticLog(diagnostic)
    if (diagnostic.severity === ServerDiagnosticSeverity.Error) {
      if (failure) failure.message += `\n${log.message}`
      else failure = { ...log, stack: '' }
    } else if (diagnostic.severity === ServerDiagnosticSeverity.Warning) context.warn(log)
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
