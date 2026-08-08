use crate::{
    compiler::{
        codegen::{
            api::Api,
            cast::DirectCastRule,
            fragment::{FragmentOnce, Printer, doubles, fragment},
            shim::{
                cast_graph::ShimCastCompiler,
                tethering::{
                    ClonedField, FieldExpr, TetheredField, TetheringStruct, TetheringStructField,
                    TetheringStructType, render_tethering_struct,
                },
            },
        },
        db::Db,
        ir::{
            rust::{AsItem as _, FieldParent, Shape, ValueType},
            shim::{ShimClass, ShimField, ShimType, ShimValueType},
        },
    },
    frag,
};

impl<'db> ShimClass<'db> {
    pub fn render_with<CastCompiler>(
        self,
        db: &'db dyn Db,
        cast_compiler: CastCompiler,
    ) -> impl FragmentOnce
    where
        CastCompiler: ShimCastCompiler<'db> + Clone + 'db,
    {
        ClassFragment {
            class: self,
            cast_compiler,
            db,
        }
    }
}

struct ClassFragment<'db, CastCompiler> {
    class: ShimClass<'db>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

impl<'db, CastCompiler> FragmentOnce for ClassFragment<'db, CastCompiler>
where
    CastCompiler: ShimCastCompiler<'db> + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> ClassFragment<'db, CastCompiler>
where
    CastCompiler: ShimCastCompiler<'db> + Clone + 'db,
{
    fn render(&self) -> impl FragmentOnce {
        frag!(
            {self.render_tethering_struct()}@@
            {self.render_struct()}@@
        )
    }

    fn render_tethering_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let tethered_fields = self
            .class
            .fields(self.db)
            .iter()
            .map(|field| field.get_tethered_field(self.db));

        let tethering_struct = TetheringStruct {
            source_type: self.class.src(self.db).into(),
            fields: tethered_fields,
        };

        render_tethering_struct(self.db, tethering_struct)
    }

    fn render_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);

        let tethering_type_name =
            ValueType::from(self.class.src(self.db)).tethering_struct_name(self.db);

        let symbol = self.class.ffi(self.db).symbol(self.db);

        frag!(
            {api.header_renamed(self.db, symbol)}
            pub struct ~{symbol}~{{
                src: std::rc::Rc<{tethering_type_name}>
            }}
        )
    }
}

impl<'db> ShimField<'db> {
    fn get_tethered_field(
        &self,
        db: &'db dyn Db,
    ) -> TetheredField<'db, ShimFieldExpr<'_, 'db>, ShimFieldExpr<'_, 'db>> {
        let ref_field_expr = ShimFieldExpr {
            field: self,
            db,
            is_mutable: false,
        };
        let mut_field_expr = ShimFieldExpr {
            field: self,
            db,
            is_mutable: true,
        };

        let src_field = self.src(db);
        if src_field.type_(db).uses_tethering() {
            TetheredField::TetheringStruct(TetheringStructField {
                name: src_field.mangled_name(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: src_field.type_(db),
                with_setter: !src_field.is_readonly(db),
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: src_field.mangled_name(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: src_field.type_(db),
            })
        }
    }
}

#[derive(Clone, Copy)]
struct ShimFieldExpr<'field, 'db> {
    field: &'field ShimField<'db>,
    db: &'db dyn Db,
    is_mutable: bool,
}

impl<'field, 'db: 'field> FieldExpr for ShimFieldExpr<'field, 'db> {
    fn render(&self, source: &str) -> impl FragmentOnce {
        let Self {
            field,
            db,
            is_mutable,
        } = *self;

        let src_field = field.src(db);

        fragment(move |p| match &src_field.parent(db) {
            FieldParent::Struct(..) => {
                let field_name = src_field.name(db);

                if is_mutable {
                    frag!(__tethering::AccessResult::Ok(&mut {source}.{field_name})).apply(p);
                } else {
                    frag!(__tethering::AccessResult::Ok(&{source}.{field_name})).apply(p);
                }
            }
            FieldParent::Variant(variant) => {
                let enum_ = variant.parent(db);
                let field_name = src_field.name(db);
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

pub struct CastClassShimTypeToTetheringStructType;

impl<'db> DirectCastRule<'db, ShimType<'db>, TetheringStructType<'db>>
    for CastClassShimTypeToTetheringStructType
{
    fn predecessor(self, db: &'db dyn Db, to: TetheringStructType<'db>) -> Option<ShimType<'db>> {
        let shim_type = to.source_type.shim_in(db);
        if matches!(shim_type.deborrow(), ShimValueType::Class(..)) {
            Some(shim_type)
        } else {
            None
        }
    }

    fn render<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: ShimType<'db>,
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

pub struct CastTetheringStructTypeToClassShimType;

impl<'db> DirectCastRule<'db, TetheringStructType<'db>, ShimType<'db>>
    for CastTetheringStructTypeToClassShimType
{
    fn predecessor(self, db: &'db dyn Db, to: ShimType<'db>) -> Option<TetheringStructType<'db>> {
        if let ShimValueType::Class(_) = to.deborrow() {
            Some(TetheringStructType {
                source_type: to.src(db),
            })
        } else {
            None
        }
    }

    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: TetheringStructType<'db>,
        to: ShimType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let ShimValueType::Class(target) = to.deborrow() else {
            unreachable!();
        };

        frag!({target.symbol(db)}~{{ src: {in_expr} }})
    }
}
