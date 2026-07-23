/// Heart on My Sleeve badge challenge
/// Adding a meaningful module with a heart emoji
pub fn heart() -> &'static str {
    "?? This project was built with love for the Rust community"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heart() {
        let msg = heart();
        assert!(msg.contains("??"));
        assert!(msg.len() > 20);
    }

    #[test]
    fn test_heart_not_empty() {
        assert!(!heart().is_empty());
    }

    #[test]
    fn test_heart_contains_love() {
        assert!(heart().to_lowercase().contains("love"));
    }

    #[test]
    fn test_heart_contains_rust() {
        assert!(heart().to_lowercase().contains("rust"));
    }

    #[test]
    fn test_heart_has_emoji() {
        assert!(heart().chars().any(|c| c as u32 > 1000));
    }
}
