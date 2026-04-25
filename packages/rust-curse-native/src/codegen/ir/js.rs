//! IR -> JS re-export printer.

use super::mangle::{mangle_fn, mangle_ref};
use super::{ExportKind, ModId, IR};

impl IR {
    pub fn print_js(&self, mid: ModId, source_js: String) -> String {
        Printer::new(self, source_js).run(mid)
    }
}

struct Printer<'a> {
    ir: &'a IR,
    source: String,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a IR, source: String) -> Self {
        Self { ir, source }
    }

    fn run(&self, mid: ModId) -> String {
        let exports = &self.ir.mod_(mid).exports;
        let mut lines: Vec<String> = Vec::new();
        for export in exports {
            match &export.kind {
                ExportKind::Fn(fid) => {
                    let canonical = &self.ir.fn_(*fid).path;
                    let js_name = mangle_fn(canonical);
                    lines.push(self.reexport_line(&js_name, &export.name));
                }
                ExportKind::Struct(sid) => {
                    let canonical = &self.ir.struct_(*sid).path;
                    let js_name = mangle_ref(canonical);
                    lines.push(self.reexport_line(&js_name, &export.name));
                }
                ExportKind::Mod(_) => {}
            }
        }
        if lines.is_empty() {
            return "export {};\n".to_string();
        }
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }

    fn reexport_line(&self, js_name: &str, local_name: &str) -> String {
        if js_name == local_name {
            format!("export {{ {js_name} }} from '{}';", self.source)
        } else {
            format!(
                "export {{ {js_name} as {local_name} }} from '{}';",
                self.source
            )
        }
    }
}
