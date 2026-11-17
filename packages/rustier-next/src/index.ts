import { createHash } from 'node:crypto'
import { readFileSync, readdirSync } from 'node:fs'
import { posix, resolve } from 'node:path'
import { RolldownMagicString, type Plugin } from 'rolldown'
import { parseSync } from 'rolldown/utils'
import { rs } from 'rustier'

export type Backend =
  | { type: 'node' }
  | { type: 'wasm', loading: 'web' | 'node-js' | 'module', mem64?: boolean }

export interface RustierOptions {
  backend: Backend
  rootDir?: string
  crates?: Record<string, { path: string } | { version: string }>
  features?: string[]
  /**
   * Whether to build optimized Rust artifacts.
   * `'auto'` enables release mode for regular builds and disables it in watch mode.
   * @default 'auto'
   */
  release?: boolean | 'auto'
}

export default function rustier(options: RustierOptions): Plugin {
  let workspace: unknown
  let files: unknown
  let program: unknown
  let backend: unknown
  let workspaceOptions: string
  let iteration = 0
  let currentIteration: number | undefined
  let runtimeFileName: string | undefined
  const retiredIterations = new Set<number>()
  const trackedIds = new Set<string>()
  const root = resolve(options.rootDir ?? process.cwd())
  const outDir = resolve(root, '.rustier/build')
  const runtimeModulePath = 'rustier:runtime'

  return {
    name: 'rustier-next',
    buildStart() {
      runtimeFileName = undefined
      if (workspace) return

      try {
        for (const name of readdirSync(resolve(outDir, 'artifacts'))) {
          if (/^(0|[1-9]\d*)$/.test(name) && Number(name) <= 0xffffffff) {
            retiredIterations.add(Number(name))
            iteration = Math.max(iteration, Number(name) + 1)
          }
        }
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
      }

      const wasm = options.backend.type === 'wasm'
      const mem64 = options.backend.type === 'wasm' && options.backend.mem64 === true
      const loading = options.backend.type === 'wasm' ? options.backend.loading : 'node-js'

      backend = rs`
        use rustier_compiler::codegen::{
            TransportBackend, TransportBackendNode, TransportBackendWasm, WasmImport,
        };

        let loading: String = ${loading};
        if ${wasm} {
            TransportBackend::Wasm(TransportBackendWasm {
                mem64: ${mem64},
                import: match loading.as_str() {
                    "web" => WasmImport::Fetch,
                    "node-js" => WasmImport::NodeJs,
                    "module" => WasmImport::Module,
                    _ => unreachable!(),
                },
            })
        } else {
            TransportBackend::Node(TransportBackendNode)
        }
      `

      workspaceOptions = JSON.stringify({
        tmp_path: resolve(root, '.rustier/workspace'),
        dependency_crates: Object.fromEntries(
          Object.entries(options.crates ?? {}).map(([name, crate]) => [
            name,
            'path' in crate ? { path: resolve(root, crate.path) } : crate,
          ]),
        ),
        features: options.features ?? [],
        target: wasm ? (mem64 ? 'wasm64-unknown-unknown' : 'wasm32-unknown-unknown') : null,
      })

      workspace = rs`
        use rustier_workspace::{Workspace, WorkspaceOpts};

        let json: String = ${workspaceOptions};
        let opts = WorkspaceOpts::from_json(&json)?;

        Ok(Workspace::load(opts)?)
      `
      files = rs`std::collections::BTreeMap::<String, rustier_compiler::ir::js::JsFile>::new()`
      trackedIds.clear()
    },
    resolveId(id) {
      if (id === runtimeModulePath) return { id, external: true }
    },
    transform(content, path) {
      if (path.includes('/node_modules/') || /\.d\.[cm]?ts(?:\?|$)/.test(path)) return
      if (!/\.[cm]?[jt]sx?(?:\?|$)/.test(path) && !content.includes('rustier') && !trackedIds.has(path)) return

      trackedIds.add(path)

      const result: { diagnostics: string[] } | { edits: [number, number, string][] } = JSON.parse(rs`
        use rustier_workspace::Workspace;
        use serde_json::json;
        use rustier_compiler::{ir::js::JsFile, diag::Theme};
        use rustier_compiler::codegen::js_transform::edits_to_utf16;

        let files: &mut std::collections::BTreeMap<String, JsFile> = ${files};
        Workspace::with_db(${workspace}, |db| {
            let file = JsFile::new::<_, String, String>(db, ${path}, ${content});
            files.insert(file.module_id(db).clone(), file);
            let mut diagnostics = file.diagnostics(db);
            let module = file.module(db);

            if diagnostics.is_empty() {
                diagnostics.extend(module.macros(db).iter().flat_map(|src| src.check_compile_errors(db)));
            }

            if !diagnostics.is_empty() {
                let messages: Vec<_> = diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor).unwrap())
                    .collect();

                return json!({ "diagnostics": messages }).to_string();
            }

            let edits: Vec<_> = edits_to_utf16(
                file.contents(db),
                module.render_transform_edits(db, ${runtimeModulePath}),
            )
              .map(|edit| (edit.start, edit.end, edit.text))
              .collect();

            json!({ "edits": edits }).to_string()
        })
      `)

      if ('diagnostics' in result) return this.error(result.diagnostics.join('\n'))
      if (result.edits.length === 0) return

      const transformed = new RolldownMagicString(content)
      for (const [start, end, text] of result.edits) {
        if (start === end) {
          transformed.appendLeft(start, text)
        } else {
          transformed.overwrite(start, end, text)
        }
      }

      return {
        code: transformed.toString(),
        map: transformed.generateMap({ source: path, includeContent: true, hires: true }).toString(),
      }
    },
    buildEnd(error) {
      if (error) return

      const moduleIds = new Set(this.getModuleIds())
      for (const id of trackedIds) {
        if (!moduleIds.has(id)) trackedIds.delete(id)
      }

      const modules = JSON.stringify([...moduleIds])

      program = rs`
        use rustier_workspace::Workspace;
        use rustier_compiler::ir::js::{JsFile, JsProgram};

        let files: &mut std::collections::BTreeMap<String, JsFile> = ${files};
        let modules: String = ${modules};
        let modules: std::collections::BTreeSet<String> = serde_json::from_str(&modules)?;
        files.retain(|id, _| modules.contains(id));

        Ok(Workspace::with_db(${workspace}, |db| {
            JsProgram::new(db, files.values().copied().collect())
        }))
      `

      const hasMacros: boolean = rs`
        use rustier_workspace::Workspace;
        use rustier_compiler::ir::js::JsProgram;
        let program: &JsProgram = ${program};
        Workspace::with_db(${workspace}, |db| !program.runtime(db).macros(db).is_empty())
      `
      if (!hasMacros) {
        if (currentIteration !== undefined) retiredIterations.add(currentIteration)
        currentIteration = undefined
        return
      }
      if (iteration > 0xffffffff) this.error('Rustier build iteration limit reached')

      const buildIteration = iteration++
      const release = options.release === undefined || options.release === 'auto'
        ? !this.meta.watchMode
        : options.release
      const artifact: { binary_path: string, warnings: string[] } = JSON.parse(rs`
        use rustier_workspace::{Workspace, WorkspaceOpts, build::{self, BuildOpts}};
        use rustier_compiler::{ir::js::JsProgram, codegen::TransportBackend};
        let program: &JsProgram = ${program};
        let backend: &TransportBackend = ${backend};
        let json: String = ${workspaceOptions};
        let opts = WorkspaceOpts::from_json(&json)?;
        let out_dir: String = ${outDir};
        let artifact = Workspace::with_db(${workspace}, |db| {
            build::build(db, *program, BuildOpts {
                out_dir: out_dir.into(),
                backend: backend.clone(),
                dependency_crates: opts.dependency_crates,
                features: opts.features,
                release: ${release},
                iteration: ${buildIteration},
            })
        })?;
        Ok(serde_json::json!({
            "binary_path": artifact.binary_path,
            "warnings": artifact.warnings,
        }).to_string())
      `)
      for (const warning of artifact.warnings) this.warn(warning)
      const binary = readFileSync(artifact.binary_path)
      const binaryHash = createHash('sha256').update(binary).digest('hex').slice(0, 16)
      const binaryName = `bridge-${binaryHash}.${options.backend.type === 'node' ? 'node' : 'wasm'}`
      const binaryImport = `./${binaryName}`
      const runtime: string = rs`
        use rustier_workspace::Workspace;
        use rustier_compiler::{ir::js::JsProgram, codegen::TransportBackend};
        let program: &JsProgram = ${program};
        let backend: &TransportBackend = ${backend};
        let binary_import: String = ${binaryImport};
        Workspace::with_db(${workspace}, |db| {
            program.runtime(db).render_js_ffi(db, backend, &binary_import)
        })
      `
      runtimeFileName = `runtime-${createHash('sha256').update(runtime).digest('hex').slice(0, 16)}.mjs`
      this.emitFile({ type: 'asset', fileName: binaryName, source: binary })
      this.emitFile({ type: 'asset', fileName: runtimeFileName, source: runtime })
      if (currentIteration !== undefined) retiredIterations.add(currentIteration)
      currentIteration = buildIteration
    },
    renderStart(output) {
      if (runtimeFileName && output.format !== 'es') {
        this.error('Rustier runtime assets require ES module output')
      }
    },
    renderChunk(code, chunk) {
      if (!runtimeFileName || !chunk.imports.includes(runtimeModulePath)) return

      const result = new RolldownMagicString(code)
      const parsed = parseSync(chunk.fileName, code)
      const relative = posix.relative(posix.dirname(chunk.fileName), runtimeFileName)
      const path = relative.startsWith('.') ? relative : `./${relative}`
      for (const statement of parsed.program.body) {
        if (statement.type === 'ImportDeclaration' && statement.source.value === runtimeModulePath) {
          result.overwrite(statement.source.start, statement.source.end, JSON.stringify(path))
        }
      }
      return { code: result.toString(), map: result.generateMap({ hires: true }).toString() }
    },
    writeBundle() {
      for (const retired of retiredIterations) {
        try {
          rs`
            use rustier_workspace::build::remove_build_iteration;
            let out_dir: String = ${outDir};
            remove_build_iteration(std::path::Path::new(&out_dir), ${retired})
          `
          retiredIterations.delete(retired)
        } catch (error) {
          this.warn(`Could not remove Rustier build iteration ${retired}: ${String(error)}`)
        }
      }
    },
    closeWatcher() {
      program = undefined
      files = undefined
      trackedIds.clear()
      workspace = undefined
      backend = undefined
    },
  }
}
