pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let prefix: String = s.chars().take(max_len - 3).collect();
    format!("{prefix}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_longer() {
        let s = "hello world";
        let max_len = 8;
        let truncated = truncate_string(s, max_len);
        assert_eq!(truncated, "hello...");
    }

    #[test]
    fn test_truncate_shorter() {
        let s = "hello";
        let max_len = 10;
        let truncated = truncate_string(s, max_len);
        assert_eq!(truncated, "hello");
    }
}
