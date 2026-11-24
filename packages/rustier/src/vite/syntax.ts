import { rs } from 'rustier'

export function syntaxEdits(path: string, content: string, runtime: string): [number, number, string][] {
  return JSON.parse(rs`
    use rustier_compiler::{codegen::js_transform::edits_to_utf16, db::RootDatabase, ir::js::JsFile};
    let db = RootDatabase::default();
    let file = JsFile::new::<_, String, String>(&db, ${path}, ${content});
    let edits = file.module(&db).syntax_transform_edits(&db, ${runtime});
    let edits: Vec<_> = edits_to_utf16(file.contents(&db), &edits)
        .map(|edit| (edit.start, edit.end, edit.text)).collect();
    serde_json::to_string(&edits).unwrap()
  `)
}
