//! Module for compiling structs into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        api::Api,
        bindings::{
            bir::{BirBoundaryType, BirSourceType},
            boundary::{
                BoundaryInExpr, BoundaryMode, compile_boundary_in_to_source_type_expr,
                compile_boundary_in_type, compile_boundary_out_from_source_type_expr,
                compile_boundary_out_from_tethered_struct_expr, compile_boundary_out_type,
            },
            tethering::{
                ClonedField, TetheredField, TetheredStructField, TetheringStruct,
                compile_tethered_struct, compile_tethered_struct_get_field_expr,
                compile_tethered_struct_mut_access_stmt, compile_tethered_struct_ref_access_stmt,
                compile_tethered_struct_set_field_stmt, compile_tethering_struct_from_source_expr,
            },
        },
        cast::{CastContinuation, DirectCastRule, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, fragment, spaced},
        ir::{
            item::{ItemId, ItemIdExt as _, ItemKind},
            shape::ShapeField,
            struct_::{Struct, StructId},
            type_::IrType,
        },
        rustdoc::mod_::{all_mod_ids, lower_mod_enums, lower_mod_structs},
    },
    frag,
};

impl<'db> StructId<'db> {
    /// Returns the paired tethering struct name.
    pub fn tethering_struct_name(self, db: &'db dyn Db) -> &'db String {
        tethering_struct_name(db, self.into())
    }
}

#[salsa::tracked(returns(ref))]
fn binding_name<'db>(db: &'db dyn Db, id: ItemId<'db>) -> String {
    format!("__binding_struct__{}", id.mangled_fqn(db))
}

#[salsa::tracked(returns(ref))]
fn tethering_struct_name<'db>(db: &'db dyn Db, id: ItemId<'db>) -> String {
    format!("__tethered_struct__{}", id.mangled_fqn(db))
}

impl<'db> Struct<'db> {
    /// Paired tethering struct name, e.g.
    /// `__tethered_struct__std__mem__Drop`.
    pub fn tethering_struct_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).tethering_struct_name(db)
    }
}

/// Compiles a Rust struct into its tethered backing struct and public binding
/// wrapper.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_struct(db: &dyn Db, struct_: Struct<'_>) -> String {
    let tethered_fields = struct_.fields(db).iter().map(|field| {
        let name = field.name(db);
        let type_ = field.type_(db);

        // todo: maybe move the __tethering::AccessResult::Ok() to the tethering.rs
        // itself, and use it here and in enums
        let get_ref_expr =
            move |source: &'static str| frag!(__tethering::AccessResult::Ok(&{source}.{name}));
        let get_mut_expr =
            move |source: &'static str| frag!(__tethering::AccessResult::Ok(&mut ~{source}.{name}));

        match type_ {
            IrType::Struct(id) => {
                let tethered_name = id.tethering_struct_name(db);

                TetheredField::TetheredStruct(TetheredStructField {
                    name,
                    get_ref_expr,
                    get_mut_expr,
                    source_type: type_.fqn(db),
                    tethered_name,
                    with_setter: !field.is_readonly(db),
                })
            }
            IrType::Enum(enum_id) => {
                let tethered_name = enum_id.tethering_struct_name(db);

                TetheredField::TetheredStruct(TetheredStructField {
                    name,
                    get_ref_expr,
                    get_mut_expr,
                    source_type: type_.fqn(db),
                    tethered_name,
                    with_setter: !field.is_readonly(db),
                })
            }
            IrType::Primitive(_) | IrType::Str | IrType::String => {
                TetheredField::Cloned(ClonedField {
                    name,
                    get_ref_expr,
                    get_mut_expr,
                    source_type: type_.fqn(db),
                })
            }
        }
    });

    frag!(
        {compile_tethered_struct(TetheringStruct {
            source_type: struct_.fqn(db),
            name: struct_.tethering_struct_name(db),
            fields: tethered_fields,
        })}@@
        {compile_wrapper_struct(db, struct_)}@@
        {compile_wrapper_impl(db, struct_)}
    )
    .print()
}

/// Returns whether compiling any lowered struct or enum requires the tethering
/// prelude.
#[salsa::tracked]
pub fn is_using_tethering(db: &dyn Db) -> bool {
    all_mod_ids(db).iter().any(|mod_id| {
        !lower_mod_structs(db, *mod_id).is_empty() || !lower_mod_enums(db, *mod_id).is_empty()
    })
}

fn compile_wrapper_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    frag!(
        {api.struct_header(db, struct_)}
        pub struct ~{struct_.extern_name(db)}~{{
            src: std::rc::Rc<{struct_.tethering_struct_name(db)}>
        }}
    )
}

