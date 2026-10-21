use crate::compiler::{
    codegen::fragment::{
        Fragment, FragmentOnce, FragmentPrint, Printer, doubles, frag, fragment, spaced,
    },
    db::Db,
    ir::{
        runtime::tethering::{RuntimeTetheredField, RuntimeTetheredFieldKind, RuntimeTethering},
        rust::{FieldParent, TypeKind},
    },
};

#[salsa::tracked]
impl<'db> RuntimeTethering<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn render(self, db: &'db dyn Db) -> String {
        TetheringFragment {
            tethering: self,
            db,
        }
        .print()
    }

    pub fn tethering_struct_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        frag!(__tethering__{self.source(db).src_type(db).render_mangled_name(db)})
    }
}

struct TetheringFragment<'db> {
    tethering: RuntimeTethering<'db>,
    db: &'db dyn Db,
}

impl<'db> FragmentOnce for TetheringFragment<'db> {
    fn apply(self, p: &mut Printer) {
        let db = self.db;
        let tethering = self.tethering;

        let fields = tethering.fields(db).iter().copied();

        let cache_field_definitions = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let RuntimeTetheredFieldKind::Tethered(tethering) = field.kind(db) {
                        let cache_field_name =
                            cache_field_name(field.src(db).declaration(db).mangled_name(db));

                        frag!(
                            {&cache_field_name}: std::cell::RefCell<Option<std::rc::Weak<{tethering.tethering_struct_name(db)}>>>,@
                        )
                        .apply(p);
                    }
                }
            })
        };

        let owned_cache_initializers = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let RuntimeTetheredFieldKind::Tethered(_) = field.kind(db) {
                        let cache_field_name =
                            cache_field_name(field.src(db).declaration(db).mangled_name(db));
                        frag!(
                            {&cache_field_name}: std::cell::RefCell::new(None),@
                        )
                        .apply(p);
                    }
                }
            })
        };

        let projected_cache_initializers = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let RuntimeTetheredFieldKind::Tethered(_) = field.kind(db) {
                        let cache_field_name =
                            cache_field_name(field.src(db).declaration(db).mangled_name(db));
                        frag!(
                            {&cache_field_name}: std::cell::RefCell::new(None),@
                        )
                        .apply(p);
                    }
                }
            })
        };

        let field_methods = spaced(fields.map(|field| {
            fragment(move |p| {
                match field.kind(db) {
                    RuntimeTetheredFieldKind::Cloned => field.render_cloned_getter(db).apply(p),
                    RuntimeTetheredFieldKind::Tethered(tethering) => {
                        field.render_projected_getter(db, tethering).apply(p);
                    }
                }

                if !field.is_readonly(db) {
                    match field.kind(db) {
                        RuntimeTetheredFieldKind::Cloned => {
                            frag!(@@{field.render_cloned_setter(db)}).apply(p);
                        }
                        RuntimeTetheredFieldKind::Tethered(_) => {
                            frag!(@@{field.render_projected_setter(db)}).apply(p);
                        }
                    }
                }
            })
        }));

        let name = || tethering.tethering_struct_name(db);
        let source_type = || tethering.source(db).src_type(db).render_fqn(db);

        frag!(
            struct ~{name()}~{{
                src: __tethering::SourceSlot<{source_type()}>,@
                {cache_field_definitions}
            }}@@

            impl __tethering::Accessor<{source_type()}> for ~{name()}~{{
                fn access_dyn(
                    &self,@
                    f: &mut dyn FnMut(&{source_type()}) -> __tethering::AccessResult<()>,@
                ) -> __tethering::AccessResult<()>~{{
                    self.src.access_dyn(f)
                }}@@

                fn access_mut_dyn(
                    &self,@
                    f: &mut dyn FnMut(&mut ~{source_type()}) -> __tethering::AccessResult<()>,@
                ) -> __tethering::AccessResult<()>~{{
                    self.src.access_mut_dyn(f)
                }}
            }}@@

            impl ~{name()}~{{
                pub fn new_owned(input: ~{source_type()})~ -> Self ~{{
                    {name()}~{{
                        src: __tethering::SourceSlot::owned(input),@
                        {owned_cache_initializers}
                    }}
                }}@@

                pub fn new_projected(accessor: std::rc::Rc<dyn __tethering::Accessor<{source_type()}>>) -> Self~{{
                    {name()}~{{
                        src: __tethering::SourceSlot::projected(accessor),@
                        {projected_cache_initializers}
                    }}
                }}@@

                {field_methods}
            }}
        )
        .apply(p);
    }
}

