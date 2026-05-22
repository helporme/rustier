//! IR -> napi JS glue printer.

use std::collections::HashSet;

use super::ir::{Ir, IrId, Item};

impl Ir {
    /// Prints the napi JS glue for a single rust orphan module.
    ///
    /// `source_js` is the path the bundler will resolve to the
    /// napi-generated `index.js`.
    pub fn print_napi_js(
        &self,
        module_id: IrId,
        source_js: String,
        exposed: &HashSet<IrId>,
    ) -> String {
        Printer::new(self, source_js, exposed).run(module_id)
    }
}

struct Printer<'a> {
    ir: &'a Ir,
    source: String,
    exposed: &'a HashSet<IrId>,
}

struct Binding {
    js_name: String,
    local_name: String,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a Ir, source: String, exposed: &'a HashSet<IrId>) -> Self {
        Self {
            ir,
            source,
            exposed,
        }
    }

    fn run(&self, mid: IrId) -> String {
        let uses = &self.ir.get_mod_unchecked(mid).uses;
        let mut bindings: Vec<Binding> = Vec::new();
        for use_ in uses {
            if !self.exposed.contains(&use_.target) {
                continue;
            }
            match self.ir.get_unchecked(use_.target) {
                Item::Fn(def) => bindings.push(Binding {
                    js_name: def.mangled_name.clone(),
                    local_name: use_.name.clone(),
                }),
                Item::Class(def) => bindings.push(Binding {
                    js_name: def.mangled_name.clone(),
                    local_name: use_.name.clone(),
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
