use std::collections::HashSet;

use miette::Diagnostic;
use ra_ap_hir::{
    Adt, GenericDef, GenericParam as HirGenericParam, HirDisplay as _, Mutability, Trait,
    Type as HirType,
};
use thiserror::Error;

use super::is_stable;
use crate::compiler::{
    db::Db,
    ir::rust::{
        BorrowedType, Crate, Enum, EnumType, GenArg, Struct, StructType, Type, TypeKind,
        TypeOrigin, TypeParam,
    },
};

pub(super) fn instantiate_adt_type<'db>(
    db: &'db dyn Db,
    source: HirType<'db>,
    adt: Adt,
    args: impl IntoIterator<Item = HirType<'db>>,
    context: GenericDef,
) -> HirType<'db> {
    let mut args = args.into_iter();
    let mut full_args = Vec::new();

    for param in GenericDef::Adt(adt).params(db) {
        let HirGenericParam::TypeParam(param) = param else {
            continue;
        };

        let arg = match param.is_unstable(db).then(|| param.default(db)).flatten() {
            Some(default) => instantiate_type(db, default, full_args.iter().cloned(), context),
            None => args.next().unwrap_or_else(|| param.ty(db)),
        };
        full_args.push(arg);
    }

    instantiate_type(db, source, full_args, context)
}

pub(super) fn instantiate_type<'db>(
    db: &'db dyn Db,
    source: HirType<'db>,
    args: impl IntoIterator<Item = HirType<'db>>,
    context: GenericDef,
) -> HirType<'db> {
    let mut args = args.into_iter().peekable();
    if args.peek().is_none() {
        return source.rebase_into_owner_or_error(db, context);
    }

    let instantiated =
        source.instantiate(args.map(|arg| arg.rebase_into_owner_or_error(db, context)));

    instantiated.rebase_into_owner_or_error(db, context)
}

/// Checks whether the HIR type is the builtin `String` declaration.
pub(super) fn is_string(db: &dyn Db, type_: &HirType<'_>, crate_: ra_ap_hir::Crate) -> bool {
    let Some(Adt::Struct(struct_)) = type_.as_adt() else {
        return false;
    };

    struct_.module(db).krate(db).is_builtin(db)
        && struct_.name(db).display(db, crate_.edition(db)).to_string() == "String"
        && struct_
            .module(db)
            .path_segments(db)
            .map(|name| name.display(db, crate_.edition(db)).to_string())
            .collect::<Vec<_>>()
            == ["string"]
}

/// Checks whether the HIR type satisfies the `Clone` trait obligation.
pub(super) fn is_clonable(
    db: &dyn Db,
    type_: &HirType<'_>,
    crate_: ra_ap_hir::Crate,
) -> Option<bool> {
    if type_.contains_unknown() {
        return None;
    }

    let clonable = Trait::lang(db, crate_, ra_ap_hir::LangItem::Clone)
        .is_some_and(|clone| type_.impls_trait(db, clone, &[]));

    (clonable || type_.generic_params(db).is_empty()).then_some(clonable)
}

pub(super) fn contains_borrowed_values(db: &dyn Db, type_: &HirType<'_>) -> bool {
    let mut pending = vec![type_.clone()];
    let mut visited = HashSet::new();

    while let Some(type_) = pending.pop() {
        if type_.is_reference() {
            return true;
        }

        pending.extend(type_.type_arguments());
        pending.extend(type_.tuple_fields(db));
        pending.extend(type_.as_slice());
        pending.extend(type_.as_array(db).map(|(inner, _)| inner));
        pending.extend(type_.as_raw_ptr().map(|(inner, _)| inner));

        if let Some(adt) = type_.as_adt()
            && visited.insert(adt)
        {
            match adt {
                Adt::Struct(struct_) => {
                    pending.extend(struct_.fields(db).into_iter().map(|field| field.ty(db)));
                }
                Adt::Enum(enum_) => {
                    pending.extend(
                        enum_
                            .variants(db)
                            .into_iter()
                            .flat_map(|variant| variant.fields(db))
                            .map(|field| field.ty(db)),
                    );
                }
                Adt::Union(union) => {
                    pending.extend(union.fields(db).into_iter().map(|field| field.ty(db)));
                }
            }
        }
    }

    false
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

    if !is_stable(db, adt, adt.module(db)) {
        return Err(TypeLowerErrorUnsupported::Unstable);
    }

    if let Adt::Enum(enum_) = adt {
        for variant in enum_.variants(db) {
            if !is_stable(db, variant, enum_.module(db)) {
                return Err(TypeLowerErrorUnsupported::Unstable);
            }
        }
    }

    let params = GenericDef::Adt(adt).params(db);

    if params.iter().any(|param| {
        matches!(param, HirGenericParam::TypeParam(param)
            if param.is_unstable(db) && param.default(db).is_none())
    }) {
        return Err(TypeLowerErrorUnsupported::Unstable);
    }

    if matches!(adt, Adt::Union(_))
        || params
            .iter()
            .any(|param| matches!(param, HirGenericParam::ConstParam(_)))
    {
        return Err(TypeLowerErrorUnsupported::Type {
            type_: type_.display(db, crate_.to_display_target(db)).to_string(),
        });
    }

    if contains_borrowed_values(db, type_) {
        return Err(TypeLowerErrorUnsupported::BorrowedValues {
            type_: type_.display(db, crate_.to_display_target(db)).to_string(),
        });
    }

    let params = params.into_iter().filter_map(|param| match param {
        HirGenericParam::TypeParam(param) => Some(param),
        _ => None,
    });

    for (index, (arg, param)) in type_.type_arguments().zip(params).enumerate() {
        if param.is_unstable(db)
            && let Some(default) = param.default(db)
        {
            let args = type_
                .type_arguments()
                .take(index)
                .map(|arg| arg.rebase_into_or_error(db, type_));
            let default = default.instantiate(args).rebase_into_or_error(db, type_);

            if !arg.contains_unknown()
                && arg.generic_params(db).is_empty()
                && !default.contains_unknown()
                && default.generic_params(db).is_empty()
                && default
                    .try_rebase_into(db, &arg)
                    .is_some_and(|default| !arg.could_unify_with(db, &default))
            {
                return Err(TypeLowerErrorUnsupported::Unstable);
            }
        } else {
            check_type(db, &arg, crate_)?;
        }
    }

    Ok(())
}

