use std::collections::{BTreeMap, HashMap};

use rust_curse::codegen::{
    db::DbImpl,
    rustdoc::{RustdocId, RustdocInput},
    ts::compile_mod,
};
use rustdoc_types::{
    Abi, Crate, Function, FunctionHeader, FunctionSignature, Generics, Id, Item, ItemEnum,
    ItemKind, ItemSummary, Module, Target, Type, Visibility,
};

fn item(id: Id, name: &str, docs: Option<&str>, inner: ItemEnum) -> Item {
    Item {
        id,
        crate_id: 0,
        name: Some(name.to_string()),
        span: None,
        visibility: Visibility::Public,
        docs: docs.map(str::to_string),
        links: HashMap::new(),
        attrs: Vec::new(),
        deprecation: None,
        inner,
    }
}

fn fn_item(
    id: Id,
    name: &str,
    docs: Option<&str>,
    inputs: Vec<(&str, Type)>,
    output: Option<Type>,
) -> Item {
    item(
        id,
        name,
        docs,
        ItemEnum::Function(Function {
            sig: FunctionSignature {
                inputs: inputs
                    .into_iter()
                    .map(|(param_name, param_type)| (param_name.to_string(), param_type))
                    .collect(),
                output,
                is_c_variadic: false,
            },
            generics: Generics {
                params: Vec::new(),
                where_predicates: Vec::new(),
            },
            header: FunctionHeader {
                is_const: false,
                is_unsafe: false,
                is_async: false,
                abi: Abi::Rust,
            },
            has_body: true,
        }),
    )
}

fn primitive(name: &str) -> Type {
    Type::Primitive(name.to_string())
}

fn summary(path: &[&str], kind: ItemKind) -> ItemSummary {
    ItemSummary {
        crate_id: 0,
        path: path.iter().map(|segment| segment.to_string()).collect(),
        kind,
    }
}

fn fixture_crate() -> Crate {
    let mut index = HashMap::new();
    index.insert(
        Id(0),
        item(
            Id(0),
            "test_crate",
            None,
            ItemEnum::Module(Module {
                is_crate: true,
                items: vec![Id(1), Id(2), Id(3)],
                is_stripped: false,
            }),
        ),
    );
    index.insert(
        Id(1),
        fn_item(
            Id(1),
            "add",
            Some("Adds two numbers."),
            vec![("a", primitive("i32")), ("b", primitive("i32"))],
            Some(primitive("i32")),
        ),
    );
    index.insert(
        Id(2),
        fn_item(
            Id(2),
            "greet",
            None,
            vec![
                (
                    "name",
                    Type::BorrowedRef {
                        lifetime: None,
                        is_mutable: false,
                        type_: Box::new(primitive("str")),
                    },
                ),
                ("excited", primitive("bool")),
            ],
            Some(primitive("u64")),
        ),
    );
    index.insert(Id(3), fn_item(Id(3), "ping", None, Vec::new(), None));

    let mut paths = HashMap::new();
    paths.insert(Id(0), summary(&["test_crate"], ItemKind::Module));
    paths.insert(Id(1), summary(&["test_crate", "add"], ItemKind::Function));
    paths.insert(Id(2), summary(&["test_crate", "greet"], ItemKind::Function));
    paths.insert(Id(3), summary(&["test_crate", "ping"], ItemKind::Function));

    Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        target: Target {
            triple: String::new(),
            target_features: Vec::new(),
        },
        format_version: rustdoc_types::FORMAT_VERSION,
    }
}

#[test]
fn compile_mod_prints_free_fns() {
    let db = DbImpl::default();

    let mut crates = BTreeMap::new();
    crates.insert("test_crate".to_string(), fixture_crate());
    RustdocInput::new(&db, crates);

    let mod_id = RustdocId::new(&db, Id(0), "test_crate".to_string());
    let printed = compile_mod(&db, mod_id);

    let expected = "\
/** Adds two numbers. */
export function add(a: number, b: number): number;

export function greet(name: string, excited: boolean): bigint;

export function ping(): void;
";

    assert_eq!(printed, expected);
}
