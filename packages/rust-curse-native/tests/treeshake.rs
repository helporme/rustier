mod common;

use std::path::Path;

use rust_curse::session::{BuildStep, BuildTarget, Session, SessionOptions, SourceOptions};

use common::copy_dir_recursive;

#[test]
fn treeshake_resolves_valid_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate");
    let tmp = tempfile::tempdir().unwrap();
    let tmp_abs_path = tmp.path().canonicalize().unwrap();

    copy_dir_recursive(&fixture, &tmp_abs_path);

    let src_dir = tmp_abs_path.join("src");

    let mut session = Session::new(SessionOptions {
        targets: vec![BuildTarget::Node {
            js_source: SourceOptions {
                include: vec![format!("{}/**/*.js", src_dir.display())],
                exclude: vec![format!("{}/bad.js", src_dir.display())],
            },
            feature_name: "target-node".into(),
        }],
        module_dir_rel_path: src_dir.to_string_lossy().into_owned(),
        output_dir_rel_path: tmp_abs_path.join(".rust-curse").to_string_lossy().into_owned(),
        cargo_toml_rel_path: tmp_abs_path.join("Cargo.toml").to_string_lossy().into_owned(),
        project_root_abs_path: tmp_abs_path.to_string_lossy().into_owned(),
        last_step: Some(BuildStep::Treeshake),
    });

    let result = session.build();

    // app.js imports { hello } from greet.rs and { add, Point } from math.rs
    // All are valid public items -> no warnings
    assert!(
        result.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        result.warnings,
    );

    // Modules should have item_ids populated from the Link step
    let modules = session.last_modules();
    assert_eq!(modules.len(), 2);

    let greet = modules.iter().find(|m| m.name == "greet").expect("missing greet");
    assert!(!greet.item_ids.is_empty(), "greet should have item_ids");

    let math = modules.iter().find(|m| m.name == "math").expect("missing math");
    assert!(math.item_ids.len() >= 2, "math should have at least 2 item_ids (Point + add)");
}

#[test]
fn treeshake_warns_on_bad_imports() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate");
    let tmp = tempfile::tempdir().unwrap();
    let tmp_abs_path = tmp.path().canonicalize().unwrap();

    copy_dir_recursive(&fixture, &tmp_abs_path);

    let src_dir = tmp_abs_path.join("src");

    let mut session = Session::new(SessionOptions {
        targets: vec![BuildTarget::Node {
            js_source: SourceOptions {
                // Only include bad.js
                include: vec![format!("{}/bad.js", src_dir.display())],
                exclude: vec![],
            },
            feature_name: "target-node".into(),
        }],
        module_dir_rel_path: src_dir.to_string_lossy().into_owned(),
        output_dir_rel_path: tmp_abs_path.join(".rust-curse").to_string_lossy().into_owned(),
        cargo_toml_rel_path: tmp_abs_path.join("Cargo.toml").to_string_lossy().into_owned(),
        project_root_abs_path: tmp_abs_path.to_string_lossy().into_owned(),
        last_step: Some(BuildStep::Treeshake),
    });

    let result = session.build();

    // bad.js imports { nonexistent } from greet.rs -> UnresolvedMember warning
    // bad.js imports { foo } from missing.rs -> missing.rs doesn't exist on disk,
    //   so scan_imports won't even produce an RsImport for it (resolve_specifier fails).
    //   Only the unresolved member warning for greet.rs should appear.
    assert_eq!(
        result.warnings.len(),
        1,
        "expected 1 warning, got: {:?}",
        result.warnings,
    );
    assert!(
        result.warnings[0].contains("nonexistent"),
        "warning should mention 'nonexistent', got: {}",
        result.warnings[0],
    );
    assert!(
        result.warnings[0].contains("greet.rs"),
        "warning should mention 'greet.rs', got: {}",
        result.warnings[0],
    );
}
