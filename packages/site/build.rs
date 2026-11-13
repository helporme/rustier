use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let rustc = env::var_os("RUSTC").expect("Cargo must provide RUSTC");
    let output = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    let library = sysroot.join("lib/rustlib/src/rust/library");
    let aux = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../crates/runtime");
    let destination = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut generated = File::create(destination.join("sources.rs")).unwrap();

    writeln!(generated, "const SOURCES: &[(&str, &str)] = &[").unwrap();
    for (root, prefix) in [
        (library.join("core/src"), "/core/src"),
        (library.join("alloc/src"), "/alloc/src"),
        (library.join("std/src"), "/std/src"),
        (aux.join("src"), "/aux/src"),
    ] {
        let mut files = Vec::new();
        collect_sources(&root, &mut files);
        files.sort();
        for file in files {
            println!("cargo:rerun-if-changed={}", file.display());
            let path = format!("{prefix}/{}", file.strip_prefix(&root).unwrap().display())
                .replace('\\', "/");
            let text = fs::read_to_string(&file).unwrap();
            writeln!(generated, "({path:?}, {text:?}),").unwrap();
        }
    }
    writeln!(generated, "];").unwrap();

    let output = Command::new(rustc).arg("--version").output().unwrap();
    let version = String::from_utf8(output.stdout).unwrap();
    let version = version.split_whitespace().nth(1).unwrap();
    println!("cargo:rustc-env=RUSTIER_RUST_VERSION={version}");
    println!("cargo:rerun-if-changed=build.rs");
}

fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(directory).expect("install rust-src with rustup component add rust-src")
    {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
