mod common;

use std::path::Path;

use rust_curse::session::{BuildStep, Session, SessionOptions};

use common::copy_dir_recursive;

#[test]
fn link_populates_item_ids() {
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
        last_step: Some(BuildStep::Link),
    });

    session.build();

    let modules = session.last_modules();

    // We expect two orphan modules: greet and math
    assert_eq!(modules.len(), 2, "expected 2 modules, got {}", modules.len());

    let greet = modules.iter().find(|m| m.name == "greet").expect("missing greet module");
    let math = modules.iter().find(|m| m.name == "math").expect("missing math module");

    // greet.rs has: pub fn hello
    assert!(
        !greet.item_ids.is_empty(),
        "greet module should have linked item IDs, got none"
    );

    // math.rs has: pub struct Point + pub fn add
    assert!(
        math.item_ids.len() >= 2,
        "math module should have at least 2 item IDs (Point + add), got {}",
        math.item_ids.len()
    );
}
