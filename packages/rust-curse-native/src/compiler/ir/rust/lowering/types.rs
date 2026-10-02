use miette::Diagnostic;
use ra_ap_hir::{Adt, GenericDef, HirDisplay as _, Mutability, Trait, Type as HirType};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    ir::rust::{
        BorrowedType, Enum, EnumType, GenArg, Struct, StructType, Type, TypeKind, TypeOrigin,
        TypeParam,
    },
};

pub(super) fn lower_type<'db>(
    db: &'db dyn Db,
    origin: TypeOrigin<'db>,
    crate_: ra_ap_hir::Crate,
) -> Result<Type<'db>, TypeLowerErrorUnsupported> {
    let type_ = Type::new(db, origin);
    lower_type_kind(db, type_, &origin.hir(db), crate_)?;
    Ok(type_)
}

#[salsa::tracked]
impl<'db> Type<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_kind(self, db: &'db dyn Db) -> TypeKind<'db> {
        // Synthesized types already describe their kind, including generic and
        // lifetime arguments which are not yet accepted by source lowering.
        match self.origin(db) {
            TypeOrigin::Struct(type_) => return TypeKind::Struct(type_),
            TypeOrigin::Enum(type_) => return TypeKind::Enum(type_),
            TypeOrigin::TypeParameter(param) => return TypeKind::Generic(param),
            TypeOrigin::Borrowed(reference) => return TypeKind::Borrowed(reference),
            TypeOrigin::Dereferenced(type_) => {
                if let TypeOrigin::Borrowed(reference) = type_.origin(db) {
                    return reference.inner(db).kind(db);
                }
            }
            _ => {}
        }
        let hir = self.hir(db);
        if let Some(param) = hir.as_type_param(db) {
            return TypeKind::Generic(TypeParam::new(db, param));
        }
        if let Some((_, mutability)) = hir.as_reference() {
            return TypeKind::Borrowed(BorrowedType::new(
                db,
                mutability == Mutability::Mut,
                Type::new(db, TypeOrigin::Dereferenced(self)),
                None,
            ));
        }
        if let Some(adt) = hir.as_adt()
            && !GenericDef::Adt(adt).params(db).is_empty()
        {
            let args = hir
                .type_arguments()
                .enumerate()
                .map(|(index, _)| {
                    GenArg::Type(Type::new(
                        db,
                        TypeOrigin::TypeArgument { type_: self, index },
                    ))
                })
                .collect::<Vec<_>>();
            return match adt {
                Adt::Struct(hir) => {
                    TypeKind::Struct(StructType::new(db, Struct::new(db, hir), args))
                }
                Adt::Enum(hir) => TypeKind::Enum(EnumType::new(db, Enum::new(db, hir), args)),
                Adt::Union(_) => unreachable!("unions are unsupported"),
            };
        }
        lower_type_kind(db, self, &hir, self.origin(db).crate_(db))
            .expect("unsupported types are discarded during lowering")
    }
}

impl<'db> TypeOrigin<'db> {
    fn crate_(self, db: &'db dyn Db) -> ra_ap_hir::Crate {
        match self {
            Self::Field(field) => field.hir(db).parent_def(db).module(db).krate(db),
            Self::QualifiedField(field) => field.target(db).origin(db).crate_(db),
            Self::CallableParameter { callable, .. }
            | Self::CallableReturn(callable)
            | Self::CallableReceiver(callable) => {
                callable.declaration(db).hir(db).module(db).krate(db)
            }
            Self::TypeArgument { type_, .. } => type_.origin(db).crate_(db),
            Self::FunctionParameter { function, .. }
            | Self::FunctionReturn(function)
            | Self::FunctionReceiver(function) => function.hir(db).module(db).krate(db),
            Self::Struct(type_) => type_.def(db).hir(db).module(db).krate(db),
            Self::Enum(type_) => type_.def(db).hir(db).module(db).krate(db),
            Self::TypeParameter(param) => param.hir(db).module(db).krate(db),
            Self::Borrowed(reference) => reference.inner(db).origin(db).crate_(db),
            Self::Dereferenced(type_) => type_.origin(db).crate_(db),
        }
    }
}

