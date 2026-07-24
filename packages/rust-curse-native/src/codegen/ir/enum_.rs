use std::collections::BTreeMap;

use rustdoc_types::ItemKind;

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        mangle::{mangle_path, to_camel_case},
        shape::{Shape, ShapeField},
        type_::Type,
    },
    rustdoc::RustdocId,
};

#[salsa::interned(debug)]
#[derive(Ord, PartialOrd)]
pub struct EnumId<'db> {
    #[returns(copy)]
    pub inner: IrId<'db>,
}

#[salsa::tracked]
impl<'db> EnumId<'db> {
    /// Enum fully qualified name, e.g. `core::option::Option`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        let id = self.rustdoc_id(db);
        let summary = id
            .crate_body(db)
            .paths
            .get(&id.idx(db))
            .expect("Enum not found in a rustdoc crate");

        assert_eq!(summary.kind, ItemKind::Enum);
        summary.path.join("::")
    }

    /// Unqualified enum name, e.g. `Option`.
    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        let id = self.rustdoc_id(db);
        let summary = id
            .crate_body(db)
            .paths
            .get(&id.idx(db))
            .expect("Enum not found in a rustdoc crate");

        assert_eq!(summary.kind, ItemKind::Enum);
        summary
            .path
            .last()
            .expect("Enum must have a name")
            .to_string()
    }

    /// Mangled fully qualified name, e.g. `core__option__Option`.
    #[salsa::tracked(returns(ref))]
    pub fn mangled_fqn(self, db: &'db dyn Db) -> String {
        let id = self.rustdoc_id(db);
        let summary = id
            .crate_body(db)
            .paths
            .get(&id.idx(db))
            .expect("Enum not found in a rustdoc crate");

        assert_eq!(summary.kind, ItemKind::Enum);
        mangle_path(&summary.path)
    }

    /// Binding mangled name, e.g. `__binding_enum__core__option__Option`.
    #[salsa::tracked(returns(ref))]
    pub fn binding_name(self, db: &'db dyn Db) -> String {
        format!("__binding_enum__{}", self.mangled_fqn(db))
    }

    /// Paired tethering struct name, e.g.
    /// `__tethered_struct__core__option__Option`.
    #[salsa::tracked(returns(ref))]
    pub fn tethered_name(self, db: &'db dyn Db) -> String {
        format!("__tethered_struct__{}", self.mangled_fqn(db))
    }

    /// Returns the rustdoc ID for this enum.
    pub fn rustdoc_id(self, db: &'db dyn Db) -> RustdocId<'db> {
        self.inner(db).rustdoc_id(db)
    }

    /// Returns the path to this enum.
    pub fn path(self, db: &'db dyn Db) -> &'db [String] {
        self.inner(db).path(db)
    }

    /// Returns the parent module ID of this item, or `None` if the item
    /// is a top-level module.
    pub fn parent_mod_id(self, db: &'db dyn Db) -> Option<IrId<'db>> {
        self.inner(db).parent_mod_id(db)
    }
}

/// A lowered enum variant and its field layout.
#[salsa::tracked(debug)]
pub struct EnumVariant<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    #[returns(ref)]
    pub shape: Shape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

impl<'db> EnumVariant<'db> {
    /// Iterates over this variant's public lowered fields in declaration order.
    pub fn fields(
        self,
        db: &'db dyn crate::codegen::db::Db,
    ) -> impl Iterator<Item = ShapeField<'db>> + Clone + 'db {
        self.shape(db).fields()
    }
}

/// A lowered Rust enum and its variants.
#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub id: EnumId<'db>,
    #[tracked]
    #[returns(ref)]
    pub variants: Vec<EnumVariant<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> Enum<'db> {
    /// Returns JavaScript discriminant tags in variant declaration order.
    #[salsa::tracked(returns(ref))]
    pub fn js_tag_names(self, db: &'db dyn Db) -> Vec<String> {
        let camel_case_names = self
            .variants(db)
            .iter()
            .map(|variant| to_camel_case(variant.name(db)))
            .collect::<Vec<_>>();
        let mut occurrences = BTreeMap::new();

        for name in &camel_case_names {
            *occurrences.entry(name.as_str()).or_insert(0) += 1;
        }

        // declash the `http` variant and `Http` variant by preserving
        // their original names
        self.variants(db)
            .iter()
            .zip(&camel_case_names)
            .map(|(variant, camel_case_name)| {
                if occurrences[camel_case_name.as_str()] > 1 {
                    variant.name(db).clone()
                } else {
                    camel_case_name.clone()
                }
            })
            .collect()
    }

    /// Returns the JavaScript discriminant field name for this enum.
    pub fn js_tag_field_name(&self, db: &'db dyn Db) -> &'static str {
        if self
            .variants(db)
            .iter()
            .any(|variant| variant.fields(db).any(|field| field.name(db) == "type"))
        {
            "$type"
        } else {
            "type"
        }
    }

    /// Returns the name of the binding struct for this enum.
    pub fn binding_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).binding_name(db)
    }

    /// Returns the name of the tethered backing struct for this enum.
    pub fn tethered_name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).tethered_name(db)
    }

    /// Returns the enum name, e.g. `Shape` for `geometry::Shape`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).name(db)
    }

    /// Returns the fully qualified enum name, e.g. `geometry::Shape`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).fqn(db)
    }

    /// Returns the enum as a [`Type`].
    pub fn as_type(&self, db: &'db dyn Db) -> Type<'db> {
        Type::Enum(self.id(db))
    }
}

// todo: move to variant?
#[salsa::tracked]
impl<'db> ShapeField<'db> {
    /// Returns the enum variant field binding getter function name, e.g.
    /// `__getter__Circle__radius`.
    #[salsa::tracked(returns(ref))]
    pub fn getter_binding_name_on_enum(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__getter{}", self.enum_binding_name(db, variant))
    }

    /// Returns the enum variant field binding setter function name, e.g.
    /// `__setter__Circle__radius`.
    #[salsa::tracked(returns(ref))]
    pub fn setter_binding_name_on_enum(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__setter{}", self.enum_binding_name(db, variant))
    }

    /// Binding mangled name for a field, e.g. `__Circle__radius`.
    #[salsa::tracked(returns(ref))]
    pub fn enum_binding_name(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__{}__{}", variant.name(db), self.name(db))
    }
}
