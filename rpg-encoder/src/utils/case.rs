fn capitalize_words(s: &str, separator: &str) -> String {
    s.split(['_', '-', '/'])
        .map(|word| {
            let mut chars = word.bytes();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut result = String::with_capacity(word.len());
                    result.push(first.to_ascii_uppercase() as char);
                    for b in chars {
                        result.push(b.to_ascii_lowercase() as char);
                    }
                    result
                }
            }
        })
        .collect::<Vec<_>>()
        .join(separator)
}

pub fn to_title_case(s: &str) -> String {
    capitalize_words(s, " ")
}

pub fn to_pascal_case(s: &str) -> String {
    capitalize_words(s, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_title_case_underscore() {
        assert_eq!(to_title_case("hello_world"), "Hello World");
    }

    #[test]
    fn test_to_title_case_hyphen() {
        assert_eq!(to_title_case("my-component"), "My Component");
    }

    #[test]
    fn test_to_title_case_slash() {
        assert_eq!(to_title_case("src/lib/module"), "Src Lib Module");
    }

    #[test]
    fn test_to_title_case_single_word() {
        assert_eq!(to_title_case("hello"), "Hello");
    }

    #[test]
    fn test_to_title_case_empty() {
        assert_eq!(to_title_case(""), "");
    }

    #[test]
    fn test_to_title_case_mixed_case_input() {
        assert_eq!(to_title_case("HELLO_WORLD"), "Hello World");
    }

    #[test]
    fn test_to_pascal_case_underscore() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
    }

    #[test]
    fn test_to_pascal_case_hyphen() {
        assert_eq!(to_pascal_case("my-component"), "MyComponent");
    }

    #[test]
    fn test_to_pascal_case_single_word() {
        assert_eq!(to_pascal_case("hello"), "Hello");
    }

    #[test]
    fn test_to_pascal_case_empty() {
        assert_eq!(to_pascal_case(""), "");
    }
}
