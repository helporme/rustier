use std::collections::BTreeMap;

use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, comma_list, frag, fragment, spaced},
        transport::js::TransportJsTarget,
    },
    db::Db,
    ir::{
        facade::imports_js::FacadeImportItemJs,
        runtime::{Runtime, RuntimeExport, RuntimeFunction, RuntimeModule, RuntimeType},
        transport::TransportReturn,
    },
};

impl<'db> Runtime<'db> {
    pub fn render_js(self, db: &'db dyn Db, target: TransportJsTarget<'_>) -> String {
        let functions: BTreeMap<_, _> = self
            .functions(db)
            .iter()
            .map(|function| (function.symbol(db), *function))
            .collect();

        let functions = spaced(
            functions
                .values()
                .map(|function| function.render_js(db, target)),
        );

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
        include_str!("rustier-root.js")
    }
}

impl<'db> RuntimeFunction<'db> {
    fn render_js(self, db: &'db dyn Db, target: TransportJsTarget<'_>) -> impl FragmentOnce {
        let symbol = self.symbol(db);
        let transport = self.transport(db);

        fragment(move |p| {
            let TransportReturn::Union(_) = transport.ret_type(db) else {
                frag!(export const ~{symbol}~= abi.{symbol};).apply(p);
                return;
            };

            let error_index = self.runtime_error_index(db);
            let is_result = matches!(self.ret_type(db), RuntimeType::Result { .. });
            let types = transport.params(db);
            let params = || comma_list((0..types.len()).map(|index| frag!(_{index})));
            let body = target.render_call(db, transport, move |value| {
                fragment(move |p| {
                    if Some(value.index) == error_index {
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
