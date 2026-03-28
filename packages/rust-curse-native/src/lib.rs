mod steps;

use std::fs;
use std::path::Path;

use napi_derive::napi;
use toml_edit::DocumentMut;

use steps::{
    s1_discovery, s2_import_scan, s3_treeshake, s4_dev_generate_rust, s4_napi_generate_rust,
    s5_dev_typescript, s6_napi_js,
};

#[napi(string_enum)]
pub enum BuildTarget {
    Dev,
    Napi,
}

#[napi(object)]
pub struct SourceOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[napi(object)]
pub struct BuildOptions {
    /// Build target
    pub target: BuildTarget,
    /// JavaScript source file to scan for imports
    pub js_source: SourceOptions,
    /// Directory to discover rust orphan modules
    pub module_dir: String,
    /// Output dir for placing generated files e.g. .rust-curse/ssr.
    pub output_dir: String,
    /// Path to the original Cargo.toml.
    pub cargo_path: String,
    /// Project root directory, used for computing relative paths.
    pub project_root: String,
}

#[napi]
pub fn build(options: BuildOptions) {
    // Step 1: Discover rust orphan modules
    let discovered =
        s1_discovery::discover(&options.module_dir).expect("failed to discover rust modules");

    // Step 2: Scan JS/TS files for imports of discovered modules
    let imports = s2_import_scan::scan_for_imports_of_rust_modules_in_js(
        &discovered,
        options.js_source.include,
        options.js_source.exclude,
    );

    // Step 3: Tree-shake modules against imports
    let shaked = s3_treeshake::treeshake(&discovered, &imports);

    // Step 4: Generate files based on target
    let output_dir = Path::new(&options.output_dir);
    fs::create_dir_all(output_dir).expect("failed to create output directory");

    let lib_rs_path = output_dir.join("lib.rs");
    match options.target {
        BuildTarget::Dev => {
            s4_dev_generate_rust::generate_lib_and_modules(output_dir, &discovered, &shaked);
        }
        BuildTarget::Napi => {
            s4_napi_generate_rust::generate_lib_and_modules(output_dir, &discovered, &shaked);
        }
    };

    // Patch Cargo.toml
    let cargo_content = fs::read_to_string(&options.cargo_path).expect("failed to read Cargo.toml");
    let doc: DocumentMut = cargo_content.parse().expect("failed to parse Cargo.toml");
    let lib_rs_rel = lib_rs_path.to_string_lossy().to_string();

    match options.target {
        BuildTarget::Dev => {
            let patched = s4_dev_generate_rust::generate_cargo_toml(doc, &lib_rs_rel);
            fs::write(&options.cargo_path, patched.to_string())
                .expect("failed to write Cargo.toml");

            // Step 5: Generate .d.ts files
            s5_dev_typescript::generate_types(
                &discovered,
                &output_dir.join("types"),
                Path::new(&options.project_root),
            );
        }
        BuildTarget::Napi => {
            let patched = s4_napi_generate_rust::generate_cargo_toml(doc, "./lib.rs");
            let cargo_out_path = output_dir.join("Cargo.toml");
            fs::write(&cargo_out_path, patched.to_string()).expect("failed to write Cargo.toml");

            // Step 6: Generate JS glue modules
            s6_napi_js::generate_js_glue(
                &shaked,
                "../output/index.js",
                &output_dir.join("modules"),
                Path::new(&options.project_root),
            );
        }
    };
}
