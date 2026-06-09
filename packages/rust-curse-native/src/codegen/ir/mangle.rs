/// Mangles a struct/enum/mod/etc. path into a unique ident.
pub fn mangle_path(path: &[String]) -> String {
    path.iter()
        .map(|s| s.replace("0", "00").replace("_", "_0"))
        .collect::<Vec<_>>()
        .join("_")
}
