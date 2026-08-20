//! Single-line display helpers for palette result labels.

/// Trailing meta for a palette row (VS Code style).
pub fn palette_meta(conn_label: &str, sublabel: &str) -> String {
    if conn_label.is_empty() || sublabel.contains(conn_label) {
        sublabel.to_string()
    } else {
        format!("{conn_label} · {sublabel}")
    }
}

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

    #[test]
    fn meta_skips_duplicate_connection_label() {
        assert_eq!(palette_meta("", "history · local"), "history · local");
        assert_eq!(palette_meta("local", "history · local"), "history · local");
        assert_eq!(palette_meta("prod", "table"), "prod · table");
    }
}
