//! "Explain this column": the question handed to the assistant from a grid header.

use crate::widgets::column_header::ColumnMeta;
use crate::widgets::virtual_table::Cell;

/// Distinct sample values shown to the model, and the longest one kept whole.
const SAMPLES: usize = 8;
const SAMPLE_CHARS: usize = 60;

/// Up to [`SAMPLES`] distinct, non-null values of column `col`, in row order.
/// An empty string is a value, shown as `""`.
pub fn distinct_samples(rows: &[Vec<Cell>], col: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cell in rows.iter().filter_map(|r| r.get(col)) {
        if cell.is_null() {
            continue;
        }
        let clipped: String = if cell.display.is_empty() {
            "\"\"".into()
        } else {
            cell.display.chars().take(SAMPLE_CHARS).collect()
        };
        if !out.contains(&clipped) {
            out.push(clipped);
        }
        if out.len() == SAMPLES {
            break;
        }
    }
    out
}

/// The question for `column` of `table`. `samples` is empty unless the connection shares rows.
pub fn column_prompt(table: &str, column: &str, meta: &ColumnMeta, samples: &[String]) -> String {
    let mut facts = Vec::new();
    if let Some(ty) = meta.data_type.as_deref().filter(|t| !t.is_empty()) {
        facts.push(format!("type {ty}"));
    }
    if meta.is_primary_key {
        facts.push("primary key".to_owned());
    }
    if let Some(target) = &meta.fk_target {
        facts.push(format!("references {target}"));
    }
    if meta.nullable == Some(false) {
        facts.push("not null".to_owned());
    }
    let mut text = format!("Explain the column `{column}` of `{table}`");
    if !facts.is_empty() {
        text.push_str(&format!(" ({})", facts.join(", ")));
    }
    text.push_str(". What does it most likely store, and how is it used?");
    if !samples.is_empty() {
        text.push_str(&format!("\nSample values: {}", samples.join(" | ")));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_distinct_and_skip_nulls() {
        let rows: Vec<Vec<Cell>> = [
            Cell::text("a"),
            Cell::new(based_core::Value::Null),
            Cell::text("a"),
            Cell::text("NULL"),
            Cell::text(""),
            Cell::text("b"),
        ]
        .into_iter()
        .map(|c| vec![c])
        .collect();
        assert_eq!(distinct_samples(&rows, 0), ["a", "NULL", "\"\"", "b"]);
        assert!(distinct_samples(&rows, 3).is_empty());
    }

    #[test]
    fn prompt_carries_constraints_and_samples() {
        let meta = ColumnMeta {
            data_type: Some("int4".into()),
            is_foreign_key: true,
            fk_target: Some("public.users(id)".into()),
            nullable: Some(false),
            ..Default::default()
        };
        let p = column_prompt("public.orders", "user_id", &meta, &["1".into(), "2".into()]);
        assert!(p.contains("`user_id` of `public.orders`"));
        assert!(p.contains("type int4, references public.users(id), not null"));
        assert!(p.ends_with("Sample values: 1 | 2"));
        let bare = column_prompt("t", "c", &ColumnMeta::default(), &[]);
        assert!(!bare.contains('(') && !bare.contains("Sample"));
    }
}
