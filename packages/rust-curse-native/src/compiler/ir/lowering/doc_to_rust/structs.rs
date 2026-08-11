use super::{ItemExt, RustdocId, RustdocSpan, fields::FieldLowerError, shapes::RustdocShape};
use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt, DiagSource, DiagSpan},
    ir::rust::{AsItem as _, FieldParent, Mod, Shape, Struct},
};
use miette::Diagnostic;
use rustdoc_types as doc;
use thiserror::Error;

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir) fn lower(
        db: &'db dyn Db,
        id: RustdocId<'db>,
        parent: Mod<'db>,
    ) -> Result<Struct<'db>, Box<StructLowerError>> {
        let item = id.item(db);
        let summary = id.summary(db);

        let doc::ItemEnum::Struct(struct_) = &item.inner else {
            unreachable!("expected struct, got {:?}", item.inner);
        };

        let mut causes = Vec::new();

        if !struct_.generics.params.is_empty() {
            causes.push(StructLowerErrorCause::UnsupportedGeneric);
        }

        if !causes.is_empty() {
            let span = item.span.as_ref().map(|span| {
                let crate_name = id.crate_name(db);
                RustdocSpan::new(db, span, crate_name).lower(db)
            });

            let source_code = span.as_ref().map(|span| span.diag_source(db)).into();
            let span = span.as_ref().map(|span| span.diag_span(db));

            return Err(Box::new(StructLowerError::Skipped {
                struct_fqn: summary.path.join("::"),
                source_code,
                span,
                causes,
            }));
        }

        let name = item.name.as_ref().unwrap();

        Ok(Self::new(db, parent, name))
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir) fn lower_shape(self, db: &'db dyn Db) -> Shape<'db> {
        let struct_id = self.rustdoc_id(db);
        let item = struct_id.item(db);

        let doc::ItemEnum::Struct(def) = &item.inner else {
            unreachable!("expected module, got {:?}", item.inner);
        };

        let shape = match &def.kind {
            doc::StructKind::Unit => RustdocShape::Unit,
            doc::StructKind::Tuple(ids) => RustdocShape::Tuple(ids),
            doc::StructKind::Plain {
                fields,
                has_stripped_fields,
            } => RustdocShape::Plain {
                fields,
                has_stripped_fields: *has_stripped_fields,
            },
        };

        let (shape, errors) = Shape::lower(db, shape, FieldParent::Struct(self));

        if !errors.is_empty() {
            let span = self.lower_span(db).as_ref().map(|span| span.diag_span(db));
            let source_code = self
                .lower_span(db)
                .as_ref()
                .map(|span| span.diag_source(db))
                .into();

            StructLowerError::FieldsSkipped {
                fqn: self.fqn(db).to_string(),
                source_code,
                span,
                causes: errors,
            }
            .accumulate(db);
        }

        shape
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructLowerError {
    #[error("the struct \"{struct_fqn}\" cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    Skipped {
        struct_fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<StructLowerErrorCause>,
    },

    #[error("the struct \"{fqn}\" has fields that cannot be exported to JS")]
    #[diagnostic(code(rust_curse::skipped_struct), severity(Warning))]
    FieldsSkipped {
        fqn: String,
        #[source_code]
        source_code: DiagSource,
        #[label]
        span: Option<DiagSpan>,
        #[related]
        causes: Vec<FieldLowerError>,
    },
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum StructLowerErrorCause {
    #[error("generic structs are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_generic_struct),
        help(
            "export a non-generic wrapper that instantiates the generic parameters with concrete types"
        ),
        severity(Warning)
    )]
    UnsupportedGeneric,
}
