pub fn accepts_string(value: String) -> bool {
    value == "owned 🦀"
}

pub fn accepts_qualified_string(value: std::string::String) -> bool {
    value == "qualified 🦀"
}

pub fn accepts_ref_string(value: &String) -> bool {
    value == "borrowed 🦀"
}

pub fn accepts_mut_string(value: &mut String) -> bool {
    value.push_str(" mutated");
    value == "mutable 🦀 mutated"
}

pub fn identity_string(value: String) -> String {
    value
}

pub fn return_string() -> String {
    String::from("returned 🦀")
}
