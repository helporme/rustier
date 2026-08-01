use crate::{
    codegen::{
        api::Api,
        bindings::{
            bir::BirSourceType,
            cast_graph::{BindingsCast, BindingsCastCompiler, Cast},
            tethering::{
                ClonedField, TetheredField, TetheringStruct, TetheringStructField,
                TetheringStructType, compile_tethering_struct,
                compile_tethering_struct_get_field_expr, compile_tethering_struct_set_field_stmt,
            },
        },
        db::Db,
        fragment::{FragmentOnce, FragmentPrint as _, fragment, spaced},
        ir::{
            item::ItemId,
            type_::{IrType, ItemIdExt as _},
        },
    },
    frag,
};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ExternClass<'db, Fields> {
    pub source_item_id: ItemId<'db>,
    pub fields: Fields,
}

#[derive(Clone, Debug)]
pub struct ExternClassField<'db, GetRefExpr, GetMutExpr> {
    pub name: &'db str,
    pub type_: IrType<'db>,
    pub is_readonly: bool,
    pub get_ref_expr: GetRefExpr,
    pub get_mut_expr: GetMutExpr,
}

impl<'db, GetRefExpr, GetMutExpr> ExternClassField<'db, GetRefExpr, GetMutExpr> {
    /// Converts this field to a [`TetheredField`] of the tethering struct.
    pub fn into_tethered_field(
        self,
        db: &'db dyn Db,
    ) -> TetheredField<'db, GetRefExpr, GetMutExpr> {
        let source_type = self.type_.source_type_owned();

        if source_type.is_tethered(db) {
            TetheredField::TetheringStruct(TetheringStructField {
                name: self.name,
                get_ref_expr: self.get_ref_expr,
                get_mut_expr: self.get_mut_expr,
                source_type,
                with_setter: !self.is_readonly,
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: self.name,
                get_ref_expr: self.get_ref_expr,
                get_mut_expr: self.get_mut_expr,
                source_type,
            })
        }
    }
}

pub fn compile_extern_class<'db, Fields, GetRefExpr, GetMutExpr, RefExpr, MutExpr>(
    db: &'db dyn Db,
    class: ExternClass<'db, Fields>,
) -> String
where
    Fields: IntoIterator<Item = ExternClassField<'db, GetRefExpr, GetMutExpr>> + 'db,
    Fields::IntoIter: Clone,
    GetRefExpr: Fn(&'static str) -> RefExpr + Clone + 'db,
    GetMutExpr: Fn(&'static str) -> MutExpr + Clone + 'db,
    RefExpr: FragmentOnce,
    MutExpr: FragmentOnce,
{
    compile_extern_class_with(db, BindingsCast::Graph, class)
}

pub fn compile_extern_class_with<
    'db,
    CastCompiler,
    Fields,
    GetRefExpr,
    GetMutExpr,
    RefExpr,
    MutExpr,
>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    class: ExternClass<'db, Fields>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
    Fields: IntoIterator<Item = ExternClassField<'db, GetRefExpr, GetMutExpr>> + 'db,
    Fields::IntoIter: Clone,
    GetRefExpr: Fn(&'static str) -> RefExpr + Clone + 'db,
    GetMutExpr: Fn(&'static str) -> MutExpr + Clone + 'db,
    RefExpr: FragmentOnce,
    MutExpr: FragmentOnce,
{
    let fields = class.fields.into_iter();

    let tethered_fields = fields.clone().map(|field| field.into_tethered_field(db));

    let tethering_struct = TetheringStruct {
        source_type: BirSourceType::Item {
            target: class.source_item_id,
        },
        fields: tethered_fields,
    };

    frag!(
        {compile_tethering_struct(db, tethering_struct)}@@
        {compile_extern_class_struct(db, class.source_item_id)}@@
        {compile_class_impl(db, cast_compiler, class.source_item_id, fields)}
    )
    .print()
}

fn compile_extern_class_struct<'db>(
    db: &'db dyn Db,
    source_item_id: ItemId<'db>,
) -> impl FragmentOnce + 'db {
    let api = Api::get(db);
    let extern_name = source_item_id.extern_name(db);

    fragment(move |p| {
        let header = api.header_renamed(db, extern_name);
        let source_type = source_item_id.into_ir_type(db).source_type_owned();
        let tethering_type_name = source_type.tethering_struct_name(db);

        frag!(
            {header}
            pub struct ~{extern_name}~{{
                src: std::rc::Rc<{tethering_type_name}>
            }}
        )
        .apply(p);
    })
}

fn compile_class_impl<'db, CastCompiler, Fields, GetRefExpr, GetMutExpr>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    source_item_id: ItemId<'db>,
    class_fields: Fields,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
    Fields: Iterator<Item = ExternClassField<'db, GetRefExpr, GetMutExpr>> + 'db,
    GetRefExpr: Clone + 'db,
    GetMutExpr: Clone + 'db,
{
    let api = Api::get(db);

    let fields = spaced(class_fields.map(move |field| {
        let cast_compiler = cast_compiler.clone();

        fragment(move |p| {
            frag!({compile_field_getter(db, cast_compiler.clone(), field.clone())}@).apply(p);

            if !field.is_readonly {
                frag!(@{compile_field_setter(db, cast_compiler, field)}).apply(p);
            }
        })
    }));

    frag!(
        {api.impl_header(db)}@
        impl~{source_item_id.extern_name(db)}~{{
            {fields}
        }}
    )
}

fn compile_field_getter<'db, CastCompiler, GetRefExpr, GetMutExpr>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    field: ExternClassField<'db, GetRefExpr, GetMutExpr>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + 'db,
{
    let api = Api::get(db);
    let field_type = field.type_;
    let result_type = api.result_type(db, field_type.boundary_out().fqn_decl(db));

    let field_value = compile_tethering_struct_get_field_expr(frag!(self.src), field.name);

    let output = fragment(move |p| {
        let field_source_type = field_type.source_type_owned();
        if field_source_type.is_tethered(db) {
            cast_compiler
                .compile_cast_expr(
                    db,
                    TetheringStructType {
                        source_type: field_source_type,
                    },
                    field_type.boundary_out(),
                    field_value,
                )
                .apply(p);
        } else {
            cast_compiler
                .compile_cast_expr(
                    db,
                    field_source_type,
                    field_type.boundary_out(),
                    field_value,
                )
                .apply(p);
        }
    });

    frag!(
        {api.field_getter_header(db, field.name)}@
        pub fn ~get_{field.name}(&self)~ -> ~{result_type}~ {{
            Ok({output})
        }}
    )
}

fn compile_field_setter<'db, CastCompiler, GetRefExpr, GetMutExpr>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    field: ExternClassField<'db, GetRefExpr, GetMutExpr>,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + 'db,
{
    let api = Api::get(db);

    let result_type = api.result_type(db, "()");
    let field_boundary_in_type = field.type_.boundary_in();
    let field_source_type = field.type_.source_type_owned();

    let set_tethered_field =
        compile_tethering_struct_set_field_stmt(frag!(self.src), field.name, "value");

    fragment(move |p| {
        let set_tethered_field = cast_compiler.compile_cast_stmt(
            db,
            Cast {
                from: field_boundary_in_type.clone(),
                to: field_source_type,
                in_expr: "value",
                out_var_name: "value",
            },
            set_tethered_field,
        );

        frag!(
            {api.field_setter_header(db, field.name)}@
            pub fn ~set_{field.name}(&self, value: ~{field_boundary_in_type.fqn_decl(db)})~ -> ~{result_type}~ {{
                {set_tethered_field}
                Ok(())
            }}
        ).apply(p);
    })
}
