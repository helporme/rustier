import { resolve } from 'node:path'
import { RolldownMagicString } from 'rolldown'
import type { Plugin } from 'vite'
import { rs } from 'rustier'

export interface RustierOptions {
  rootDir?: string
  crates?: Record<string, { path: string }>
  features?: string[]
  runtimeModulePath: string
}

export default function rustier(options: RustierOptions): Plugin {
  let workspace: unknown

  return {
    name: 'rustier-next',
    enforce: 'post',
    config() {
      return { optimizeDeps: { exclude: ['rustier'] } }
    },
    configResolved(config) {
      const root = resolve(options.rootDir ?? config.root)
      const json = JSON.stringify({
        tmp_path: resolve(root, '.rustier/workspace'),
        dependency_crates: Object.fromEntries(
          Object.entries(options.crates ?? {}).map(([name, crate]) => [name, resolve(root, crate.path)]),
        ),
        features: options.features ?? [],
      })

      workspace = rs`
        use rustier_workspace::{Workspace, WorkspaceOpts};

        let json: String = ${json};
        let opts = WorkspaceOpts::from_json(&json)?;

        Ok(Workspace::load(opts)?)
      `
    },
    transform(content, path) {
      if (path.includes('/node_modules/') || /\.d\.[cm]?ts(?:\?|$)/.test(path) || !content.includes('rustier')) return

      const runtimeModulePath = options.runtimeModulePath

      const result: { diagnostics: string[] } | { edits: [number, number, string][] } = JSON.parse(rs`
        use rustier_workspace::{Workspace, serde_json::json};
        use rustier_compiler::{ir::js::JsFile, diag::Theme};
        use rustier_compiler::codegen::js_transform::edits_to_utf16;

        Workspace::with_db(${workspace}, |db| {
            let file = JsFile::new::<_, String, String>(db, ${path}, ${content});
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

      if ('diagnostics' in result) this.error(result.diagnostics.join('\n'))
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
    closeWatcher() {
      workspace = undefined
    },
  }
}
