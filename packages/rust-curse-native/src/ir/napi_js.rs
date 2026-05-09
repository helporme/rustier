//! IR -> napi JS glue printer.

use std::collections::HashSet;

use super::mangle::{mangle_fn, mangle_ref};
use super::{Id, Item, IR};

impl IR {
    /// Prints the napi JS glue for a single rust orphan module.
    ///
    /// `source_js` is the path the bundler will resolve to the
    /// napi-generated `index.js`.
    pub fn print_napi_js(&self, module_id: Id, source_js: String, exposed: &HashSet<Id>) -> String {
        Printer::new(self, source_js, exposed).run(module_id)
    }
}

struct Printer<'a> {
    ir: &'a IR,
    source: String,
    exposed: &'a HashSet<Id>,
}

struct Binding {
    js_name: String,
    local_name: String,
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
        let mut bindings: Vec<Binding> = Vec::new();
        for export in exports {
            if !self.exposed.contains(&export.target) {
                continue;
            }
            match self.ir.get_unchecked(export.target) {
                Item::Fn(def) => bindings.push(Binding {
                    js_name: mangle_fn(&def.path),
                    local_name: export.name.clone(),
                }),
                Item::Struct(def) => bindings.push(Binding {
                    js_name: mangle_ref(&def.path),
                    local_name: export.name.clone(),
                }),
                Item::Mod(_) => {}
            }
        }

        if bindings.is_empty() {
            return "export {};\n".to_string();
        }

        let import_list = bindings
            .iter()
            .map(|b| b.js_name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let export_list = bindings
            .iter()
            .map(|b| {
                if b.js_name == b.local_name {
                    b.js_name.clone()
                } else {
                    format!("{} as {}", b.js_name, b.local_name)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let mut out = String::new();
        out.push_str(&format!(
            "import {{ {import_list} }} from '{}';\n",
            self.source
        ));
        out.push_str(&format!("export {{ {export_list} }};\n"));
        out
    }
}
