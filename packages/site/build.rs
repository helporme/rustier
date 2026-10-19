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
        .expect("rustc must report its sysroot");
    let sysroot = PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("sysroot must be UTF-8")
            .trim(),
    );
    let library = sysroot.join("lib/rustlib/src/rust/library");
    let mut files = Vec::new();

    for name in ["core", "alloc", "std"] {
        collect_sources(&library.join(name).join("src"), &mut files);
    }

    files.sort();

    let destination = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"));
    let mut generated =
        File::create(destination.join("sysroot.rs")).expect("sysroot output must be writable");

    writeln!(generated, "const SYSROOT: &[(&str, &str)] = &[").unwrap();

    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());

        let relative = file
            .strip_prefix(&library)
            .expect("source must belong to sysroot");
        let path = relative.to_string_lossy().replace('\\', "/");
        let source = fs::read_to_string(&file).expect("rust-src must contain UTF-8 source");

        writeln!(generated, "({path:?}, {source:?}),").unwrap();
    }

    writeln!(generated, "];").unwrap();

    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("rustc must report its version");
    let version = String::from_utf8(output.stdout).expect("rustc version must be UTF-8");
    let version = version
        .split_whitespace()
        .nth(1)
        .expect("rustc must report a version");

    println!("cargo:rustc-env=RUSTIER_RUST_VERSION={version}");
    println!("cargo:rerun-if-changed=build.rs");
}

fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(directory).expect("install rust-src with rustup component add rust-src")
    {
        let path = entry.expect("rust-src entries must be readable").path();

        if path.is_dir() {
            collect_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