#[salsa::tracked]
impl<'db> Type<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_from_alloc_crate(self, db: &'db dyn Db) -> bool {
        self.hir(db)
            .strip_references()
            .as_adt()
            .is_some_and(|adt| Crate::new(db, adt.module(db).krate(db)).name(db) == "alloc")
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_is_clonable(self, db: &'db dyn Db) -> Option<bool> {
        let crate_ = self.origin(db).context(db).module(db).krate(db);

        is_clonable(db, &self.hir(db), crate_)
    }

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
            ));
        }

        if let Some(adt) = hir.as_adt().filter(|adt| match adt {
            Adt::Struct(hir) => !Struct::new(db, *hir).generic_params(db).is_empty(),
            Adt::Enum(hir) => !Enum::new(db, *hir).generic_params(db).is_empty(),
            Adt::Union(_) => false,
        }) {
            let args = adt_type_args(db, adt, &hir)
                .map(|(index, _)| {
                    GenArg::Type(Type::new(
                        db,
                        TypeOrigin::TypeArgument { type_: self, index },
                    ))
                })
                .collect::<Vec<_>>();
            return match adt {
                Adt::Struct(hir) => TypeKind::Struct(StructType::new(
                    db,
                    Struct::new(db, hir),
                    args,
                    self.origin(db),
                )),
                Adt::Enum(hir) => {
                    TypeKind::Enum(EnumType::new(db, Enum::new(db, hir), args, self.origin(db)))
                }
                Adt::Union(_) => unreachable!("unions are unsupported"),
            };
        }

        let crate_ = self.origin(db).context(db).module(db).krate(db);
        if let Some(builtin) = hir.as_builtin() {
            return lower_builtin(db, builtin, crate_)
                .expect("unsupported types are discarded during lowering");
        }

        let adt = hir
            .as_adt()
            .expect("unsupported types are discarded during lowering");

        let args = adt_type_args(db, adt, &hir)
            .map(|(index, _)| {
                GenArg::Type(Type::new(
                    db,
                    TypeOrigin::TypeArgument { type_: self, index },
                ))
            })
            .collect::<Vec<_>>();

        match adt {
            Adt::Struct(_) if is_string(db, &hir, crate_) => TypeKind::String,
            Adt::Struct(struct_) => TypeKind::Struct(StructType::new(
                db,
                Struct::new(db, struct_),
                args,
                self.origin(db),
            )),
            Adt::Enum(enum_) => TypeKind::Enum(EnumType::new(
                db,
                Enum::new(db, enum_),
                args,
                self.origin(db),
            )),
            Adt::Union(_) => unreachable!("unions are unsupported"),
        }
    }
}

impl<'db> TypeOrigin<'db> {
    pub(super) fn context(self, db: &'db dyn Db) -> GenericDef {
        match self {
            Self::Field(field) => GenericDef::Adt(field.hir(db).parent_def(db).adt(db)),
            Self::QualifiedField(field) => field.target(db).origin(db).context(db),
            Self::CallableParameter { callable, .. }
            | Self::CallableReturn(callable)
            | Self::CallableReceiver(callable) => callable.context(db),
            Self::TypeArgument { type_, .. } => type_.origin(db).context(db),
            Self::FunctionParameter { function, .. }
            | Self::FunctionReturn(function)
            | Self::FunctionReceiver(function) => GenericDef::Function(function.hir(db)),
            Self::StructDeclaration(struct_) => GenericDef::Adt(Adt::Struct(struct_.hir(db))),
            Self::EnumDeclaration(enum_) => GenericDef::Adt(Adt::Enum(enum_.hir(db))),
            Self::Struct(type_) => type_.origin(db).context(db),
            Self::Enum(type_) => type_.origin(db).context(db),
            Self::TypeParameter(param) => param.hir(db).parent(db),
            Self::Borrowed(reference) => reference.inner(db).origin(db).context(db),
            Self::Dereferenced(type_) => type_.origin(db).context(db),
        }
    }
}

fn adt_type_args<'a, 'db: 'a>(
    db: &'db dyn Db,
    adt: Adt,
    type_: &'a HirType<'db>,
) -> impl Iterator<Item = (usize, HirType<'db>)> + 'a {
    let params = GenericDef::Adt(adt)
        .params(db)
        .into_iter()
        .filter_map(|param| match param {
            HirGenericParam::TypeParam(param) => Some(param),
            _ => None,
        });

    type_
        .type_arguments()
        .enumerate()
        .zip(params)
        .filter_map(move |((index, arg), param)| {
            (!param.is_unstable(db) || param.default(db).is_none()).then_some((index, arg))
        })
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
    #[error("unstable items are not supported")]
    Unstable,

    #[error("unsupported type: {type_}")]
    #[diagnostic(code(rust_curse::unsupported_type), severity(Warning))]
    Type { type_: String },

    #[error("the type \"{type_}\" contains borrowed values")]
    #[diagnostic(
        code(rust_curse::borrowed_type),
        help("use owned values inside the ADT"),
        severity(Warning)
    )]
    BorrowedValues { type_: String },

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
