//! Unified-diff helper for showing a proposed edit against the current text.

use similar::TextDiff;

/// Unified patch turning `old` into `new` for `path`, or `None` when identical.
pub fn unified(path: &str, old: &str, new: &str) -> Option<String> {
    if old == new {
        return None;
    }
    let patch = TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string();
    Some(format!("diff --git a/{path} b/{path}\n{patch}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_has_no_patch() {
        assert_eq!(unified("q.sql", "SELECT 1\n", "SELECT 1\n"), None);
    }

    #[test]
    fn patch_has_headers_and_changes() {
        let p = unified("q.sql", "SELECT a\nFROM t\n", "SELECT b\nFROM t\n").unwrap();
        assert!(p.starts_with("diff --git a/q.sql b/q.sql\n--- a/q.sql\n+++ b/q.sql\n"));
        assert!(p.contains("-SELECT a\n"));
        assert!(p.contains("+SELECT b\n"));
        assert!(p.contains(" FROM t\n"));
    }

    #[test]
    fn empty_old_is_all_additions() {
        let p = unified("q.sql", "", "SELECT 1\n").unwrap();
        assert!(p.contains("+SELECT 1\n"));
        assert!(
            !p.lines()
                .any(|l| l.starts_with('-') && !l.starts_with("---"))
        );
    }
}
