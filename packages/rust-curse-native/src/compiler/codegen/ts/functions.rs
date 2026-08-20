use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, FragmentPrint, Printer, comma_list, fragment},
            ts::jsdoc::render_jsdoc,
        },
        db::Db,
        ir::decals::DecalFunction,
    },
    frag,
};

#[salsa::tracked]
impl<'db> DecalFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_ts(self, db: &'db dyn Db) -> String {
        let params = fragment(move |p: &mut Printer| {
            if self.params(db).is_empty() {
                frag!(()).apply(p);
            } else {
                frag!(({
                    comma_list(
                        self.params(db)
                            .iter()
                            .map(|param| frag!({param.name(db)}:~{param.type_(db).render_ts(db)})),
                    )
                },))
                .apply(p);
            }
        });
        let return_type = fragment(move |p: &mut Printer| match self.ret_type(db) {
            Some(type_) => type_.render_ts(db).apply(p),
            None => frag!(void).apply(p),
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export function ~{self.name(db)}{params}:~{return_type};
        )
        .print()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{codegen::fragment::FragmentOnce as _, ir::rust::Crate, test::TEST_CRATE_NAME},
    };

    macro_rules! test_function {
        ($name:ident, $function_name:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let db = &*db;
                        let root = Crate::by(db, TEST_CRATE_NAME)
                            .expect("test crate should be lowered")
                            .root(db)
                            .shim(db)
                            .decals(db);
                        let function = root
                            .fns(db)
                            .iter()
                            .find(|function| function.name(db) == $function_name)
                            .expect("test function should be lowered");

                        function.render_ts(db).apply(p);
                    },
                }
            }
        };
    }

    test_function!(empty_function, "empty", {
        pub fn empty() {}
    });

    test_function!(documented_mixed_signature, "transform", {
        #[derive(Clone)]
        pub struct Input;

        #[derive(Clone)]
        pub struct Output;

        /// Transforms several JavaScript-compatible values.
        pub fn transform(enabled: bool, label: String, amount: i32, input: Input) -> Output {
            unimplemented!()
        }
    });
}
