use std::collections::BTreeMap;

use crate::compiler::{
    codegen::{
        abi::js::AbiJsTarget,
        fragment::{FragmentOnce, comma_list, frag, fragment, spaced},
    },
    db::Db,
    ir::{
        abi::AbiReturn,
        facade::imports_js::FacadeImportItemJs,
        runtime::{Runtime, RuntimeExport, RuntimeFunction, RuntimeModule, RuntimeType},
    },
};

impl<'db> Runtime<'db> {
    pub fn render_js(self, db: &'db dyn Db, target: AbiJsTarget<'_>) -> String {
        let functions: BTreeMap<_, _> = self
            .functions(db)
            .iter()
            .map(|function| (function.symbol(db), *function))
            .collect();

        let functions = spaced(functions.values().map(|function| function.render_js(db)));

        let imports = spaced(self.imports_js(db).iter().map(|export| {
            FacadeImportItemJs::Runtime(*export).render_js_from(db, export.module().virtual_path())
        }));

        target.render_js_with(frag!(
            {imports}@@
            {functions}
        ))
    }
}

impl RuntimeModule {
    pub fn render_js(self) -> &'static str {
        include_str!("rustier-rt.js")
    }
}

impl<'db> RuntimeFunction<'db> {
    fn render_js(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        let symbol = self.symbol(db);
        let abi = self.abi(db);

        fragment(move |p| {
            let AbiReturn::Union(union) = abi.ret_type(db) else {
                frag!(export const ~{symbol}~= abi.{symbol};).apply(p);
                return;
            };

            let error_index = union.variants(db).len() - 1;
            let is_result = matches!(self.ret_type(db), RuntimeType::Result { .. });
            let params = || comma_list((0..self.params(db).len()).map(|index| frag!(_{index})));
            let body = abi.render_js_with(db, frag!(abi.{symbol}({params()})), |value| {
                fragment(move |p| {
                    if value.index == error_index {
                        let error = RuntimeExport::RuntimeError.import_alias();

                        frag!(throw new ~{error}({value.expr});).apply(p);
                    } else if is_result {
                        frag!(return ~{value.whole_expr};).apply(p);
                    } else {
                        frag!(return ~{value.expr};).apply(p);
                    }
                })
            });

            frag!(
                export function ~{symbol}({params()}) {{
                    {body}
                }}
            )
            .apply(p);
        })
    }
}
