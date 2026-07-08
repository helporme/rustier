/// Mangles a struct/enum/mod/etc. path into a unique identiefer.
pub fn mangle_path(path: &[String]) -> String {
    let mut mangled = String::new();

    for (i, s) in path.iter().enumerate() {
        if i > 0 {
            mangled.push('_');
        }

        for ch in s.chars() {
            match ch {
                '0' => mangled.push_str("00"),
                '_' => mangled.push_str("_0"),
                _ => mangled.push(ch),
            }
        }
    }

    mangled
}

#[cfg(test)]
mod tests {
    use super::mangle_path;

    #[test]
    fn mangles_empty_path_to_empty_ident() {
        assert_eq!(mangle_path(&[]), "");
    }

    #[test]
    fn joins_path_segments_with_underscores() {
        assert_eq!(
            mangle_path(&[
                "crate".to_string(),
                "module".to_string(),
                "Type".to_string()
            ]),
            "crate_module_Type"
        );
    }

    #[test]
    fn escapes_zeroes_inside_segments() {
        assert_eq!(mangle_path(&["item0".to_string()]), "item00");
    }

    #[test]
    fn escapes_underscores_inside_segments() {
        assert_eq!(mangle_path(&["my_mod".to_string()]), "my_0mod");
    }

    #[test]
    fn escaped_segment_chars_do_not_collide_with_separators() {
        assert_ne!(
            mangle_path(&["my_mod".to_string()]),
            mangle_path(&["my".to_string(), "mod".to_string()])
        );
        assert_eq!(mangle_path(&["my_mod".to_string()]), "my_0mod");
        assert_eq!(
            mangle_path(&["my".to_string(), "mod".to_string()]),
            "my_mod"
        );
    }
}
