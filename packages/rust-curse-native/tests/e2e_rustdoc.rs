mod common;

use std::path::Path;

use rustdoc_types::ItemEnum;

use rust_curse::discovery::rustdoc::generate_rustdoc;
use rust_curse::session::{BuildStep, Session, SessionOptions};

use common::copy_dir_recursive;

#[test]
#[ignore] // requires cargo +nightly, slow (~10s)
fn rustdoc_loads_basic_crate() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate");
    let tmp = tempfile::tempdir().unwrap();
    let tmp_abs_path = tmp.path().canonicalize().unwrap();

    copy_dir_recursive(&fixture, &tmp_abs_path);

    let mut session = Session::new(SessionOptions {
        targets: vec![],
        module_dir_rel_path: tmp_abs_path.join("src").to_string_lossy().into_owned(),
        output_dir_rel_path: tmp_abs_path.join(".rust-curse").to_string_lossy().into_owned(),
        cargo_toml_rel_path: tmp_abs_path.join("Cargo.toml").to_string_lossy().into_owned(),
        project_root_abs_path: tmp_abs_path.to_string_lossy().into_owned(),
        last_step: Some(BuildStep::Rustdoc),
    });

    session.build();

    // Load the generated rustdoc JSON to verify its contents
    let crate_ = generate_rustdoc(&tmp_abs_path).unwrap();

    let items_by_name: std::collections::HashMap<&str, &rustdoc_types::Item> = crate_
        .index
        .values()
        .filter_map(|item| item.name.as_deref().map(|n| (n, item)))
        .collect();

    // Verify pub fn hello
    assert!(
        matches!(items_by_name.get("hello").map(|i| &i.inner), Some(ItemEnum::Function(_))),
        "expected 'hello' to be a function, got {:?}",
        items_by_name.get("hello").map(|i| &i.inner)
    );

    // Verify pub struct Point with fields x, y
    let point_item = items_by_name.get("Point").expect("expected 'Point' in index");
    assert!(matches!(&point_item.inner, ItemEnum::Struct(_)), "expected 'Point' to be a struct");

    if let ItemEnum::Struct(s) = &point_item.inner {
        if let rustdoc_types::StructKind::Plain { fields, .. } = &s.kind {
            let field_names: Vec<&str> = fields
                .iter()
                .filter_map(|id| crate_.index.get(id))
                .filter_map(|item| item.name.as_deref())
                .collect();
            assert!(field_names.contains(&"x"), "expected field 'x', got {field_names:?}");
            assert!(field_names.contains(&"y"), "expected field 'y', got {field_names:?}");
        } else {
            panic!("expected Point to be a plain struct");
        }
    }

    // Verify pub fn add
    assert!(
        matches!(items_by_name.get("add").map(|i| &i.inner), Some(ItemEnum::Function(_))),
        "expected 'add' to be a function"
    );
}
