/// Converts an upper-camel-case identifier to lower camel case.
pub fn to_camel_case(name: &str) -> String {
    let char_count = name.chars().count();
    let uppercase_count = name
        .chars()
        .take_while(|character| character.is_uppercase())
        .count();
    let lowercase_count = if uppercase_count > 1 && uppercase_count < char_count {
        uppercase_count - 1
    } else {
        uppercase_count
    };

    let mut camel_case_name = String::with_capacity(name.len());
    for (index, character) in name.chars().enumerate() {
        if index < lowercase_count {
            camel_case_name.extend(character.to_lowercase());
        } else {
            camel_case_name.push(character);
        }
    }
    camel_case_name
}

/// Mangles a struct/enum/mod/etc. path into a unique identiefer.
pub fn mangle_path<'a>(path: impl IntoIterator<Item = &'a String>) -> String {
    let mut mangled = String::new();

    for (i, s) in path.into_iter().enumerate() {
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
    use super::{mangle_path, to_camel_case};

    #[test]
    fn converts_upper_camel_case_to_lower_camel_case() {
        assert_eq!(to_camel_case("Circle"), "circle");
        assert_eq!(to_camel_case("HttpRequest"), "httpRequest");
    }

    #[test]
    fn converts_uppercase_acronym_to_lower_camel_case() {
        assert_eq!(to_camel_case("HTTPRequest"), "httpRequest");
        assert_eq!(to_camel_case("URL"), "url");
    }

    #[test]
    fn preserves_names_without_an_uppercase_prefix() {
        assert_eq!(to_camel_case("alreadyCamel"), "alreadyCamel");
        assert_eq!(to_camel_case(""), "");
    }

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
