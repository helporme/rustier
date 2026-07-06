// todo: find a better way to structure such tests?

#[path = "codegen/fns/diags.rs"]
mod fns_diags;

#[path = "codegen/fns/docs.rs"]
mod fns_docs;

#[path = "codegen/fns/primitives.rs"]
mod fns_primitives;

#[path = "codegen/fns/sourcemap.rs"]
mod fns_sourcemap;

#[path = "codegen/fns/strings.rs"]
mod fns_strings;

#[path = "codegen/structs/diags.rs"]
mod structs_diags;

#[path = "codegen/structs/docs.rs"]
mod structs_docs;

#[path = "codegen/structs/primitives.rs"]
mod structs_primitives;

#[path = "codegen/structs/strings.rs"]
mod structs_strings;

#[path = "codegen/structs/visibility.rs"]
mod structs_visibility;

#[path = "codegen/structs/unit.rs"]
mod structs_unit;

#[path = "codegen/mods/docs.rs"]
mod mods_docs;

#[path = "codegen/mods/submods.rs"]
mod mods_submods;

#[path = "codegen/mods/mods.rs"]
mod mods_mods;

#[path = "codegen/utils.rs"]
mod utils;
