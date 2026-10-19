use std::{collections::BTreeMap, fmt::Write as _, path::Path};

use crate::compiler::{
    codegen::{
        abi::js::AbiJsTarget,
        fragment::{FragmentOnce, FragmentPrint, comma_list, frag, fragment, spaced},
    },
    db::Db,
    ir::{
        abi::{AbiReturn, AbiType},
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

        let wasm = matches!(&target, AbiJsTarget::Wasm { .. });
        let host_import = match &target {
            AbiJsTarget::Wasm { path, .. }
                if functions
                    .values()
                    .any(|function| function.abi(db).uses_js_values(db)) =>
            {
                let path =
                    Path::new(path).with_file_name(rustier_rt::js::wasm::WASM_RUNTIME_FILENAME);
                let path = fragment(move |p| write!(p, "{:?}", path.to_string_lossy()).unwrap());

                Some(
                    frag!(
                        import ~{{
                            __rustier_js_insert,
                            __rustier_js_take,
                            __rustier_js_drop_ref
                        }}~ from ~{path};@@
                    )
                    .print(),
                )
            }
            _ => None,
        };
        let functions = spaced(
            functions
                .values()
                .map(|function| function.render_js(db, wasm)),
        );

        let imports = spaced(self.imports_js(db).iter().map(|export| {
            FacadeImportItemJs::Runtime(*export).render_js_from(db, export.module().virtual_path())
        }));

        target.render_js_with(frag!(
            {host_import}
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
    fn render_js(self, db: &'db dyn Db, wasm: bool) -> impl FragmentOnce + 'db {
        let symbol = self.symbol(db);
        let abi = self.abi(db);

        fragment(move |p| {
            let AbiReturn::Union(union) = abi.ret_type(db) else {
                frag!(export const ~{symbol}~= abi.{symbol};).apply(p);
                return;
            };

            let error_index = union.variants(db).len() - 1;
            let is_result = matches!(self.ret_type(db), RuntimeType::Result { .. });
            let types = abi.params(db);
            let params = || comma_list((0..types.len()).map(|index| frag!(_{index})));
            let call_params = || {
                comma_list(types.iter().enumerate().map(|(index, type_)| {
                    fragment(move |p| {
                        if wasm && *type_ == AbiType::JsValue {
                            frag!(__js_ { index }).apply(p);
                        } else {
                            frag!(_{index}).apply(p);
                        }
                    })
                }))
            };
            let body = abi.render_js_with(db, frag!(abi.{symbol}({call_params()})), |value| {
                fragment(move |p| {
                    if value.index == error_index {
                        let error = RuntimeExport::RuntimeError.import_alias();

                        frag!(throw new ~{error}({value.expr});).apply(p);
                    } else if wasm && value.type_ == AbiType::JsValue {
                        let result = frag!(__rustier_js_take({ value.expr }));

                        if is_result {
                            frag!(return ~[{value.index}, {result}];).apply(p);
                        } else {
                            frag!(return ~{result};).apply(p);
                        }
                    } else if is_result {
                        frag!(return ~{value.whole_expr};).apply(p);
                    } else {
                        frag!(return ~{value.expr};).apply(p);
                    }
                })
            });
            let body = fragment(move |p| {
                for (index, type_) in types.iter().enumerate() {
                    if wasm && *type_ == AbiType::JsValue {
                        frag!(
                            const __js_{index} = __rustier_js_insert(_{index});@
                            try ~{"{"}@
                        )
                        .apply(p);
                    }
                }

                body.apply(p);

                for (index, type_) in types.iter().enumerate().rev() {
                    if wasm && *type_ == AbiType::JsValue {
                        frag!(
                            @{"}"}~ finally ~{"{"}@
                            __rustier_js_drop_ref(__js_{index});@
                            {"}"}@
                        )
                        .apply(p);
                    }
                }
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
