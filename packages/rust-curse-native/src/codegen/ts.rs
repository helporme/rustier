//! Compiles rustdoc into TypeScript declarations.

pub mod sourcemap;

use crate::codegen::{
    db::Db,
    fragment::{
        comma_list, frag, fragment, spaced, FragmentOnce, FragmentPrint, IntoFnFragment, Printer,
    },
    ir::{
        fn_::Fn,
        struct_::{Struct, StructField},
    },
    rustdoc::{
        mod_::{lower_mod, lower_mod_fns, lower_mod_structs},
        RustdocId,
    },
};

/// Compiles an rustdoc id to ts declaration.
#[salsa::tracked(returns(ref))]
pub fn compile_mod<'db>(db: &'db dyn Db, mod_id: RustdocId<'db>) -> String {
    let mod_ = lower_mod(db, mod_id);
    let fns = lower_mod_fns(db, mod_id);
    let structs = lower_mod_structs(db, mod_id);

    let declarations = || {
        fns.iter()
            .map(|fn_| compile_fn(db, *fn_))
            .chain(structs.iter().map(|struct_| compile_struct(db, *struct_)))
    };

    if let Some(docs) = mod_.docs(db).as_deref() {
        // todo: move to ambient.d.ts and put a comment on declaration of the module
        // so it's will be shown in IDE when hovering over the module name in import
        frag!({compile_jsdoc(Some(docs))}@{spaced(declarations())}).print()
    } else {
        spaced(declarations()).print()
    }
}

/// Compiles an ir function to ts declaration.
#[salsa::tracked(returns(ref))]
fn compile_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let params = (move |printer: &mut Printer| {
        if fn_.params(db).is_empty() {
            "()".apply(printer);
        } else {
            frag!(({
                comma_list(
                    fn_.params(db)
                        .iter()
                        .map(|param| frag!({param.name(db)}:~{param.type_(db).ts_name(db)})),
                )
            },))
            .apply(printer);
        }
    })
    .into_fragment();
    let return_type = match fn_.return_type(db) {
        Some(type_) => &type_.ts_name(db),
        None => "void",
    };

    frag!(
        {compile_jsdoc(fn_.docs(db).as_deref())}
        export function ~{fn_.name(db)}{params}:~{return_type};
    )
    .print()
}

/// Compiles an ir struct to ts declaration.
#[salsa::tracked(returns(ref))]
fn compile_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    let fields = fragment(move |p| {
        let fields = struct_.fields(db);

        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                "\n".apply(p);
            }
            compile_class_field(db, *field).apply(p);
        }

        if !fields.is_empty() {
            "\n".apply(p);
        }
        frag!(
            private __rust_curse_brand: never;@
            private constructor();
        )
        .apply(p);
    });

    frag!(
        {compile_jsdoc(struct_.docs(db).as_deref())}
        export class ~{struct_.name(db)}~{{ {fields} }}
    )
    .print()
}

fn compile_class_field<'db>(db: &'db dyn Db, field: StructField<'db>) -> impl FragmentOnce + 'db {
    let readonly = if field.is_readonly(db) {
        "readonly "
    } else {
        ""
    };

    frag!(
        {compile_jsdoc(field.docs(db).as_deref())}
        public ~{readonly}{field.name(db)}:~{field.type_(db).ts_name(db)};
    )
}

fn compile_jsdoc(docs: Option<&str>) -> impl FragmentOnce + '_ {
    fragment(move |p| {
        let Some(docs) = docs else {
            return;
        };
        let trimmed = docs.trim();

        if trimmed.contains('\n') {
            "/**\n".apply(p);
            for line in trimmed.lines() {
                (" * ", line.trim(), "\n").apply(p);
            }
            " */\n".apply(p);
        } else {
            ("/** ", trimmed, " */\n").apply(p);
        }
    })
}
