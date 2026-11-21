import { resolve } from 'node:path'
import { rs } from 'rustier'
import type { EffectiveOptions } from './options'
import type { Loading, ResolvedTarget } from './targets'

export type TransformResult =
  | { diagnostics: string[] }
  | { edits: [number, number, string][] }

export interface BuildOutcome {
  target: string
  binary_path?: string
  warnings: string[]
  error?: string
}

export function hostTarget(): string {
  return rs`Ok(rustc_version::version_meta()?.host)`
}

function targetBuilds(targets: ResolvedTarget[]): string {
  return JSON.stringify(targets.map(({ target, buildDriver, features, env, release }) => ({
    target, buildDriver, features, env, release,
  })))
}

export function checkDrivers(targets: ResolvedTarget[]): void {
  const json = targetBuilds(targets)
  rs`
    use rustier_workspace::build::{check_build_drivers, TargetBuild};
    let json: String = ${json};
    let targets: Vec<TargetBuild> = serde_json::from_str(&json)?;
    check_build_drivers(&targets)?;
    Ok(())
  `
}

function createBackend(kind: ResolvedTarget['kind'], loading: Loading): unknown {
  const wasm = kind !== 'node'
  const mem64 = kind === 'wasm64'
  return rs`
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
}

export class Compilation {
  private workspace: unknown
  private files: unknown
  private program: unknown
  private backend: unknown
  private optionsJson: string

  constructor(readonly targets: ResolvedTarget[], options: EffectiveOptions) {
    const first = targets[0]
    this.backend = createBackend(first.kind, first.loading)
    this.optionsJson = JSON.stringify({
      tmp_path: resolve(options.root, '.rustier/workspace', first.kind),
      dependency_crates: options.crates,
      features: [...new Set(targets.flatMap(target => target.features))],
      target: first.kind === 'node' ? null : first.target,
      env: {
        CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? resolve(options.root, '.rustier/workspace/target'),
        ...first.env,
      },
    })
    this.workspace = rs`
      use rustier_workspace::{Workspace, WorkspaceOpts};
      let json: String = ${this.optionsJson};
      Ok(Workspace::load(WorkspaceOpts::from_json(&json)?)?)
    `
    this.files = rs`std::collections::BTreeMap::<String, rustier_compiler::ir::js::JsFile>::new()`
  }

  transform(path: string, content: string, runtime: string): TransformResult {
    return JSON.parse(rs`
      use rustier_workspace::Workspace;
      use rustier_compiler::{
          ir::js::JsFile, diag::Theme, codegen::js_transform::edits_to_utf16,
      };
      let files: &mut std::collections::BTreeMap<String, JsFile> = ${this.files};
      Workspace::with_db(${this.workspace}, |db| {
          let file = JsFile::new::<_, String, String>(db, ${path}, ${content});
          files.insert(file.module_id(db).clone(), file);
          let mut diagnostics = file.diagnostics(db);
          let module = file.module(db);
          if diagnostics.is_empty() {
              diagnostics.extend(module.macros(db).iter().flat_map(|src| src.check_compile_errors(db)));
          }
          if !diagnostics.is_empty() {
              let messages: Vec<_> = diagnostics.iter()
                  .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor).unwrap())
                  .collect();
              return serde_json::json!({ "diagnostics": messages }).to_string();
          }
          let edits: Vec<_> = edits_to_utf16(
              file.contents(db), module.render_transform_edits(db, ${runtime}),
          ).map(|edit| (edit.start, edit.end, edit.text)).collect();
          serde_json::json!({ "edits": edits }).to_string()
      })
    `)
  }

  collectProgram(moduleIds: string[]): string[] {
    const modules = JSON.stringify(moduleIds)
    this.program = rs`
      use rustier_workspace::Workspace;
      use rustier_compiler::ir::js::{JsFile, JsProgram};
      let files: &mut std::collections::BTreeMap<String, JsFile> = ${this.files};
      let modules: String = ${modules};
      let modules: std::collections::BTreeSet<String> = serde_json::from_str(&modules)?;
      files.retain(|id, _| modules.contains(id));
      Ok(Workspace::with_db(${this.workspace}, |db| {
          JsProgram::new(db, files.values().copied().collect())
      }))
    `
    return JSON.parse(rs`
      use rustier_workspace::Workspace;
      use rustier_compiler::{ir::js::JsProgram, codegen::runtime_exports};
      let program: &JsProgram = ${this.program};
      Workspace::with_db(${this.workspace}, |db| {
          serde_json::to_string(&runtime_exports(db, *program)).unwrap()
      })
    `)
  }

  prepare(batch: unknown, outDir: string): void {
    const targets = targetBuilds(this.targets)
    rs`
      use rustier_workspace::{
          Workspace, WorkspaceOpts, build::{write_glue_crate, GlueCrate, GlueOpts, TargetBuild},
      };
      use rustier_compiler::{ir::js::JsProgram, codegen::TransportBackend};
      let batch: &mut Vec<(GlueCrate, Vec<TargetBuild>)> = ${batch};
      let program: &JsProgram = ${this.program};
      let backend: &TransportBackend = ${this.backend};
      let json: String = ${this.optionsJson};
      let opts = WorkspaceOpts::from_json(&json)?;
      let out_dir: String = ${outDir};
      let targets: String = ${targets};
      let targets: Vec<TargetBuild> = serde_json::from_str(&targets)?;
      let glue = Workspace::with_db(${this.workspace}, |db| {
          write_glue_crate(db, *program, GlueOpts {
              out_dir: out_dir.into(),
              backend: backend.clone(),
              dependency_crates: opts.dependency_crates,
          })
      })?;
      batch.push((glue, targets));
      Ok(())
    `
  }

  renderRuntime(binary: string, loading: Loading): string {
    const backend = createBackend(this.targets[0].kind, loading)
    return rs`
      use rustier_workspace::Workspace;
      use rustier_compiler::{ir::js::JsProgram, codegen::TransportBackend};
      let program: &JsProgram = ${this.program};
      let backend: &TransportBackend = ${backend};
      let binary: String = ${binary};
      Workspace::with_db(${this.workspace}, |db| {
          program.runtime(db).render_js_ffi(db, backend, &binary)
      })
    `
  }
}

export function createBatch(): unknown {
  return rs`Vec::<(rustier_workspace::build::GlueCrate, Vec<rustier_workspace::build::TargetBuild>)>::new()`
}

export function buildBatch(batch: unknown, iteration: number, concurrency: number): BuildOutcome[] {
  return JSON.parse(rs`
    use rustier_workspace::build::{self, GlueCrate, TargetBuild, BuildRequest, BatchOpts};
    let batch: &Vec<(GlueCrate, Vec<TargetBuild>)> = ${batch};
    let concurrency: u32 = ${concurrency};
    let requests: Vec<_> = batch.iter().flat_map(|(glue, targets)| {
        targets.iter().cloned().map(move |target| BuildRequest { glue, target })
    }).collect();
    let results: Vec<_> = build::build_glue_crates(&requests, BatchOpts {
        iteration: ${iteration},
        concurrency: std::num::NonZeroUsize::new(concurrency as usize).unwrap(),
    }).into_iter().map(|outcome| match outcome.result {
        Ok(artifact) => serde_json::json!({
            "target": artifact.target, "binary_path": artifact.binary_path,
            "warnings": artifact.warnings,
        }),
        Err(error) => serde_json::json!({
            "target": outcome.target, "error": error.to_string(), "warnings": [],
        }),
    }).collect();
    serde_json::to_string(&results).unwrap()
  `)
}

export function removeIteration(outDir: string, iteration: number): void {
  rs`
    let out_dir: String = ${outDir};
    rustier_workspace::build::remove_build_iteration(std::path::Path::new(&out_dir), ${iteration})
  `
}
