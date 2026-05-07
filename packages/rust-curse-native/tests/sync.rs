#[path = "sync/common.rs"]
mod common;

#[path = "sync/patch_cargo_toml.rs"]
mod patch_cargo_toml;

#[path = "sync/write_build.rs"]
mod write_build;

#[path = "sync/write_lib.rs"]
mod write_lib;

#[path = "sync/write_orphan_tree.rs"]
mod write_orphan_tree;

#[path = "sync/write_rustdoc.rs"]
mod write_rustdoc;

#[path = "sync/write_ts.rs"]
mod write_ts;

#[path = "sync/write_wired_cargo_toml.rs"]
mod write_wired_cargo_toml;
