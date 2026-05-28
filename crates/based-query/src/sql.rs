//! SQL statement boundaries for run-current / run-script (P0 baseline).

/// Byte range of a single SQL statement in the source script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlStatement {
    pub start: usize,
    pub end: usize,
}

impl SqlStatement {
    pub fn text<'a>(&self, script: &'a str) -> &'a str {
        script[self.start..self.end].trim()
    }
}

/// Split `script` into statements delimited by `;` outside single-quoted strings.
pub fn statements_in_script(script: &str) -> Vec<SqlStatement> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let bytes = script.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string && b == b';' {
            let end = i;
            let slice = script[start..end].trim();
            if !slice.is_empty() {
                out.push(SqlStatement { start, end });
            }
            start = i + 1;
        }
        i += 1;
    }
    let tail = script[start..].trim();
    if !tail.is_empty() {
        out.push(SqlStatement {
            start,
            end: script.len(),
        });
    }
    out
}

/// Statement containing `offset`, or the next non-empty statement after `offset`.
pub fn statement_at_offset(script: &str, offset: usize) -> Option<SqlStatement> {
    let stmts = statements_in_script(script);
    if stmts.is_empty() {
        return None;
    }
    for s in &stmts {
        if offset >= s.start && offset <= s.end {
            return Some(*s);
        }
    }
    stmts
        .iter()
        .find(|s| s.start > offset)
        .copied()
        .or_else(|| stmts.last().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_two_statements() {
        let script = "SELECT 1;\nSELECT 2;";
        let stmts = statements_in_script(script);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0].text(script), "SELECT 1");
        assert_eq!(stmts[1].text(script), "SELECT 2");
    }

    #[test]
    fn semicolon_in_string_ignored() {
        let script = "SELECT ';' AS x; SELECT 2;";
        let stmts = statements_in_script(script);
        assert_eq!(stmts.len(), 2);
    }
}
