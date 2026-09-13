fn main() {
    println!(
        "cargo:rustc-env=RUST_CURSE_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    #[cfg(feature = "napi-binding")]
    napi_build::setup();
}
