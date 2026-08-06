use crate::{
    compiler::{
        bir::{BindingClass, BindingClassField, ExternType, ExternValueType, FieldAccess},
        codegen::{
            api::Api,
            bindings::{
                binding_fn::render_binding_fn,
                cast_graph::{BindingsCastCompiler, Cast},
                tethering::{
                    ClonedField, FieldExpr, TetheredField, TetheringStruct, TetheringStructField,
                    TetheringStructType, render_tethering_struct,
                    render_tethering_struct_get_field_expr, render_tethering_struct_set_field_stmt,
                },
            },
            cast::DirectCastRule,
            fragment::{FragmentOnce, Printer, doubles, fragment, spaced},
        },
        db::Db,
        ir::{ItemIdExt, Shape, Type},
    },
    frag,
};

pub fn render_binding_class<'db, CastCompiler>(
    class: BindingClass<'db>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
) -> impl FragmentOnce + 'db
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    BindingClassRenderer {
        class,
        cast_compiler,
        db,
    }
}

struct BindingClassRenderer<'db, CastCompiler> {
    class: BindingClass<'db>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

impl<'db, CastCompiler> FragmentOnce for BindingClassRenderer<'db, CastCompiler>
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> BindingClassRenderer<'db, CastCompiler>
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    pub fn render<'a>(&'a self) -> impl FragmentOnce + 'a {
        frag!(
            {self.render_tethering_struct()}@@
            {self.render_struct()}@@
            {self.render_impl()}
        )
    }

    fn render_tethering_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let tethered_fields = self
            .class
            .fields
            .iter()
            .map(|field| field.get_tethered_field(self.db));

        let tethering_struct = TetheringStruct {
            source_type: self.class.source_type,
            fields: tethered_fields,
        };
        render_tethering_struct(self.db, tethering_struct)
    }

    fn render_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);
        let name = self.class.external.symbol(self.db).value(self.db);
        let tethering_type_name = self.class.source_type.tethering_struct_name(self.db);

        frag!(
            {api.header_renamed(self.db, name)}
            pub struct ~{name}~{{
                src: std::rc::Rc<{tethering_type_name}>
            }}
        )
    }

    fn render_impl<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);

        let fields = spaced(self.class.fields.iter().map(move |field| {
            let field_renderer = BindingClassFieldRenderer {
                field: field.clone(),
                cast_compiler: self.cast_compiler.clone(),
                db: self.db,
            };

            fragment(move |p| {
                frag!({field_renderer.render_getter()}@).apply(p);

                if !field.external.is_readonly {
                    frag!(@{field_renderer.render_setter()}).apply(p);
                }
            })
        }));

        let fns =
            spaced(self.class.fns.iter().map(move |fn_| {
                render_binding_fn(self.db, fn_.clone(), self.cast_compiler.clone())
            }));

        let name = self.class.external.symbol(self.db).value(self.db);

        frag!(
            {api.impl_header(self.db)}@
            impl~{name}~{{
                {fields}
                {fns}
            }}
        )
    }
}

struct BindingClassFieldRenderer<'db, CastCompiler> {
    pub field: BindingClassField<'db>,
    pub cast_compiler: CastCompiler,
    pub db: &'db dyn Db,
}

impl<'db, CastCompiler> BindingClassFieldRenderer<'db, CastCompiler>
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    fn render_getter<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);

        let result_type = api.result_type(
            self.db,
            self.field.external.out_type.render_type_decl(self.db),
        );

        let field_tethering_value_expr = render_tethering_struct_get_field_expr(
            frag!(self.src),
            self.field.external.symbol.value(self.db),
        );

        let output = fragment(move |p| {
            let external_type_out = self.field.external.out_type;

            if external_type_out.uses_tethering() {
                self.cast_compiler
                    .compile_cast_expr(
                        self.db,
                        TetheringStructType {
                            source_type: self.field.source_type,
                        },
                        ExternType::Value(external_type_out),
                        field_tethering_value_expr,
                    )
                    .apply(p);
            } else {
                self.cast_compiler
                    .compile_cast_expr(
                        self.db,
                        self.field.source_type,
                        ExternType::Value(external_type_out),
                        field_tethering_value_expr,
                    )
                    .apply(p);
            }
        });

        let symbol = self.field.external.symbol.value(self.db);

        frag!(
            {api.field_getter_header(self.db, symbol)}@
            pub fn ~get_{symbol}(&self)~ -> ~{result_type}~ {{
                Ok({output})
            }}
        )
    }

    fn render_setter<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);

        let set_tethered_field_stmt = render_tethering_struct_set_field_stmt(
            frag!(self.src),
            self.field.external.symbol.value(self.db),
            "value",
        );

        let set_tethered_field_stmt = self.cast_compiler.compile_cast_stmt(
            self.db,
            Cast {
                from: self.field.external.in_type,
                to: ExternType::Value(self.field.external.out_type),
                in_expr: "value",
                out_var_name: "value",
            },
            set_tethered_field_stmt,
        );

        let symbol = self.field.external.symbol.value(self.db);
        let result_type = api.result_type(self.db, "()");
        let field_in_type = self.field.external.in_type.render_type_decl(self.db);

        frag!(
            {api.field_setter_header(self.db, symbol)}@
            pub fn ~set_{symbol}(&self, value: ~{field_in_type})~ -> ~{result_type}~ {{
                {set_tethered_field_stmt}
                Ok(())
            }}
        )
    }
}

