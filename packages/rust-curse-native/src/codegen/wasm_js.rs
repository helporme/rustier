//! IR -> wasm-bindgen JS glue printer.

use std::collections::HashSet;

use super::ir::{Ir, IrId, Item};

/// Options for [`Ir::print_wasm_js`].
pub struct WasmJsOpts {
    /// Path the bundler will resolve to the wasm-bindgen-generated
    /// `package.js`.
    pub source_js: String,
    /// Eagerly import and await the default `__wbg_init` export. Web deploys
    /// need it; node deploys self-initialize on import, so they omit it.
    pub init: bool,
}

impl Ir {
    /// Prints the wasm-bindgen JS glue for a single rust orphan module.
    pub fn print_wasm_js(
        &self,
        module_id: IrId,
        exposed: &HashSet<IrId>,
        opts: WasmJsOpts,
    ) -> String {
        Printer::new(self, opts.source_js, exposed).run(module_id, opts.init)
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

    /// `init` selects whether the default `__wbg_init` export is imported and
    /// awaited (web) or omitted entirely (node).
    fn run(&self, mid: IrId, init: bool) -> String {
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
        if init {
            out.push_str(&format!(
                "import __wbg_init, {{ {import_list} }} from '{}';\n",
                self.source
            ));
            out.push_str("await __wbg_init();\n");
        } else {
            out.push_str(&format!(
                "import {{ {import_list} }} from '{}';\n",
                self.source
            ));
        }
        out.push_str(&format!("export {{ {export_list} }};\n"));
        out
    }
}