fn lower_type_kind<'db>(
    db: &'db dyn Db,
    source: Type<'db>,
    type_: &HirType<'db>,
    crate_: ra_ap_hir::Crate,
) -> Result<TypeKind<'db>, TypeLowerErrorUnsupported> {
    if let Some((inner, mutability)) = type_.as_reference() {
        if inner.as_reference().is_some() {
            return Err(TypeLowerErrorUnsupported::DoubleReference);
        }
        let inner = lower_type(db, TypeOrigin::Dereferenced(source), crate_)?;
        return Ok(TypeKind::Borrowed(BorrowedType::new(
            db,
            mutability == Mutability::Mut,
            inner,
            None,
        )));
    }

    if let Some(builtin) = type_.as_builtin() {
        let name = builtin.name().display(db, crate_.edition(db)).to_string();
        let value = match name.as_str() {
            "bool" => TypeKind::bool,
            "char" => TypeKind::char,
            "str" => TypeKind::Str,
            "i8" => TypeKind::i8,
            "u8" => TypeKind::u8,
            "i16" => TypeKind::i16,
            "u16" => TypeKind::u16,
            "i32" => TypeKind::i32,
            "u32" => TypeKind::u32,
            "f32" => TypeKind::f32,
            "f64" => TypeKind::f64,
            _ => return Err(TypeLowerErrorUnsupported::Primitive { name }),
        };
        return Ok(value);
    }

    let Some(adt) = type_.as_adt() else {
        return Err(unsupported_type(db, type_, crate_));
    };
    if !GenericDef::Adt(adt).params(db).is_empty() {
        return Err(unsupported_type(db, type_, crate_));
    }
    let value = match adt {
        Adt::Struct(struct_)
            if struct_.name(db).display(db, crate_.edition(db)).to_string() == "String"
                && struct_
                    .module(db)
                    .path_segments(db)
                    .map(|name| name.display(db, crate_.edition(db)).to_string())
                    .collect::<Vec<_>>()
                    .ends_with(&["string".to_owned()]) =>
        {
            TypeKind::String
        }
        Adt::Struct(struct_) => TypeKind::Struct(StructType::new(
            db,
            Struct::new(db, struct_),
            Vec::<GenArg<'db>>::new(),
        )),
        Adt::Enum(enum_) => TypeKind::Enum(EnumType::new(
            db,
            Enum::new(db, enum_),
            Vec::<GenArg<'db>>::new(),
        )),
        Adt::Union(_) => return Err(unsupported_type(db, type_, crate_)),
    };
    Ok(value)
}

pub(super) fn is_clonable(db: &dyn Db, type_: &HirType<'_>, crate_: ra_ap_hir::Crate) -> bool {
    Trait::lang(db, crate_, ra_ap_hir::LangItem::Clone)
        .is_some_and(|clone| type_.impls_trait(db, clone, &[]))
}

fn unsupported_type(
    db: &dyn Db,
    type_: &HirType<'_>,
    crate_: ra_ap_hir::Crate,
) -> TypeLowerErrorUnsupported {
    TypeLowerErrorUnsupported::Type {
        type_: type_.display(db, crate_.to_display_target(db)).to_string(),
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum TypeLowerErrorUnsupported {
    #[error("unsupported type: {type_}")]
    #[diagnostic(code(rust_curse::unsupported_type), severity(Warning))]
    Type { type_: String },

    #[error("the primitive type `{name}` is not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_primitive),
        help(
            "supported primitive types are bool, char, str, i8, i16, i32, u8, u16, u32, f32 and f64"
        ),
        severity(Warning)
    )]
    Primitive { name: String },

    #[error("references with explicit lifetimes are not supported")]
    #[diagnostic(code(rust_curse::unsupported_lifetime_ref), severity(Warning))]
    LifetimeRef,

    #[error("double references are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_double_reference),
        help("use a single reference instead of a double reference"),
        severity(Warning)
    )]
    DoubleReference,
}
