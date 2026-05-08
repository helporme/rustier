//! End-to-end tree-shake tests.
//!
//! Each test builds a tempdir fixture (orphan `.rs` modules + JS files that
//! import them), generates rustdoc + IR, runs `tree_shake::shake`, and
//! snapshots the resolved item names. The on-disk fixture is included as
//! the snapshot description so `cargo insta review` shows the source layout
//! when pressing `i`.

use std::path::PathBuf;

use rust_curse::tree_shake::TreeShakeError;

use crate::common::{File, Fixture};

#[test]
fn named_import_seeds_one_fn() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[
            File {
                rel_path: "src/greet.rs",
                content: "pub fn hello() {}\npub fn unused() {}\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { hello } from '../src/greet.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn star_import_seeds_all_module_exports() {
    let fixture = Fixture::new(
        "src",
        &["math.rs"],
        &[
            File {
                rel_path: "src/math.rs",
                content: "pub fn add() {}\npub fn sub() {}\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import * from '../src/math.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn fn_returning_struct_pulls_in_struct() {
    let fixture = Fixture::new(
        "src",
        &["shapes.rs"],
        &[
            File {
                rel_path: "src/shapes.rs",
                content: "pub struct Point { pub x: i32, pub y: i32 }\n\
                          pub fn make_origin() -> Point { Point { x: 0, y: 0 } }\n\
                          pub fn unused_fn() {}\n\
                          pub struct UnusedStruct { pub z: i32 }\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { make_origin } from '../src/shapes.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn struct_field_pulls_in_inner_struct() {
    let fixture = Fixture::new(
        "src",
        &["shapes.rs"],
        &[
            File {
                rel_path: "src/shapes.rs",
                content: "pub struct Point { pub x: i32, pub y: i32 }\n\
                          pub struct Circle { pub center: Point, pub radius: f64 }\n\
                          pub fn make_circle() -> Circle { Circle { center: Point { x: 0, y: 0 }, radius: 1.0 } }\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { make_circle } from '../src/shapes.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn no_js_imports_yields_empty_set() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[File {
            rel_path: "src/greet.rs",
            content: "pub fn hello() {}\n",
        }],
    );

    let include: Vec<String> = vec![];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn unresolved_named_import_errors() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[
            File {
                rel_path: "src/greet.rs",
                content: "pub fn hello() {}\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { goodbye } from '../src/greet.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let err = fixture.run_shake("src", &include).unwrap_err();
    assert!(
        matches!(err, TreeShakeError::UnresolvedImport { ref name, .. } if name == "goodbye"),
        "{err:?}"
    );
}

#[test]
fn unresolved_file_import_errors() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[
            File {
                rel_path: "src/greet.rs",
                content: "pub fn hello() {}\n",
            },
            File {
                rel_path: "src/missing.rs",
                content: "pub fn bye() {}\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { bye } from '../src/missing.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let err = fixture.run_shake("src", &include).unwrap_err();
    assert!(
        matches!(
            err,
            TreeShakeError::UnresolvedFile { ref rel_rs_path }
                if rel_rs_path == &PathBuf::from("src/missing.rs")
        ),
        "{err:?}"
    );
}

#[test]
fn type_only_import_yields_empty_set() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[
            File {
                rel_path: "src/greet.rs",
                content: "pub fn hello() {}\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import type { hello } from '../src/greet.rs'\n\
                          import { type hello as H } from '../src/greet.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn mixed_inline_type_value_import_seeds_only_value() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[
            File {
                rel_path: "src/greet.rs",
                content: "pub fn hello() {}\n\
                          pub struct Greeting { pub msg: String }\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { hello, type Greeting } from '../src/greet.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn value_import_and_separate_type_import_seeds_only_value() {
    let fixture = Fixture::new(
        "src",
        &["greet.rs"],
        &[
            File {
                rel_path: "src/greet.rs",
                content: "pub fn hello() {}\n\
                          pub struct Greeting { pub msg: String }\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { hello } from '../src/greet.rs'\n\
                          import type { Greeting } from '../src/greet.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}

#[test]
fn nested_orphan_module_resolved_by_path() {
    let fixture = Fixture::new(
        "src",
        &["nested/deep.rs"],
        &[
            File {
                rel_path: "src/nested/deep.rs",
                content: "pub fn go() {}\n",
            },
            File {
                rel_path: "app/main.js",
                content: "import { go } from '../src/nested/deep.rs'\n",
            },
        ],
    );

    let include = vec![format!("{}/app/**/*.js", fixture.abs_root.display())];
    let (ir, exposed) = fixture.run_shake("src", &include).expect("shake ok");
    snapshot!(fixture, ir, exposed);
}
