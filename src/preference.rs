//! Preference sentence detection for improved agent memory recall.
//!
//! Drawers containing user preferences are tagged at write time so they can be
//! surfaced by a dedicated search pass even when BM25 has no keyword overlap.

static PREFERENCE_PATTERNS: &[&str] = &[
    "i prefer ",
    "i like ",
    "i always ",
    "i never ",
    "i want ",
    "i dislike ",
    "i avoid ",
    "i hate ",
    "i love ",
    "i tend to ",
    "i usually ",
    "i typically ",
    "i find it ",
    "i'm comfortable",
    "i'm not comfortable",
    "i am comfortable",
    "i am not comfortable",
    "in my experience",
    "my convention",
    "my preference",
    "my style",
    "my approach is",
    "always use ",
    "never use ",
    "prefer to use",
    "prefer to write",
    "prefer to keep",
    "prefer to avoid",
    "don't like ",
    "do not like ",
    "would rather ",
    "feel strongly",
    "strongly prefer",
    "it's important to me",
    "it is important to me",
];

/// Return true if `text` contains a preference or personal convention signal.
pub fn is_preference(text: &str) -> bool {
    let lower = text.to_lowercase();
    PREFERENCE_PATTERNS.iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_basic_preferences() {
        assert!(is_preference("I prefer using snake_case for variables"));
        assert!(is_preference("I always add tests before shipping"));
        assert!(is_preference("I never commit directly to main"));
        assert!(is_preference(
            "My convention is to use Result for error handling"
        ));
    }

    #[test]
    fn ignores_neutral_text() {
        assert!(!is_preference("The function returns a vector of strings"));
        assert!(!is_preference("cargo fmt runs the formatter"));
        assert!(!is_preference("This was fixed in commit abc123"));
    }
}
