pub fn replace_start(s: String, from: &str, to: &str) -> String {
    if let Some(stripped) = s.strip_prefix(from) {
        format!("{}{}", to, stripped)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_start_matching_prefix() {
        assert_eq!(replace_start("abac".to_string(), "a", "z"), "zbac");
    }

    #[test]
    fn test_replace_start_no_match() {
        assert_eq!(replace_start("bac".to_string(), "a", "z"), "bac");
    }

    #[test]
    fn test_replace_start_empty_string() {
        assert_eq!(replace_start("".to_string(), "a", "z"), "");
    }

    #[test]
    fn test_replace_start_longer_prefix() {
        assert_eq!(
            replace_start("hello world hello there".to_string(), "hello", "hi"),
            "hi world hello there"
        );
    }

    #[test]
    fn test_replace_start_whole_string() {
        assert_eq!(replace_start("abc".to_string(), "abc", "xyz"), "xyz");
    }

    #[test]
    fn test_replace_start_empty_from() {
        assert_eq!(replace_start("abc".to_string(), "", "xyz"), "xyzabc");
    }
}
