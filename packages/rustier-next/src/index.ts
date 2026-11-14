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

      const result = {
        fatal: false,
        diagnostics: [] as string[],
        edits: [] as { start: number, end: number, text: string }[],
      }
      const runtimeModulePath = options.runtimeModulePath

      rs`
        use rustier_workspace::Workspace;
        use rustier_compiler::{ir::js::JsFile, diag::Theme};
        use rustier::runtime::js::JsValue;

        let result: JsValue = ${result};
        let path: String = ${path};
        let content: String = ${content};
        let runtime_module_path: String = ${runtimeModulePath};

        Workspace::with_db(${workspace}, |db| {
            let file = JsFile::new(db, path, content);
            let diagnostics = result.get("diagnostics").unwrap();
            let parsing_diagnostics = file.diagnostics(db);
            let module = file.module(db);
            let errors = if parsing_diagnostics.is_empty() {
                module.macros(db).iter()
                    .flat_map(|src| src.check_compile_errors(db))
                    .collect::<Vec<_>>()
            } else {
                result.set("fatal", &result.env().new_boolean(true)).unwrap();
                parsing_diagnostics.clone()
            };

            for (index, diagnostic) in errors.iter().enumerate() {
                let message = diagnostic.render(Theme::UnicodeNoColor).unwrap();
                diagnostics.set(index as u32, &result.env().new_string(message)).unwrap();
            }
            if !parsing_diagnostics.is_empty() {
                return;
            }

            let edits = result.get("edits").unwrap();
            let source = file.contents(db);
            let mut byte_offset = 0;
            let mut utf16_offset = 0;
            for (index, edit) in module.render_transform_edits(db, runtime_module_path).iter().enumerate() {
                utf16_offset += source[byte_offset..edit.start as usize].encode_utf16().count();
                let start = utf16_offset;
                utf16_offset += source[edit.start as usize..edit.end as usize].encode_utf16().count();
                byte_offset = edit.end as usize;

                let value = result.env().new_object();
                value.set("start", &result.env().new_number(start as f64)).unwrap();
                value.set("end", &result.env().new_number(utf16_offset as f64)).unwrap();
                value.set("text", &result.env().new_string(&edit.text)).unwrap();
                edits.set(index as u32, &value).unwrap();
            }
        });
      `

      if (result.fatal) this.error(result.diagnostics.join('\n'))
      for (const diagnostic of result.diagnostics) this.warn(diagnostic)
      if (result.edits.length === 0) return

      const transformed = new RolldownMagicString(content)
      for (const edit of result.edits) {
        if (edit.start === edit.end) {
          transformed.appendLeft(edit.start, edit.text)
        } else {
          transformed.overwrite(edit.start, edit.end, edit.text)
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
