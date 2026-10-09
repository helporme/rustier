use crate::compiler::{
    db::Db,
    ir::{
        decals::{DecalParam, DecalType},
        shim::{ShimFunction, ShimType},
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

impl<'db> ShimType<'db> {
    pub(super) fn lower_decals(self, db: &'db dyn Db) -> DecalType<'db> {
        match self {
            ShimType::f64 => DecalType::number,
            ShimType::String => DecalType::string,
            ShimType::bool => DecalType::boolean,
            ShimType::Class { inner, .. } => DecalType::Class {
                inner: inner.decals(db),
                coerced: false,
            },
        }
    }
}

#[salsa::tracked]
impl<'db> DecalParam<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_name(self, db: &'db dyn Db) -> String {
        let shim = self.shim(db);
        let (function, param_idx) = match shim.parent(db) {
            ShimFunction::Free(function) => (function.src(db), shim.idx(db)),
            ShimFunction::Associated(function) => {
                let receiver_offset =
                    usize::from(function.src(db).declaration(db).receiver(db).is_some());
                (function.src(db), shim.idx(db) - receiver_offset)
            }
            _ => unreachable!("declaration parameters only belong to source-defined functions"),
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
