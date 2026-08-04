use crate::{
    compiler::{
        codegen::api::Api,
        codegen::bindings::{
            bir::ExternType,
            cast_graph::{BindingsCastCompiler, Cast},
            extern_fn::{ExternFn, ExternFnParam, compile_extern_fn},
            tethering::{
                ClonedField, TetheredField, TetheringStruct, TetheringStructField,
                TetheringStructType, compile_tethering_struct,
                compile_tethering_struct_get_field_expr, compile_tethering_struct_set_field_stmt,
            },
        },
        codegen::cast::DirectCastRule,
        codegen::fragment::{FragmentOnce, FragmentPrint as _, fragment, spaced},
        db::Db,
        ir::{IntoIrTypeExt as _, ItemId, Type, ValueType},
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
    pub extern_name: &'db str,
    pub type_: ValueType<'db>,
    pub is_readonly: bool,
    pub get_ref_expr: GetRefExpr,
    pub get_mut_expr: GetMutExpr,
}

impl<'db, GetRefExpr, GetMutExpr> ExternClassField<'db, GetRefExpr, GetMutExpr> {
    /// Converts this field to a [`TetheredField`] of the tethering struct.
    pub fn into_tethered_field(self) -> TetheredField<'db, GetRefExpr, GetMutExpr> {
        let source_type = Type::Value(self.type_);

        if self.type_.uses_tethering() {
            TetheredField::TetheringStruct(TetheringStructField {
                name: self.extern_name,
                get_ref_expr: self.get_ref_expr,
                get_mut_expr: self.get_mut_expr,
                source_type,
                with_setter: !self.is_readonly,
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: self.extern_name,
                get_ref_expr: self.get_ref_expr,
                get_mut_expr: self.get_mut_expr,
                source_type,
            })
        }
    }
}

pub fn compile_extern_class<'db, CastCompiler, Fields, GetRefExpr, GetMutExpr, RefExpr, MutExpr>(
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
    RefExpr: FragmentOnce + 'db,
    MutExpr: FragmentOnce + 'db,
{
    let fields = class.fields.into_iter();

    let tethered_fields = fields.clone().map(|field| field.into_tethered_field());

    let tethering_struct = TetheringStruct {
        source_type: Type::Value(class.source_item_id.into_ir_type(db)),
        fields: tethered_fields,
    };

    frag!(
        {compile_tethering_struct(db, tethering_struct)}@@
        {compile_extern_class_struct(db, class.source_item_id)}@@
        {compile_class_impl_for_fields(db, cast_compiler.clone(), class.source_item_id, fields)}
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
        let source_type = Type::Value(source_item_id.into_ir_type(db));
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

fn compile_class_impl_for_fields<'db, CastCompiler, Fields, GetRefExpr, GetMutExpr>(
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
    let result_type = api.result_type(db, field_type.extern_out().fqn_decl(db));

    let field_value = compile_tethering_struct_get_field_expr(frag!(self.src), field.extern_name);

    let output = fragment(move |p| {
        let field_source_type = Type::Value(field_type);
        if field_type.uses_tethering() {
            cast_compiler
                .compile_cast_expr(
                    db,
                    TetheringStructType {
                        source_type: field_source_type,
                    },
                    field_type.extern_out(),
                    field_value,
                )
                .apply(p);
        } else {
            cast_compiler
                .compile_cast_expr(db, field_source_type, field_type.extern_out(), field_value)
                .apply(p);
        }
    });

    frag!(
        {api.field_getter_header(db, field.extern_name)}@
        pub fn ~get_{field.extern_name}(&self)~ -> ~{result_type}~ {{
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
    let field_boundary_in_type = field.type_.extern_in();
    let field_source_type = Type::Value(field.type_);

    let set_tethered_field =
        compile_tethering_struct_set_field_stmt(frag!(self.src), field.extern_name, "value");

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
            {api.field_setter_header(db, field.extern_name)}@
            pub fn ~set_{field.extern_name}(&self, value: ~{field_boundary_in_type.fqn_decl(db)})~ -> ~{result_type}~ {{
                {set_tethered_field}
                Ok(())
            }}
        ).apply(p);
    })
}

// todo: somehow merge into one function
pub fn compile_extern_class_fns<'db, CastCompiler, Fns, SourceExpr, Params>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    source_item_id: ItemId<'db>,
    fns: Fns,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
    Fns: IntoIterator<Item = ExternFn<'db, Params, SourceExpr>> + 'db,
    SourceExpr: FragmentOnce + 'db,
    Params: IntoIterator<Item = ExternFnParam<'db>> + 'db,
    Params::IntoIter: Clone,
{
    let api = Api::get(db);
    let fns = spaced(
        fns.into_iter()
            .map(move |fn_| compile_extern_fn(db, cast_compiler.clone(), fn_)),
    );

    frag!(
        {api.impl_header(db)}@
        impl~{source_item_id.extern_name(db)}~{{
            {fns}
        }}
    )
}

pub struct CastClassBoundaryTypeToTetheringStructType;

impl<'db> DirectCastRule<'db, ExternType<'db>, TetheringStructType<'db>>
    for CastClassBoundaryTypeToTetheringStructType
{
    fn predecessor(
        self,
        _db: &'db dyn Db,
        to: TetheringStructType<'db>,
    ) -> Option<ExternType<'db>> {
        if let Type::Value(ir_type) = to.source_type {
            if ir_type.uses_class() {
                Some(ir_type.extern_in())
            } else {
                None
            }
        } else {
            None
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: ExternType<'db>,
        _to: TetheringStructType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({ in_expr }.src)
    }
}

pub struct CastTetheringStructTypeToClassBoundaryType;

impl<'db> DirectCastRule<'db, TetheringStructType<'db>, ExternType<'db>>
    for CastTetheringStructTypeToClassBoundaryType
{
    fn predecessor(self, db: &'db dyn Db, to: ExternType<'db>) -> Option<TetheringStructType<'db>> {
        match to {
            ExternType::Item { target } if target.into_ir_type(db).uses_class() => {
                Some(TetheringStructType {
                    source_type: Type::Value(target.into_ir_type(db)),
                })
            }
            _ => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: TetheringStructType<'db>,
        to: ExternType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let ExternType::Item { target } = to else {
            unreachable!();
        };

        frag!({target.extern_name(db)}~{{ src: {in_expr} }})
    }
}
