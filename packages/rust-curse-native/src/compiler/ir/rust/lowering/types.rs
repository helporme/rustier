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

pub(super) fn instantiate_type<'db>(
    db: &'db dyn Db,
    source: HirType<'db>,
    args: impl IntoIterator<Item = HirType<'db>>,
) -> HirType<'db> {
    let mut args = args.into_iter().peekable();
    if args.peek().is_none() {
        return source;
    }
    let instantiated = source.instantiate(args);
    instantiated
        .try_rebase_into(db, &source)
        .unwrap_or(instantiated)
}

/// Checks whether the HIR type is a struct named `String` in a `string` module.
pub(super) fn is_string(db: &dyn Db, type_: &HirType<'_>, crate_: ra_ap_hir::Crate) -> bool {
    let Some(Adt::Struct(struct_)) = type_.as_adt() else {
        return false;
    };
    struct_.name(db).display(db, crate_.edition(db)).to_string() == "String"
        && struct_
            .module(db)
            .path_segments(db)
            .map(|name| name.display(db, crate_.edition(db)).to_string())
            .collect::<Vec<_>>()
            .ends_with(&["string".to_owned()])
}

/// Checks whether the HIR type satisfies the `Clone` trait obligation.
pub(super) fn is_clonable(db: &dyn Db, type_: &HirType<'_>, crate_: ra_ap_hir::Crate) -> bool {
    Trait::lang(db, crate_, ra_ap_hir::LangItem::Clone)
        .is_some_and(|clone| type_.impls_trait(db, clone, &[]))
}

/// Lowers a type origin to Rust IR after validating its HIR type.
pub(super) fn lower_type<'db>(
    db: &'db dyn Db,
    origin: TypeOrigin<'db>,
    crate_: ra_ap_hir::Crate,
) -> Result<Type<'db>, TypeLowerErrorUnsupported> {
    let type_ = Type::new(db, origin);
    check_type(db, &origin.hir(db), crate_)?;
    Ok(type_)
}

/// Validates a HIR type and its nested arguments for Rust IR lowering.
pub(super) fn check_type(
    db: &dyn Db,
    type_: &HirType<'_>,
    crate_: ra_ap_hir::Crate,
) -> Result<(), TypeLowerErrorUnsupported> {
    if type_.as_type_param(db).is_some() {
        return Ok(());
    }
    if let Some((inner, _)) = type_.as_reference() {
        if inner.as_reference().is_some() {
            return Err(TypeLowerErrorUnsupported::DoubleReference);
        }
        return check_type(db, &inner, crate_);
    }
    if let Some(builtin) = type_.as_builtin() {
        return lower_builtin(db, builtin, crate_).map(|_| ());
    }
    let Some(adt) = type_.as_adt() else {
        return Err(TypeLowerErrorUnsupported::Type {
            type_: type_.display(db, crate_.to_display_target(db)).to_string(),
        });
    };
    if matches!(adt, Adt::Union(_))
        || GenericDef::Adt(adt)
            .params(db)
            .iter()
            .any(|param| matches!(param, ra_ap_hir::GenericParam::ConstParam(_)))
    {
        return Err(TypeLowerErrorUnsupported::Type {
            type_: type_.display(db, crate_.to_display_target(db)).to_string(),
        });
    }
    for arg in type_.type_arguments() {
        check_type(db, &arg, crate_)?;
    }
    Ok(())
}

#[salsa::tracked]
impl<'db> Type<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_kind(self, db: &'db dyn Db) -> TypeKind<'db> {
        // Synthesized types already describe their kind.
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
        if let Some(adt) = hir.as_adt().filter(|adt| match adt {
            Adt::Struct(hir) => !Struct::new(db, *hir).generic_params(db).is_empty(),
            Adt::Enum(hir) => !Enum::new(db, *hir).generic_params(db).is_empty(),
            Adt::Union(_) => false,
        }) {
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
        let crate_ = self.origin(db).crate_(db);
        if let Some(builtin) = hir.as_builtin() {
            return lower_builtin(db, builtin, crate_)
                .expect("unsupported types are discarded during lowering");
        }

        let adt = hir
            .as_adt()
            .expect("unsupported types are discarded during lowering");

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

        match adt {
            Adt::Struct(_) if is_string(db, &hir, crate_) => TypeKind::String,
            Adt::Struct(struct_) => {
                TypeKind::Struct(StructType::new(db, Struct::new(db, struct_), args))
            }
            Adt::Enum(enum_) => TypeKind::Enum(EnumType::new(db, Enum::new(db, enum_), args)),
            Adt::Union(_) => unreachable!("unions are unsupported"),
        }
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

fn lower_builtin<'db>(
    db: &'db dyn Db,
    builtin: ra_ap_hir::BuiltinType,
    crate_: ra_ap_hir::Crate,
) -> Result<TypeKind<'db>, TypeLowerErrorUnsupported> {
    let name = builtin.name().display(db, crate_.edition(db)).to_string();
    Ok(match name.as_str() {
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
    })
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

    #[error("double references are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_double_reference),
        help("use a single reference instead of a double reference"),
        severity(Warning)
    )]
    DoubleReference,
}
