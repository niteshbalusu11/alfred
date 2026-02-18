use shared::llm::safety::sanitize_untrusted_text;

pub(super) fn is_small_talk_fast_path_query(query: &str) -> bool {
    let normalized = normalize_small_talk_query(query);
    matches!(
        normalized.as_str(),
        "hi" | "hello"
            | "hey"
            | "how are you"
            | "how are you doing"
            | "how are you doing today"
            | "hi how are you"
            | "hello how are you"
            | "hey how are you"
            | "whats up"
            | "what s up"
            | "sup"
            | "yo"
    )
}

fn normalize_small_talk_query(query: &str) -> String {
    sanitize_untrusted_text(query)
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::is_small_talk_fast_path_query;

    #[test]
    fn small_talk_fast_path_matches_common_greetings() {
        assert!(is_small_talk_fast_path_query("Hey"));
        assert!(is_small_talk_fast_path_query("how are you doing today?"));
        assert!(is_small_talk_fast_path_query("what's up"));
        assert!(!is_small_talk_fast_path_query(
            "hey can you plan my Alaska trip"
        ));
    }
}
