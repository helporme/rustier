use miette::{Diagnostic, SourceSpan};
use ra_ap_edition::Edition;
use ra_ap_hir::{
    HasAttrs as _, HasVisibility as _, StructKind, Variant as HirFieldParent, Visibility,
};
use ra_ap_syntax::SyntaxKind;
use thiserror::Error;

use super::{
    TypeLowerErrorUnsupported, diagnostic_location, is_stable, lower_item_span, lower_type,
};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{
        Field, FieldName, FieldParent, FieldQualified, Span, Struct, StructType, Type, TypeKind,
        TypeOrigin, Variant, VariantQualified,
    },
};

impl<'db> StructType<'db> {
    pub(in crate::compiler::ir::rust) fn lower_fields(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = FieldQualified<'db>> + 'db {
        let target = self.into_type(db);
        self.def(db)
            .hir(db)
            .fields(db)
            .into_iter()
            .filter_map(move |field| lower_qualified_field(db, field, target))
    }
}

impl<'db> VariantQualified<'db> {
    pub(in crate::compiler::ir::rust) fn lower_fields(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = FieldQualified<'db>> + 'db {
        let target = self.target(db).into_type(db);
        self.declaration(db)
            .hir(db)
            .fields(db)
            .into_iter()
            .filter_map(move |field| lower_qualified_field(db, field, target))
    }
}

#[salsa::tracked]
fn lower_qualified_field<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Field,
    target: Type<'db>,
) -> Option<FieldQualified<'db>> {
    let field = FieldQualified::new(db, Field::new(db, hir), target);

    validate_field(db, hir, TypeOrigin::QualifiedField(field))
        .map_err(|error| {
            if matches!(*error, FieldLowerError::Unsupported { .. }) {
                (*error).accumulate(db);
            }
        })
        .ok()
        .flatten()
        .map(|_| {
            field.is_readonly(db);

            field
        })
}

#[salsa::tracked]
impl<'db> FieldQualified<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_readonly(self, db: &'db dyn Db) -> bool {
        let type_ = Type::new(db, TypeOrigin::QualifiedField(self));
        let value_type = type_.deborrow(db);

        if !matches!(value_type.kind(db), TypeKind::Struct(_) | TypeKind::Enum(_)) {
            return false;
        }

        if value_type.is_clonable(db) == Some(false) {
            let source_span = self.declaration(db).span(db);

            FieldReadonlyWarning {
                field_name: self.declaration(db).name(db).to_string(),
                field_type_fqn: value_type.display(db).to_owned(),
                source_code: source_span
                    .map(|span| span.src(db).into())
                    .unwrap_or_default(),
                span: source_span.map(|span| (span.offset, span.length).into()),
            }
            .accumulate(db);
        }

        type_.is_clonable(db) != Some(true)
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
#[error(
    "the field `{field_name}` is readonly in JS because \"{field_type_fqn}\" does not implement `Clone`"
)]
#[diagnostic(
    code(rustier::readonly_field),
    help(
        "derive or implement `Clone` for \"{field_type_fqn}\" to make this field writable from JS"
    ),
    severity(Warning)
)]
struct FieldReadonlyWarning {
    field_name: String,
    field_type_fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
}

#[salsa::tracked]
impl<'db> Field<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_parent(self, db: &'db dyn Db) -> FieldParent<'db> {
        match self.hir(db).parent_def(db) {
            HirFieldParent::Struct(struct_) => FieldParent::Struct(Struct::new(db, struct_)),
            HirFieldParent::EnumVariant(variant) => FieldParent::Variant(Variant::new(db, variant)),
            HirFieldParent::Union(_) => unreachable!("union fields are not lowered"),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_name(self, db: &'db dyn Db) -> FieldName {
        field_name(db, self.hir(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_escaped_name(self, db: &'db dyn Db) -> String {
        match self.name(db) {
            FieldName::Name(name)
                if SyntaxKind::from_keyword(name, Edition::Edition2024).is_some() =>
            {
                format!("r#{name}")
            }
            name => name.to_string(),
        }
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_type(self, db: &'db dyn Db) -> Type<'db> {
        let hir = self.hir(db);
        lower_type(
            db,
            TypeOrigin::Field(self),
            hir.parent_def(db).module(db).krate(db),
        )
        .expect("unsupported fields are discarded while lowering their parent shape")
        .deborrow(db)
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_docs(self, db: &'db dyn Db) -> Option<String> {
        self.hir(db).hir_docs(db).map(|docs| docs.docs().to_owned())
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_span(self, db: &'db dyn Db) -> Option<Span> {
        lower_item_span(db, self.hir(db))
    }
}

pub(super) fn lower_field<'db>(
    db: &'db dyn Db,
    field: ra_ap_hir::Field,
) -> Result<Option<Field<'db>>, Box<FieldLowerError>> {
    let declaration = Field::new(db, field);
    let Some(_) = validate_field(db, field, TypeOrigin::Field(declaration))? else {
        return Ok(None);
    };
    Ok(Some(declaration))
}

fn validate_field<'db>(
    db: &'db dyn Db,
    field: ra_ap_hir::Field,
    origin: TypeOrigin<'db>,
) -> Result<Option<Type<'db>>, Box<FieldLowerError>> {
    if matches!(field.parent_def(db), HirFieldParent::Struct(_))
        && field.visibility(db) != Visibility::Public
    {
        return Ok(None);
    }
    let crate_ = field.parent_def(db).module(db).krate(db);

    if !is_stable(db, field, field.parent_def(db).module(db)) {
        return Err(Box::new(FieldLowerError::Unstable));
    }

    let mut causes = Vec::new();

    if origin.hir(db).contains_reference(db) {
        causes.push(FieldLowerCause::BorrowedField);
    }
    let lowered_type = match lower_type(db, origin, crate_) {
        Ok(type_) => Some(type_.deborrow(db)),
        Err(TypeLowerErrorUnsupported::Unstable) => {
            return Err(Box::new(FieldLowerError::Unstable));
        }
        Err(error) => {
            causes.push(FieldLowerCause::Type(error));
            None
        }
    };
    if !causes.is_empty() {
        let span = lower_item_span(db, field);
        let (source_code, span) = diagnostic_location(db, span.as_ref());

        return Err(Box::new(FieldLowerError::Unsupported {
            field_name: field_name(db, field).to_string(),
            source_code,
            span,
            causes,
        }));
    }
    Ok(lowered_type)
}

fn field_name(db: &dyn Db, field: ra_ap_hir::Field) -> FieldName {
    match field.parent_def(db) {
        HirFieldParent::Struct(struct_) if struct_.kind(db) == StructKind::Tuple => {
            FieldName::Idx(field.index())
        }
        HirFieldParent::EnumVariant(variant) if variant.kind(db) == StructKind::Tuple => {
            FieldName::Idx(field.index())
        }
        _ => FieldName::Name(field.name(db).as_str().to_owned()),
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum FieldLowerError {
    #[error("unstable items are not supported")]
    Unstable,

    #[error("the field \"{field_name}\" cannot be exported to JS")]
    #[diagnostic(code(rustier::skipped_field), severity(Warning))]
    Unsupported {
        field_name: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<SourceSpan>,
        #[related]
        causes: Vec<FieldLowerCause>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum FieldLowerCause {
    #[error("fields containing borrowed values are not supported")]
    #[diagnostic(code(rustier::unsupported_borrowed_field), severity(Warning))]
    BorrowedField,

    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] TypeLowerErrorUnsupported),
}