fn cache_field_name(name: &str) -> impl Fragment + '_ {
    frag!(__c__ { name })
}

impl<'db> RuntimeTetheredField<'db> {
    pub fn render_get_expr<'a>(
        self,
        db: &'db dyn Db,
        tethered_struct_expr: impl FragmentOnce + 'a,
    ) -> impl FragmentOnce + 'a
    where
        'db: 'a,
    {
        let name = self.src(db).declaration(db).mangled_name(db);

        frag!({tethered_struct_expr}.get_{name}()?)
    }

    pub fn render_set_stmt<'a>(
        self,
        db: &'db dyn Db,
        tethered_struct_expr: impl FragmentOnce + 'a,
        value: impl FragmentOnce + 'a,
    ) -> impl FragmentOnce + 'a
    where
        'db: 'a,
    {
        let name = self.src(db).declaration(db).mangled_name(db);

        frag!({tethered_struct_expr}.set_{name}({value})?;)
    }

    fn render_cloned_getter(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let get_ref_expr = self.render_access(db, "src", false);
            let name = self.src(db).declaration(db).mangled_name(db);
            let source_type = self.src(db)._type(db);

            frag!(
                pub fn get_{name}(&self) -> __tethering::AccessResult<{source_type.render_fqn(db)}>~{{
                    self.access(|src| Ok({get_ref_expr}?.clone()))
                }}
            )
            .apply(p);
        })
    }

    fn render_cloned_setter(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let get_mut_expr = self.render_access(db, "src", true);
            let name = self.src(db).declaration(db).mangled_name(db);
            let source_type = self.src(db)._type(db);

            frag!(
                pub fn set_{name}(&self, value: ~{source_type.render_fqn(db)}) -> __tethering::AccessResult<()>~{{
                    self.access_mut(|src|~{{
                        *{get_mut_expr}? = value;@
                        Ok(())
                    }})
                }}
            )
            .apply(p);
        })
    }

    fn render_projected_getter(
        self,
        db: &'db dyn Db,
        tethering: RuntimeTethering<'db>,
    ) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let name = self.src(db).declaration(db).mangled_name(db);
            let cache_field_name = cache_field_name(name);
            let validate_get_ref_expr = self.render_access(db, "parent", false);
            let get_ref_expr = self.render_access(db, "parent", false);
            let get_mut_expr = self.render_access(db, "parent", true);

            let tethered_name = || tethering.tethering_struct_name(db);
            let source_type = || self.src(db)._type(db).render_fqn(db);

            frag!(
                pub fn get_{name}(
                    self: &std::rc::Rc<Self>,@
                ) -> __tethering::AccessResult<std::rc::Rc<{tethered_name()}>>~{{
                    self.access(|parent|~{{
                        {validate_get_ref_expr}?;@
                        Ok(())
                    }})?;@@

                    if let Some(tethered)~ = __tethering::try_borrow(&self.{&cache_field_name})?.as_ref().and_then(std::rc::Weak::upgrade)~{{
                        return Ok(tethered);@
                    }}@@

                    let field_access: std::rc::Rc<dyn __tethering::Accessor<{source_type()}>> = std::rc::Rc::new(__tethering::FieldAccess~{{
                        parent: self.clone(),@
                        get: |parent|~{get_ref_expr},@
                        get_mut: |parent|~{get_mut_expr},@
                    }});@@

                    let tethered = std::rc::Rc::new({tethered_name()}::new_projected(field_access));@@

                    *__tethering::try_borrow_mut(&self.{&cache_field_name})? = Some(std::rc::Rc::downgrade(&tethered));@@

                    Ok(tethered)
                }}
            )
            .apply(p);
        })
    }

    fn render_projected_setter(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let name = self.src(db).declaration(db).mangled_name(db);
            let cache_field_name = cache_field_name(name);
            let validate_get_mut_expr = self.render_access(db, "src", true);
            let get_mut_expr = self.render_access(db, "src", true);
            let source_type = self.src(db)._type(db).render_fqn(db);

            frag!(
                pub fn set_{name}(&self, value: ~{source_type}) -> __tethering::AccessResult<()>~{{
                    self.access_mut(|src|~{{
                        {validate_get_mut_expr}?;@
                        Ok(())
                    }})?;@@

                    let cached_value = __tethering::try_borrow_mut(&self.{&cache_field_name})?
                        .take()
                        .and_then(|tethered| tethered.upgrade());@@

                    let old_value = self.access_mut(|src|~{{
                        Ok(std::mem::replace(
                            {get_mut_expr}?,
                            value,
                        ))
                    }})?;@@

                    if let Some(cached_value)~= cached_value~{{
                        cached_value.src.replace_with_owned(old_value)?;
                    }}@
                    Ok(())
                }}
            )
            .apply(p);
        })
    }

    fn render_access<'a>(
        self,
        db: &'db dyn Db,
        source: &'a str,
        is_mutable: bool,
    ) -> impl FragmentOnce + 'a
    where
        'db: 'a,
    {
        let src_field = self.src(db);

        fragment(move |p| match &src_field.declaration(db).parent(db) {
            FieldParent::Struct(..) => {
                let field_name = src_field.declaration(db).escaped_name(db);

                if is_mutable {
                    frag!(__tethering::AccessResult::Ok(&mut ~{source}.{field_name})).apply(p);
                } else {
                    frag!(__tethering::AccessResult::Ok(&{source}.{field_name})).apply(p);
                }
            }
            FieldParent::Variant(variant) => {
                let enum_ = src_field.target(db);
                let field_name = src_field.declaration(db).name(db);
                let error = frag!(
                    rustier_ext::runtime::errors::EnumError::InactiveField~{{
                        type_name: {doubles(enum_.display(db))},@
                        variant: {doubles(variant.name(db))},@
                        field: {doubles(field_name)},
                    }}.into()
                );

                let TypeKind::Enum(enum_type) = enum_.kind(db) else {
                    unreachable!("variant fields belong to enums")
                };
                let enum_name = enum_type.path(db).fqn(db);
                let variant_name = variant.name(db);
                let field_name = src_field.declaration(db).escaped_name(db);

                frag!(
                    match ~{source}~{{
                        {enum_name}::{variant_name}~{{ {field_name}: __field, .. }} => __tethering::AccessResult::Ok(__field),@
                        _ => __tethering::AccessResult::Err({error}),
                    }}
                )
                .apply(p);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        compiler::ir::rust::{Member, Rust, Type},
    };

    fn test_type<'db>(db: &'db dyn Db, name: &str) -> Type<'db> {
        let crate_ = Rust::get(db)
            .crates(db)
            .first()
            .expect("test crate should be lowered");

        crate_
            .root(db)
            .members(db)
            .iter()
            .find_map(|member| match member {
                Member::Struct(struct_) if struct_.segment(db).name(db) == name => {
                    struct_.as_mono_type(db)
                }
                Member::Enum(enum_) if enum_.segment(db).name(db) == name => enum_.as_mono_type(db),
                _ => None,
            })
            .unwrap_or_else(|| panic!("test value type `{name}` should be a root member"))
    }

    #[test]
    fn compiles_tethered_struct_empty() {
        assert_rust_snap! {
            source: {
                pub struct Empty;
            },
            expression: |db, _transport, p| {
                let db: &dyn Db = db;

                test_type(db, "Empty").tethering(db).unwrap().render(db).apply(p);
            },
        }
    }

    #[test]
    fn compiles_tethered_struct_cloned_field() {
        assert_rust_snap! {
            source: {
                pub struct Parent {
                    pub count: u32,
                }
            },
            expression: |db, _transport, p| {
                let db: &dyn Db = db;

                test_type(db, "Parent").tethering(db).unwrap().render(db).apply(p);
            },
        }
    }

    #[test]
    fn compiles_tethered_struct_tethered_struct_field() {
        assert_rust_snap! {
            source: {
                #[derive(Clone)]
                pub struct Child;
                pub struct Parent {
                    pub child: Child,
                }
            },
            expression: |db, _transport, p| {
                let db: &dyn Db = db;

                test_type(db, "Parent").tethering(db).unwrap().render(db).apply(p);
            },
        }
    }

    #[test]
    fn compiles_tethered_struct_mixed() {
        assert_rust_snap! {
            source: {
                #[derive(Clone)]
                pub struct Child;
                pub struct ReadonlyChild;
                pub struct Parent {
                    pub count: u32,
                    pub child: Child,
                    pub readonly_child: ReadonlyChild,
                }
            },
            expression: |db, _transport, p| {
                let db: &dyn Db = db;

                test_type(db, "Parent").tethering(db).unwrap().render(db).apply(p);
            },
        }
    }
}
