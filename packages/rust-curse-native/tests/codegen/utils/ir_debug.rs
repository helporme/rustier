use rust_curse::codegen::{db::Db, ir};

/// Renders the item as a debug human-readable string.
///
/// Direct alternative to Debug.
// todo: put a #[cfg(debug)] or smth like that on it?
pub trait DebugRender<'db> {
    /// Returns the human-readable debug string representation of the item.
    fn render(&self, db: &'db dyn Db) -> String;
}

impl<'db> DebugRender<'db> for ir::ModDef<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        let fns = render_list(self.fns(db).iter().map(|fn_| fn_.render(db)).collect());

        format!(
            "ModDef {{\n    id: {:?},\n    fns: {fns},\n}}",
            self.id(db).path(db).join("::"),
        )
    }
}

impl<'db> DebugRender<'db> for ir::Type {
    fn render(&self, db: &'db dyn Db) -> String {
        match self {
            ir::Type::Primitive(primitive) => primitive.render(db),
        }
    }
}

impl<'db> DebugRender<'db> for ir::Primitive {
    fn render(&self, _db: &'db dyn Db) -> String {
        match self {
            ir::Primitive::Boolean => "bool",
            ir::Primitive::Char => "char",
            ir::Primitive::Str => "str",
            ir::Primitive::i8 => "i8",
            ir::Primitive::u8 => "u8",
            ir::Primitive::i16 => "i16",
            ir::Primitive::u16 => "u16",
            ir::Primitive::i32 => "i32",
            ir::Primitive::u32 => "u32",
            ir::Primitive::f32 => "f32",
            ir::Primitive::f64 => "f64",
        }
        .to_string()
    }
}

impl<'db> DebugRender<'db> for ir::FnParam<'db> {
    fn render(&self, db: &'db dyn Db) -> String {
        format!(
            "FnParam {{\n    name: {:?},\n    type: {},\n    mode: {:?},\n}}",
            self.name(db),
            self.type_(db).render(db),
            self.mode(db),
        )
    }
}

impl<'db> DebugRender<'db> for ir::Fn<'db> {
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

        format!(
            "Fn {{\n    id: {:?},\n    params: {params},\n    return_type: {return_type},\n}}",
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
