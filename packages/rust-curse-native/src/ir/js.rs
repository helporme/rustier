//! IR -> JS re-export printer.

use std::collections::HashSet;

use super::mangle::{mangle_fn, mangle_ref};
use super::{Id, Item, IR};

impl IR {
    /// Prints the IR as a JavaScript re-export module, exposing items in the
    /// `exposed` set.
    pub fn print_js(&self, module_id: Id, source_js: String, exposed: &HashSet<Id>) -> String {
        Printer::new(self, source_js, exposed).run(module_id)
    }
}

struct Printer<'a> {
    ir: &'a IR,
    source: String,
    exposed: &'a HashSet<Id>,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a IR, source: String, exposed: &'a HashSet<Id>) -> Self {
        Self {
            ir,
            source,
            exposed,
        }
    }

    fn run(&self, mid: Id) -> String {
        let exports = &self.ir.get_mod_unchecked(mid).exports;
        let mut lines: Vec<String> = Vec::new();
        for export in exports {
            if !self.exposed.contains(&export.target) {
                continue;
            }
            match self.ir.get_unchecked(export.target) {
                Item::Fn(def) => {
                    let js_name = mangle_fn(&def.path);
                    lines.push(self.reexport_line(&js_name, &export.name));
                }
                Item::Struct(def) => {
                    let js_name = mangle_ref(&def.path);
                    lines.push(self.reexport_line(&js_name, &export.name));
                }
                Item::Mod(_) => {}
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
