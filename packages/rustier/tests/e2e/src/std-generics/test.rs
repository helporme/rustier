pub fn optional_number(present: bool) -> Option<i32> {
    present.then_some(42)
}

pub fn checked_number(valid: bool) -> Result<i32, String> {
    if valid {
        Ok(42)
    } else {
        Err(String::from("invalid"))
    }
}

pub fn words() -> Vec<String> {
    vec![String::from("first")]
}

pub fn nested_words() -> Vec<Vec<String>> {
    vec![words()]
}

pub fn word_count(words: &Vec<String>) -> u32 {
    words.len() as u32
}

pub fn nested_word_count(words: &Vec<Vec<String>>) -> u32 {
    words.iter().map(|group| group.len() as u32).sum()
}