fn compile_wrapper_impl<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    let fields = spaced(struct_.fields(db).iter().copied().map(move |field| {
        fragment(move |p| {
            frag!({compile_field_getter(db, field)}@).apply(p);

            if !field.is_readonly(db) {
                frag!(@{compile_field_setter(db, field)}).apply(p);
            }
        })
    }));

    frag!(
        {api.impl_header(db)}@
        impl~{struct_.extern_name(db)}~{{
            {fields}
        }}
    )
}

/// Compiles the public binding getter for a source struct field.
fn compile_field_getter<'db>(db: &'db dyn Db, field: ShapeField<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let getter_name = field.getter_binding_name_on_struct(db);
    let field_type = field.type_(db);
    let result_type = api.result_type(db, compile_boundary_out_type(db, field_type));

    let tethered_value = compile_tethered_struct_get_field_expr(frag!(self.src), field.name(db));
    let output = fragment(move |p| match field_type {
        IrType::Struct(_) | IrType::Enum(_) => {
            compile_boundary_out_from_tethered_struct_expr(db, tethered_value, field_type).apply(p);
        }
        IrType::Primitive(_) | IrType::Str | IrType::String => {
            compile_boundary_out_from_source_type_expr(db, tethered_value, field_type).apply(p);
        }
    });

    frag!(
        {api.field_getter_header(db, field.name(db))}@
        pub fn ~{getter_name}(&self)~ -> ~{result_type}~ {{
            Ok({output})
        }}
    )
}

/// Compiles the public binding setter for a source struct field.
fn compile_field_setter<'db>(db: &'db dyn Db, field: ShapeField<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let setter_name = field.setter_binding_name_on_struct(db);
    let param_type_name = compile_boundary_in_type(db, field.type_(db));
    let result_type = api.result_type(db, "()");
    let value = BoundaryInExpr {
        in_expr: "value",
        out_var_name: "value",
        type_: field.type_(db),
        // Struct fields cannot store references, so setters always consume a source value.
        mode: BoundaryMode::Move,
    };
    let set_tethered_field =
        compile_tethered_struct_set_field_stmt(frag!(self.src), field.name(db), "value");

    frag!(
        {api.field_setter_header(db, field.name(db))}@
        pub fn ~{setter_name}(&self, value: ~{param_type_name})~ -> ~{result_type}~ {{
            let value = ~{compile_boundary_in_to_source_type_expr(db, [value], "value")};@
            {set_tethered_field}
            Ok(())
        }}
    )
}

/// Compiles an expression that retrieves the Rc of the tethered struct from a
/// binding struct.
pub fn compile_binding_struct_get_tethered_expr<'a>(
    binding_struct_expr: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!({ binding_struct_expr }.src)
}

/// Compiles an expression that wraps a tethered struct in its binding struct.
pub fn compile_binding_struct_from_tethered_expr<'db>(
    db: &'db dyn Db,
    id: StructId<'db>,
    tethered_struct_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    let extern_name = id.extern_name(db);
    frag!({extern_name}~{{ src: {tethered_struct_expr} }})
}

/// Casts a generated struct boundary value to a borrowed source struct.
pub struct CastStructBoundaryTypeToBorrowedSourceType;

