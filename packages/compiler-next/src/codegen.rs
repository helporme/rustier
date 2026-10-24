use crate::{
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    ir::{Macro, Part, Transport, Type},
};

#[cfg(test)]
pub mod test;

#[salsa::tracked]
impl<'db> Macro<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let name = self.name(db);
        let return_type = self.return_type(db);
        let return_source = return_type.source(db);
        let arguments = self.body(db).iter().filter_map(|part| match part {
            Part::Text(_) => None,
            Part::Interpolation { type_, .. } => Some(*type_),
        });

        let params = fragment(|printer| {
            for (index, _) in arguments.clone().enumerate() {
                frag!(, ~__rustier_raw_{index}: u32).apply(printer);
            }
        });

        let validation = fragment(|printer| {
            let handles = fragment(|printer| {
                for (index, type_) in arguments.clone().enumerate() {
                    if matches!(type_, Type::Owned(_)) {
                        frag!(__rustier_raw_ { index },).apply(printer);
                    }
                }
            });

            frag!(
                ::runtime_next::runtime::rust_handles::validate_distinct(
                    &[{handles}]
                ).expect("cannot move the same Rust handle twice");@
            )
            .apply(printer);

            for (index, type_) in arguments.clone().enumerate() {
                if let Type::Owned(type_) = type_ {
                    let source = type_.source(db);

                    frag!(
                        ::runtime_next::runtime::rust_handles::RustHandle::from_raw(
                            __rustier_raw_{index}
                        ).validate::<{source}>().expect("invalid Rust handle or type");@
                    )
                    .apply(printer);
                }
            }
        });

        let conversions = fragment(|printer| {
            for (index, type_) in arguments.clone().enumerate() {
                if matches!(type_, Type::Owned(_)) {
                    continue;
                }

                frag!(
                    let __rustier_js_{index} =
                        ::runtime_next::runtime::js::JsValue::from(unsafe {{
                            ::runtime_next::transport::JsValueHandle::clone_from_raw(
                                __rustier_raw_{index}
                            )
                        }});@
                )
                .apply(printer);

                let source = type_.source(db);
                let value = fragment(|printer| {
                    frag!(__rustier_js_ { index }).apply(printer);

                    if let Some(method) = type_.js_conversion() {
                        frag!(.{method}().expect("JavaScript value cannot be converted"))
                            .apply(printer);
                    }
                });

                frag!(let __rustier_arg_{index}: ~{source} = {value};@).apply(printer);
            }

            for (index, type_) in arguments.clone().enumerate() {
                if let Type::Owned(type_) = type_ {
                    let source = type_.source(db);

                    frag!(
                        let __rustier_arg_{index}: ~{source} =
                            ::runtime_next::runtime::rust_handles::RustHandle::from_raw(
                                __rustier_raw_{index}
                            ).take::<{source}>().expect("invalid Rust handle or type");@
                    )
                    .apply(printer);
                }
            }
        });

        let body = fragment(|printer| {
            let mut index = 0;
            for part in self.body(db) {
                match part {
                    Part::Text(text) => text.apply(printer),
                    Part::Interpolation { .. } => {
                        frag!(__rustier_arg_ { index }).apply(printer);
                        index += 1;
                    }
                }
            }
        });

        let export = fragment(|printer| {
            if matches!(return_type, Type::Owned(_)) {
                frag!(
                    ::runtime_next::runtime::rust_handles::RustHandle::new(__rustier_output)
                        .into_raw()
                )
                .apply(printer);
                return;
            }

            let value = fragment(|printer| match return_type {
                Type::Owned(_) => unreachable!(),
                Type::JsValue => frag!(__rustier_output).apply(printer),
                Type::String => frag!(__rustier_env.new_string(__rustier_output)).apply(printer),
                _ => frag!(__rustier_env.new_number(__rustier_output as f64)).apply(printer),
            });

            frag!(::runtime_next::transport::JsValueHandle::from({ value }).into_raw())
                .apply(printer);
        });

        frag!(
            #[unsafe(no_mangle)]@
            pub unsafe extern "C" fn ~{name}(
                __rustier_env_raw: usize{params}
            ) -> u32 ~{"{\n"}
            let __rustier_env = ::runtime_next::runtime::js::JsEnv::from(unsafe {{
                ::runtime_next::transport::JsEnvHandle::from_raw(__rustier_env_raw)
            }});@
            {validation}
            {conversions}
            let __rustier_output: ~{return_source} = {"(|| -> "}{return_source}{" {\n"}
            {body}
            {"\n})();\n"}
            {export}
            {"\n}"}
        )
        .print()
    }
}

#[salsa::tracked]
impl<'db> Transport<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        let functions = fragment(|printer| {
            for function in self.macros(db) {
                frag!({function.render(db)}@@).apply(printer);
            }
        });

        functions.print()
    }
}