impl<'db> BindingClassField<'db> {
    fn get_tethered_field(
        &self,
        db: &'db dyn Db,
    ) -> TetheredField<'db, BindingClassFieldExpr<'_, 'db>, BindingClassFieldExpr<'_, 'db>> {
        let ref_field_expr = BindingClassFieldExpr {
            field: self,
            db,
            is_mutable: false,
        };

        let mut_field_expr = BindingClassFieldExpr {
            field: self,
            db,
            is_mutable: true,
        };

        if self.external.out_type.uses_tethering() {
            TetheredField::TetheringStruct(TetheringStructField {
                name: self.external.symbol.value(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: self.source_type,
                with_setter: !self.external.is_readonly,
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: self.external.symbol.value(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: self.source_type,
            })
        }
    }
}

#[derive(Clone, Copy)]
struct BindingClassFieldExpr<'field, 'db> {
    field: &'field BindingClassField<'db>,
    db: &'db dyn Db,
    is_mutable: bool,
}

impl<'field, 'db: 'field> FieldExpr for BindingClassFieldExpr<'field, 'db> {
    fn render(&self, source: &str) -> impl FragmentOnce {
        let Self {
            field,
            db,
            is_mutable,
        } = *self;

        fragment(move |p| match &field.access {
            FieldAccess::StructAccess { field_name, .. } => {
                let field_name = field_name.as_str();

                if is_mutable {
                    frag!(__tethering::AccessResult::Ok(&mut {source}.{field_name})).apply(p);
                } else {
                    frag!(__tethering::AccessResult::Ok(&{source}.{field_name})).apply(p);
                }
            }
            FieldAccess::EnumVariantAccess {
                enum_,
                variant,
                field_name,
            } => {
                let enum_ = *enum_;
                let variant = *variant;
                let field_name = field_name.as_str();
                let reason = frag!(
                    {
                        doubles((
                            "cannot access inactive enum variant field `",
                            enum_.fqn(db),
                            "::",
                            variant.name(db),
                            "::",
                            field_name,
                            "`",
                        ))
                    }
                    .to_string()
                );
                let error = Api::get(db).error(db, reason);
                let enum_name = enum_.fqn(db);
                let variant_name = variant.name(db);

                match variant.shape(db) {
                    Shape::Unit => unreachable!("unit enum variants do not have fields"),
                    Shape::Tuple(fields) => {
                        let index = fields
                            .iter()
                            .position(|candidate| {
                                candidate
                                    .as_ref()
                                    .is_some_and(|candidate| candidate.name(db) == field_name)
                            })
                            .expect("enum field must belong to its variant");
                        let pattern = fragment(move |p| {
                            for _ in 0..index {
                                frag!(_,).apply(p);
                            }
                            frag!(__field, ..).apply(p);
                        });

                        frag!(
                            match ~{source}~{{
                                {enum_name}::{variant_name}({pattern}) => Ok(__field),@
                                _ => Err({error}),
                            }}
                        )
                        .apply(p);
                    }
                    Shape::Plain { .. } => {
                        frag!(
                            match ~{source}~{{
                                {enum_name}::{variant_name}~{{ {field_name}: __field, .. }} => Ok(__field),@
                                _ => Err({error}),
                            }}
                        )
                        .apply(p);
                    }
                }
            }
        })
    }
}

pub struct CastClassExternTypeToTetheringStructType;

impl<'db> DirectCastRule<'db, ExternType<'db>, TetheringStructType<'db>>
    for CastClassExternTypeToTetheringStructType
{
    fn predecessor(self, db: &'db dyn Db, to: TetheringStructType<'db>) -> Option<ExternType<'db>> {
        let extern_type = Type::Value(to.source_type).lower_extern_in(db);
        if matches!(extern_type.deref(), ExternValueType::Class(_)) {
            Some(extern_type)
        } else {
            None
        }
    }

    fn render<'fragment, Expr>(
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

pub struct CastTetheringStructTypeToClassExternType;

impl<'db> DirectCastRule<'db, TetheringStructType<'db>, ExternType<'db>>
    for CastTetheringStructTypeToClassExternType
{
    fn predecessor(self, db: &'db dyn Db, to: ExternType<'db>) -> Option<TetheringStructType<'db>> {
        // extern type -> source type
        if let ExternValueType::Class(sym) = to.deref() {
            Some(TetheringStructType {
                source_type:
            })
        } else {
            None
        }
    }

    fn render<'fragment, Expr>(
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
