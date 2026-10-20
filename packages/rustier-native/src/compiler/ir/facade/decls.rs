use crate::compiler::{
    db::Db,
    ir::facade::{FacadeFreeFunction, FacadeFunction, FacadeParam, traits::FacadeTrait},
};

const RESERVED_BINDING_NAMES: &[&str] = &[
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

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn decl_name(self, db: &'db dyn Db) -> String {
        let mut name = self.name(db).to_owned();

        if RESERVED_BINDING_NAMES.contains(&name.as_str()) {
            name.push('$');
        }

        name
    }
}

#[salsa::tracked]
impl<'db> FacadeTrait<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn decl_name(self, db: &'db dyn Db) -> String {
        format!("$trait__{}", self.src(db).path(db).mangled(db))
    }
}

#[salsa::tracked]
impl<'db> FacadeParam<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn decl_name(self, db: &'db dyn Db) -> String {
        let (function, param_idx) = match self.parent(db) {
            FacadeFunction::Free(function) => (function.src(db), self.idx(db)),
            FacadeFunction::Method(function) => {
                let receiver_offset =
                    usize::from(function.src(db).declaration(db).receiver(db).is_some());

                (function.src(db), self.idx(db) - receiver_offset)
            }
        };
        let name = function.params(db)[param_idx].name(db).as_str();
        let name = name.strip_prefix("r#").unwrap_or(name);
        let mut lowered_name = name.to_owned();

        if RESERVED_BINDING_NAMES.contains(&name) {
            lowered_name.push('$');
        }

        lowered_name
    }
}
