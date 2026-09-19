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

#[cfg(test)]
mod tests {
    use super::to_camel_case;

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
}
