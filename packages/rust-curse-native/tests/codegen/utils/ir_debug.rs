use rust_curse::codegen::{
    db::Db,
    ir::{
        fn_::{Fn, FnParam},
        mod_::Mod,
        struct_::{Struct, StructField, StructShape, struct_fqn_from_ir_id},
        type_::{Primitive, Type},
    },
    rustdoc::mod_::{lower_mod_fns, lower_mod_structs, lower_mod_submods},
};

/// Renders the item as a debug human-readable string.
///
/// Direct alternative to Debug.
// todo: put a #[cfg(debug)] or smth like that on it?
pub trait DebugRender<'db> {
    /// Returns the human-readable debug string representation of the item.
    fn render(&self, db: &'db dyn Db) -> String;
}

impl<'db> DebugRender<'db> for Mod<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        let docs = self
            .docs(db)
            .as_ref()
            .map(|docs| format!("\n    docs: Some({docs:?}),"))
            .unwrap_or_default();

        let fns = render_list(
            lower_mod_fns(db, self.id(db).rustdoc_id(db))
                .iter()
                .map(|fn_| fn_.render(db))
                .collect(),
        );

        let structs = lower_mod_structs(db, self.id(db).rustdoc_id(db))
            .iter()
            .map(|struct_| struct_.render(db))
            .collect::<Vec<_>>();

        let submods = render_list(
            lower_mod_submods(db, self.id(db).rustdoc_id(db))
                .iter()
                .map(|mod_| mod_.render(db))
                .collect(),
        );

        if structs.is_empty() {
            return format!(
                "ModDef {{\n    id: {:?},{docs}\n    fns: {fns},\n    submods: {submods},\n}}",
                self.id(db).path(db).join("::"),
            );
        }

        let structs = render_list(structs);

        format!(
            "ModDef {{\n    id: {:?},{docs}\n    fns: {fns},\n    structs: {structs},\n    submods: {submods},\n}}",
            self.id(db).path(db).join("::"),
        )
    }
}

impl<'db> DebugRender<'db> for Type<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        match self {
            Type::Primitive(primitive) => primitive.render(db),
            Type::Str => "str".to_string(),
            Type::String => "String".to_string(),
            Type::Struct(id) => struct_fqn_from_ir_id(db, *id).to_owned(),
        }
    }
}

impl<'db> DebugRender<'db> for Primitive {
    fn render(&self, _db: &'db dyn Db) -> String {
        match self {
            Primitive::Boolean => "bool",
            Primitive::Char => "char",
            Primitive::i8 => "i8",
            Primitive::u8 => "u8",
            Primitive::i16 => "i16",
            Primitive::u16 => "u16",
            Primitive::i32 => "i32",
            Primitive::u32 => "u32",
            Primitive::f32 => "f32",
            Primitive::f64 => "f64",
        }
        .to_string()
    }
}

impl<'db> DebugRender<'db> for StructField<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        let docs = self
            .docs(db)
            .as_ref()
            .map(|docs| format!("\n    docs: Some({docs:?}),"))
            .unwrap_or_default();

        let is_readonly = if self.is_readonly(db) {
            "\n    is_readonly: true,"
        } else {
            Default::default()
        };

        format!(
            "PlainStructField {{\n    name: {:?},\n    type: {},{is_readonly}{docs}\n}}",
            self.name(db),
            self.type_(db).render(db),
        )
    }
}

impl<'db> DebugRender<'db> for StructShape<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        match self {
            StructShape::Unit => "Unit".to_string(),
            StructShape::Tuple(fields) => {
                let fields = render_list(
                    fields
                        .iter()
                        .map(|field| match field {
                            Some(field) => {
                                let docs = field
                                    .docs(db)
                                    .as_ref()
                                    .map(|docs| format!("\n    docs: Some({docs:?}),"))
                                    .unwrap_or_default();

                                let is_readonly = if field
                                    .is_readonly(db) { "\n    is_readonly: true," } else { Default::default() };

                                format!(
                                    "Some(TupleStructField {{\n    type: {},{is_readonly}{docs}\n}})",
                                    field.type_(db).render(db),
                                )
                            }
                            None => "None".to_string(),
                        })
                        .collect(),
                );

                format!("Tuple({fields})")
            }
            StructShape::Plain {
                fields,
                has_stripped_fields,
            } => {
                let fields = render_list(fields.iter().map(|field| field.render(db)).collect());

                format!(
                    "Plain {{\n    fields: {fields},\n    has_stripped_fields: {has_stripped_fields},\n}}"
                )
            }
        }
    }
}

impl<'db> DebugRender<'db> for Struct<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        let docs = self
            .docs(db)
            .as_ref()
            .map(|docs| format!("\n    docs: Some({docs:?}),"))
            .unwrap_or_default();
        let shape = indent(&self.shape(db).render(db), 8);

        format!(
            "Struct {{\n    id: {:?},{docs}\n    shape:\n{shape},\n}}",
            self.fqn(db),
        )
    }
}

impl<'db> DebugRender<'db> for FnParam<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        format!(
            "FnParam {{\n    name: {:?},\n    type: {},\n    mode: {:?},\n}}",
            self.name(db),
            self.type_(db).render(db),
            self.mode(db),
        )
    }
}

impl<'db> DebugRender<'db> for Fn<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        let params = render_list(
            self.params(db)
                .iter()
                .map(|param| param.render(db))
                .collect(),
        );

        let return_type = match self.return_type(db) {
            Some(return_type) => format!("Some({})", return_type.render(db)),
            None => "None".to_string(),
        };
        let docs = self
            .docs(db)
            .as_ref()
            .map(|docs| format!("\n    docs: Some({docs:?}),"))
            .unwrap_or_default();

        format!(
            "Fn {{\n    id: {:?},{docs}\n    params: {params},\n    return_type: {return_type},\n}}",
            self.fqn(db),
        )
    }
}

fn render_list(items: Vec<String>) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }

    let mut rendered = String::from("[\n");

    for item in items {
        rendered.push_str(&indent(&item, 8));
        rendered.push_str(",\n");
    }

    rendered.push_str(&" ".repeat(4));
    rendered.push(']');
    rendered
}

fn indent(text: &str, spaces: usize) -> String {
    let padding = " ".repeat(spaces);
    text.lines()
        .map(|line| format!("{padding}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
