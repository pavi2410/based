//! Single-line display helpers for palette result labels.

/// Collapse whitespace (including newlines/tabs) and truncate for one-line display.
pub fn palette_single_line(text: &str, max_chars: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let mut out: String = collapsed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_whitespace_and_newlines() {
        assert_eq!(
            palette_single_line("SELECT\n  *\nFROM\tusers", 200),
            "SELECT * FROM users"
        );
    }

    #[test]
    fn truncates_with_ellipsis() {
        let long = "SELECT * FROM very_long_table_name WHERE id = 1";
        assert!(palette_single_line(long, 20).ends_with('…'));
        assert!(palette_single_line(long, 20).chars().count() <= 20);
    }
}
