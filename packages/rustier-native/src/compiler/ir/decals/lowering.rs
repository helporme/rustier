use crate::compiler::{
    db::Db,
    ir::{
        decals::{DecalParam, DecalType},
        facade::FacadeFunction,
        rust::{Type, TypeKind},
    },
};

const RESERVED_PARAM_NAMES: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

impl<'db> Type<'db> {
    pub(super) fn lower_decals(self, db: &'db dyn Db) -> DecalType<'db> {
        if let Some(class) = self.facade_class(db) {
            return DecalType::Class {
                inner: class.decals(db),
                coerced: false,
            };
        }

        match self.kind(db) {
            TypeKind::bool => DecalType::boolean,
            TypeKind::char
            | TypeKind::i8
            | TypeKind::u8
            | TypeKind::i16
            | TypeKind::u16
            | TypeKind::i32
            | TypeKind::u32
            | TypeKind::f32
            | TypeKind::f64 => DecalType::number,
            TypeKind::Str | TypeKind::String => DecalType::string,
            TypeKind::Borrowed(borrowed) => borrowed.inner(db).lower_decals(db),
            TypeKind::Struct(_) | TypeKind::Enum(_) | TypeKind::Generic(_) => {
                unreachable!("unsupported Rust types do not enter the declarations")
            }
        }
    }
}

#[salsa::tracked]
impl<'db> DecalParam<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_name(self, db: &'db dyn Db) -> String {
        let interface = self.interface(db);
        let (function, param_idx) = match interface.parent(db) {
            FacadeFunction::Free(function) => (function.src(db), interface.idx(db)),
            FacadeFunction::Method(function) => {
                let receiver_offset =
                    usize::from(function.src(db).declaration(db).receiver(db).is_some());
                (function.src(db), interface.idx(db) - receiver_offset)
            }
        };

        let name = function.params(db)[param_idx].name(db).as_str();
        let name = name.strip_prefix("r#").unwrap_or(name);
        let mut lowered_name = name.to_owned();

        if RESERVED_PARAM_NAMES.contains(&name) {
            lowered_name.push('$');
        }

        lowered_name
    }
}
