//! Engine-agnostic table page, as returned by headless engine crates.

/// One page of a table browse. `columns` is `None` when the page is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawPage {
    pub total: i64,
    pub columns: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
}

/// Quote a SQL identifier (`a"b` → `"a""b"`).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_embedded_quotes() {
        assert_eq!(quote_ident(r#"a"b"#), r#""a""b""#);
    }
}
