use miette::{Diagnostic, SourceSpan};
use ra_ap_edition::Edition;
use ra_ap_hir::{
    HasAttrs as _, HasVisibility as _, StructKind, Variant as HirFieldParent, Visibility,
};
use ra_ap_syntax::{AstNode as _, SyntaxKind, ast};
use thiserror::Error;

use super::{
    TypeLowerErrorUnsupported, diagnostic_location, is_clonable, lower_item_span, lower_type,
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
        let target = Type::new(db, TypeOrigin::Struct(self));
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
        let target = Type::new(db, TypeOrigin::Enum(self.target(db)));
        self.declaration(db)
            .hir(db)
            .fields(db)
            .into_iter()
            .filter_map(move |field| lower_qualified_field(db, field, target))
    }
}

fn lower_qualified_field<'db>(
    db: &'db dyn Db,
    hir: ra_ap_hir::Field,
    target: Type<'db>,
) -> Option<FieldQualified<'db>> {
    let field = FieldQualified::new(db, Field::new(db, hir), target);
    validate_field(db, hir, TypeOrigin::QualifiedField(field))
        .ok()
        .flatten()
        .map(|_| field)
}

#[salsa::tracked]
impl<'db> FieldQualified<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_readonly(self, db: &'db dyn Db) -> bool {
        let type_ = Type::new(db, TypeOrigin::QualifiedField(self));
        let crate_ = self
            .declaration(db)
            .hir(db)
            .parent_def(db)
            .module(db)
            .krate(db);
        matches!(
            type_.deborrow(db).kind(db),
            TypeKind::Struct(_) | TypeKind::Enum(_)
        ) && !is_clonable(db, &type_.hir(db), crate_)
    }
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

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_readonly(self, db: &'db dyn Db) -> bool {
        let hir = self.hir(db);
        let crate_ = hir.parent_def(db).module(db).krate(db);
        let type_ = hir.ty(db);
        matches!(
            lower_type(db, TypeOrigin::Field(self), crate_)
                .map(|type_| type_.deborrow(db).kind(db)),
            Ok(TypeKind::Struct(_) | TypeKind::Enum(_))
        ) && !is_clonable(db, &type_, crate_)
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
    let Some(lowered_type) = validate_field(db, field, TypeOrigin::Field(declaration))? else {
        return Ok(None);
    };
    let crate_ = field.parent_def(db).module(db).krate(db);
    if matches!(
        lowered_type.kind(db),
        TypeKind::Struct(_) | TypeKind::Enum(_)
    ) && !is_clonable(db, &field.ty(db), crate_)
    {
        let span = lower_item_span(db, field);
        let (source_code, span) = diagnostic_location(db, span.as_ref());
        FieldReadonlyError {
            field_name: field_name(db, field).to_string(),
            field_type_fqn: lowered_type.display(db).to_owned(),
            source_code,
            span,
        }
        .accumulate(db);
    }
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
    let mut causes = Vec::new();
    if has_explicit_reference_lifetime(db, field) {
        causes.push(TypeLowerErrorUnsupported::LifetimeRef);
    }
    let lowered_type = match lower_type(db, origin, crate_) {
        Ok(type_) => Some(type_.deborrow(db)),
        Err(error) => {
            causes.push(error);
            None
        }
    };
    if !causes.is_empty() {
        let span = lower_item_span(db, field);
        let (source_code, span) = diagnostic_location(db, span.as_ref());
        return Err(Box::new(FieldLowerError {
            field_name: field_name(db, field).to_string(),
            source_code,
            span,
            causes,
        }));
    }
    Ok(lowered_type)
}

fn has_explicit_reference_lifetime<T>(db: &dyn Db, item: T) -> bool
where
    T: ra_ap_hir::HasSource + Copy,
{
    item.source(db).is_some_and(|source| {
        source
            .value
            .syntax()
            .descendants()
            .filter_map(ast::RefType::cast)
            .any(|reference| reference.lifetime().is_some())
    })
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
#[error("the field \"{field_name}\" cannot be exported to JS")]
#[diagnostic(code(rust_curse::skipped_field), severity(Warning))]
pub(super) struct FieldLowerError {
    field_name: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
    #[related]
    causes: Vec<TypeLowerErrorUnsupported>,
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
struct FieldReadonlyError {
    field_name: String,
    field_type_fqn: String,
    #[source_code]
    source_code: DiagSource,
    #[label]
    span: Option<SourceSpan>,
}
