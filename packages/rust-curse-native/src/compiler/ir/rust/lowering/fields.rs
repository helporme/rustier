use super::{ItemExt, RustdocId, RustdocSpan, RustdocType, types::TypeLowerError};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource, DiagSpan},
    ir::rust::{Field, FieldName, FieldParent, Type, ValueType},
};
use miette::Diagnostic;
use rustdoc_types as doc;
use thiserror::Error;

#[salsa::tracked]
pub fn lower_field<'db>(
    db: &'db dyn Db,
    id: RustdocId<'db>,
    parent: FieldParent<'db>,
    idx: Option<usize>,
) -> Result<Field<'db>, Box<FieldLowerError>> {
    let item = id.item(db);

    let doc::ItemEnum::StructField(field_type) = &item.inner else {
        unreachable!("expected field, got {:?}", item.inner);
    };

    let mod_ = match parent {
        FieldParent::Struct(s) => s.parent(db),
        FieldParent::Variant(v) => v.parent(db).parent(db),
    };

    let span = item.span.as_ref().map(|span| {
        let crate_name = id.crate_name(db);
        RustdocSpan::new(db, span, crate_name).lower(db).to_owned()
    });

    let name = match item.name.as_ref() {
        Some(name) => FieldName::Name(name.to_owned()),
        None => FieldName::Idx(idx.unwrap()),
    };

    let field_type = RustdocType::new(db, id.crate_name(db), field_type);
    let field_type = Type::lower(db, field_type, mod_)
        .map_err(|causes| {
            let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
            let span = span.as_ref().map(|span| span.diag_span(db));

            FieldLowerError {
                field_name: name.to_string(),
                source_code,
                span,
                causes,
            }
        })?
        .deborrow();

    let is_readonly = match field_type {
        ValueType::Struct(s) => !is_clonable(db, s.rustdoc_id(db)),
        ValueType::Enum(e) => !is_clonable(db, e.rustdoc_id(db)),
        _ => false,
    };

    if is_readonly {
        let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
        let span = span.as_ref().map(|span| span.diag_span(db));

        FieldReadonlyError {
            field_name: name.to_string(),
            field_type_fqn: field_type.fqn(db).to_string(),
            source_code,
            span,
        }
        .accumulate(db);
    }

    Ok(Field::new(
        db,
        parent,
        name,
        field_type,
        is_readonly,
        item.docs.clone(),
        span,
    ))
}

#[salsa::tracked]
pub fn is_clonable<'db>(db: &'db dyn Db, id: RustdocId<'db>) -> bool {
    let item = id.item(db);
    let crate_ = id.crate_(db);

    let impls = match &item.inner {
        doc::ItemEnum::Enum(def) => &def.impls,
        doc::ItemEnum::Struct(def) => &def.impls,
        _ => unreachable!(),
    };

    impls.iter().any(|impl_id| {
        let impl_item = crate_
            .index
            .get(impl_id)
            .expect("enum impl must be present in the rustdoc index");

        let doc::ItemEnum::Impl(impl_) = &impl_item.inner else {
            unreachable!()
        };
        if impl_.is_negative {
            return false;
        }
        let Some(trait_) = &impl_.trait_ else {
            return false;
        };
        let Some(trait_summary) = crate_.paths.get(&trait_.id) else {
            return false;
        };

        trait_summary.path == ["core", "clone", "Clone"]
    })
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error("the field \"{field_name}\" cannot be exported to JS")]
#[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
pub struct FieldLowerError {
    field_name: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<DiagSpan>,
    #[related]
    causes: Vec<TypeLowerError>,
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "the field `{field_name}` is readonly in JS because \"{field_type_fqn}\" does not implement `Clone`"
)]
#[diagnostic(
    code(rust_curse::readonly_field),
    help(
        "derive or implement `Clone` for \"{field_type_fqn}\" to make this field writable from JS"
    ),
    severity(Warning)
)]
pub struct FieldReadonlyError {
    field_name: String,
    field_type_fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<DiagSpan>,
}
