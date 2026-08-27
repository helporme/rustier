use crate::compiler::{
    codegen::{
        fragment::{
            FragmentOnce, FragmentPrint, Printer, comma_list, doubles, frag, fragment, squares,
        },
        ts::jsdoc::render_jsdoc,
    },
    db::Db,
    ir::decals::{DecalFunction, DecalFunctionKind, DecalParam, DecalType},
};

#[salsa::tracked]
impl<'db> DecalFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_ts(self, db: &'db dyn Db) -> String {
        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export function ~{self.name(db)}{render_params(db, self.params(db))}:~{
                render_return_type(db, self.ret_type(db))
            };
        )
        .print()
    }
}

impl<'db> DecalFunction<'db> {
    pub(super) fn render_in_class_ts(self, db: &'db dyn Db) -> String {
        let static_prefix =
            matches!(self.kind(db), DecalFunctionKind::StaticMethod).then_some("static ");

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            {static_prefix}{render_class_method_name(self.name(db))}{
                render_params(db, self.params(db))
            }:~{render_return_type(db, self.ret_type(db))};
        )
        .print()
    }

    pub(super) fn render_in_namespace_ts(self, db: &'db dyn Db) -> String {
        debug_assert_eq!(self.kind(db), DecalFunctionKind::StaticMethod);

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export function ~{self.name(db)}{render_params(db, self.params(db))}:~{
                render_return_type(db, self.ret_type(db))
            };
        )
        .print()
    }
}

fn render_params<'db>(db: &'db dyn Db, params: &'db [DecalParam<'db>]) -> impl FragmentOnce + 'db {
    fragment(move |p: &mut Printer| {
        if params.is_empty() {
            frag!(()).apply(p);
        } else {
            frag!(({
                comma_list(
                    params
                        .iter()
                        .map(|param| frag!({param.name(db)}:~{param.type_(db).render_ts(db)})),
                )
            },))
            .apply(p);
        }
    })
}

fn render_return_type<'db>(
    db: &'db dyn Db,
    ret_type: Option<DecalType<'db>>,
) -> impl FragmentOnce + 'db {
    fragment(move |p: &mut Printer| match ret_type {
        Some(type_) => type_.render_ts(db).apply(p),
        None => frag!(void).apply(p),
    })
}

fn render_class_method_name(name: &str) -> impl FragmentOnce + '_ {
    fragment(move |p| {
        if name == "constructor" {
            squares(doubles(name)).apply(p);
        } else {
            name.apply(p);
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{codegen::fragment::FragmentOnce as _, ir::rust::Rust},
    };

    macro_rules! test_function {
        ($name:ident, $function_name:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let db = &*db;
                        let root = Rust::get(db)
                            .crates(db)
                            .first()
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