impl<'db> IndirectCastRule<'db, BirBoundaryType<'db>, BirSourceType<'db>>
    for CastStructBoundaryTypeToBorrowedSourceType
{
    fn predecessor(self, db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirBoundaryType<'db>> {
        match to {
            BirSourceType::Ref { inner, .. } => match *inner {
                BirSourceType::Item { target: id } if id.kind(db) == ItemKind::Struct => {
                    Some(BirBoundaryType::Item { target: id })
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        from: BirBoundaryType<'db>,
        to: BirSourceType<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let BirSourceType::Ref { is_mutable, inner } = to else {
            unreachable!()
        };
        let BirSourceType::Item {
            target: source_target,
        } = *inner
        else {
            unreachable!()
        };
        let BirBoundaryType::Item { target } = from else {
            unreachable!()
        };

        assert_eq!(
            target, source_target,
            "struct boundary cast source and target items must match"
        );

        let tethered = compile_binding_struct_get_tethered_expr(in_expr);
        let continue_with = continue_with.compile("value");
        fragment(move |printer| {
            if is_mutable {
                compile_tethered_struct_mut_access_stmt(tethered, "value", continue_with)
                    .apply(printer);
            } else {
                compile_tethered_struct_ref_access_stmt(tethered, "value", continue_with)
                    .apply(printer);
            }
        })
    }
}

/// Casts a generated struct boundary value to an owned source struct by cloning
/// it.
pub struct CastStructBoundaryTypeToOwnedSourceType;

impl<'db> IndirectCastRule<'db, BirBoundaryType<'db>, BirSourceType<'db>>
    for CastStructBoundaryTypeToOwnedSourceType
{
    fn predecessor(self, db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirBoundaryType<'db>> {
        match to {
            BirSourceType::Item { target } if target.kind(db) == ItemKind::Struct => {
                Some(BirBoundaryType::Ref(Box::new(BirBoundaryType::Item {
                    target,
                })))
            }
            _ => None,
        }
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        from: BirBoundaryType<'db>,
        to: BirSourceType<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let BirBoundaryType::Ref(target) = from else {
            unreachable!("struct boundary casts require a generated boundary type ref");
        };
        let BirBoundaryType::Item { target } = *target else {
            unreachable!("struct boundary casts require a generated boundary type ref");
        };
        let BirSourceType::Item {
            target: source_target,
        } = to
        else {
            unreachable!("owned struct boundary casts require an item target");
        };

        assert_eq!(
            target, source_target,
            "struct boundary cast source and target items must match"
        );

        let tethered = compile_binding_struct_get_tethered_expr(in_expr);
        compile_tethered_struct_ref_access_stmt(
            tethered,
            "value",
            continue_with.compile(frag!(value.clone())),
        )
    }
}

/// Casts an owned source struct to its generated boundary type.
pub struct CastStructSourceTypeToBoundaryType;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirBoundaryType<'db>>
    for CastStructSourceTypeToBoundaryType
{
    fn predecessor(self, db: &'db dyn Db, to: BirBoundaryType<'db>) -> Option<BirSourceType<'db>> {
        match to {
            BirBoundaryType::Item { target } => {
                StructId::new(db, target).map(|_| BirSourceType::Item { target })
            }
            _ => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        from: BirSourceType<'db>,
        to: BirBoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let BirSourceType::Item {
            target: source_target,
        } = from
        else {
            unreachable!("struct boundary casts require an owned item source");
        };
        let BirBoundaryType::Item { target } = to else {
            unreachable!("struct boundary casts require a generated boundary target");
        };

        let struct_id = StructId::new(db, target)
            .expect("struct boundary casts require a generated struct boundary type");

        assert_eq!(
            source_target, target,
            "struct boundary cast source and target items must match"
        );

        let tethered =
            compile_tethering_struct_from_source_expr(struct_id.tethering_struct_name(db), in_expr);
        compile_binding_struct_from_tethered_expr(db, struct_id, tethered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        bindings::cast_graph::test::Consume,
        db::DbImpl,
        ir::item::{ItemKind, ItemPath},
        test::assert_snap,
    };

    #[test]
    fn casts_struct_boundary_type_to_shared_source_ref() {
        assert_snap!({
            let db = DbImpl::default();
            let path = ItemPath::new(&db, vec!["crate".into(), "Item".into()]);
            let target = ItemId::new(&db, path, ItemKind::Struct);

            IndirectCastRule::compile(
                CastStructBoundaryTypeToBorrowedSourceType,
                &db,
                BirBoundaryType::Item { target },
                BirSourceType::Ref {
                    is_mutable: false,
                    inner: Box::new(BirSourceType::Item { target }),
                },
                "input",
                Consume,
            )
            .print()
        });
    }

    #[test]
    fn casts_struct_boundary_type_to_mutable_source_ref() {
        assert_snap!({
            let db = DbImpl::default();
            let path = ItemPath::new(&db, vec!["crate".into(), "Item".into()]);
            let target = ItemId::new(&db, path, ItemKind::Struct);

            IndirectCastRule::compile(
                CastStructBoundaryTypeToBorrowedSourceType,
                &db,
                BirBoundaryType::Item { target },
                BirSourceType::Ref {
                    is_mutable: true,
                    inner: Box::new(BirSourceType::Item { target }),
                },
                "input",
                Consume,
            )
            .print()
        });
    }

    #[test]
    fn casts_struct_boundary_type_to_owned_source_type() {
        assert_snap!({
            let db = DbImpl::default();
            let path = ItemPath::new(&db, vec!["crate".into(), "Item".into()]);
            let target = ItemId::new(&db, path, ItemKind::Struct);

            IndirectCastRule::compile(
                CastStructBoundaryTypeToOwnedSourceType,
                &db,
                BirBoundaryType::Item { target },
                BirSourceType::Item { target },
                "input",
                Consume,
            )
            .print()
        });
    }

    #[test]
    fn casts_owned_struct_source_type_to_boundary_type() {
        assert_snap!({
            let db = DbImpl::default();
            let path = ItemPath::new(&db, vec!["crate".into(), "Item".into()]);
            let target = ItemId::new(&db, path, ItemKind::Struct);

            DirectCastRule::compile(
                CastStructSourceTypeToBoundaryType,
                &db,
                BirSourceType::Item { target },
                BirBoundaryType::Item { target },
                "input",
            )
            .print()
        });
    }
}
