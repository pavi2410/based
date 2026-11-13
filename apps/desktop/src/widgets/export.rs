// widgets/export.rs — serialise RowDelegate snapshots to CSV, Excel, or JSON
// and save them to a user-chosen path via an rfd save-file dialog.

use anyhow::Context as _;
use based_core::Value;
use gpui_kit::AsyncApp;
use rust_xlsxwriter::{Format, Workbook};

use crate::db;
use crate::widgets::virtual_table::Cell;

use std::fs;
use std::path::PathBuf;

use tokio::task::spawn_blocking;

// ── Serialisers ──────────────────────────────────────────────────────────────

/// Text for a non-null value in a text format. Bytes are `\x…` hex rather than
/// the grid's `<n bytes>`, so the data survives the round trip.
fn export_text(value: &Value) -> String {
    match value {
        Value::Bytes(_) => match value.to_json() {
            serde_json::Value::String(hex) => hex,
            other => other.to_string(),
        },
        Value::Text(s) => s.to_string(),
        other => other.display(),
    }
}

fn push_csv_field(out: &mut String, field: &str, force_quote: bool) {
    let quote = force_quote || field.contains([',', '"', '\n', '\r']);
    if !quote {
        out.push_str(field);
        return;
    }
    out.push('"');
    out.push_str(&field.replace('"', "\"\""));
    out.push('"');
}

/// Serialise tabular data to CSV bytes (RFC 4180, CRLF).
///
/// NULL is an empty field; an empty string is `""`, so the two stay apart.
pub fn to_csv(headers: &[String], rows: &[Vec<Cell>]) -> anyhow::Result<Vec<u8>> {
    let mut out = String::new();
    for (i, h) in headers.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_csv_field(&mut out, h, false);
    }
    out.push_str("\r\n");
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if !cell.is_null() {
                let text = export_text(&cell.value);
                push_csv_field(&mut out, &text, text.is_empty());
            }
        }
        out.push_str("\r\n");
    }
    Ok(out.into_bytes())
}

/// Largest integer an Excel (f64) cell holds exactly.
const XLSX_MAX_EXACT_INT: i64 = 1 << 53;

/// Serialise tabular data to an `.xlsx` workbook buffer.
///
/// Row 0 is a bold header row; data starts at row 1. Numbers and booleans are
/// written as such (unless Excel would round them); NULL cells stay blank.
pub fn to_xlsx(headers: &[String], rows: &[Vec<Cell>]) -> anyhow::Result<Vec<u8>> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    let bold = Format::new().set_bold();
    for (col_ix, header) in headers.iter().enumerate() {
        sheet
            .write_with_format(0, col_ix as u16, header.as_str(), &bold)
            .context("write header")?;
    }

    for (row_ix, row) in rows.iter().enumerate() {
        let r = row_ix as u32 + 1;
        for (col_ix, cell) in row.iter().enumerate() {
            let c = col_ix as u16;
            match &cell.value {
                v if v.is_null() => continue,
                Value::Bool(b) => sheet.write_boolean(r, c, *b).map(|_| ()),
                Value::Int(n) if n.unsigned_abs() <= XLSX_MAX_EXACT_INT as u64 => {
                    sheet.write_number(r, c, *n as f64).map(|_| ())
                }
                Value::Float(f) if f.is_finite() => sheet.write_number(r, c, *f).map(|_| ()),
                Value::Numeric(s) if Value::numeric_fits_f64(s) => {
                    let n: f64 = s.parse().unwrap_or_default();
                    sheet.write_number(r, c, n).map(|_| ())
                }
                v => sheet.write_string(r, c, export_text(v)).map(|_| ()),
            }
            .context("write cell")?;
        }
    }

    workbook.save_to_buffer().context("save xlsx buffer")
}

/// Serialise tabular data to a pretty-printed JSON array of objects, keeping
/// NULL, numbers, booleans and nested JSON typed.
pub fn to_json(headers: &[String], rows: &[Vec<Cell>]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (col_ix, header) in headers.iter().enumerate() {
                let val = row
                    .get(col_ix)
                    .map_or(serde_json::Value::Null, |c| c.value.to_json());
                obj.insert(header.clone(), val);
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default()
}

// ── File dialog + write ───────────────────────────────────────────────────────

/// Open a native save-file dialog, write `bytes` to the chosen path, and
/// return the saved path.
///
/// Both the dialog and the write run inside `spawn_blocking` on Tokio's thread
/// pool — the same pattern as `project/pick.rs`. Using `std::fs::write` (not
/// `tokio::fs::write`) is correct here because we're already on a blocking
/// thread. The extension from `extensions[0]` is appended if the platform
/// omitted it (e.g. Linux GTK dialogs).
///
/// Returns `None` if the user cancelled or the write failed.
pub async fn save_bytes(
    cx: &mut AsyncApp,
    filename: &str,
    filter_name: &str,
    extensions: &[&str],
    bytes: Vec<u8>,
) -> Option<PathBuf> {
    let fname = filename.to_string();
    let filter = filter_name.to_string();
    let exts: Vec<String> = extensions.iter().map(|s| s.to_string()).collect();

    db::run_infallible(cx, async move {
        spawn_blocking(move || -> Option<PathBuf> {
            let exts_ref: Vec<&str> = exts.iter().map(String::as_str).collect();
            let Some(mut path) = rfd::FileDialog::new()
                .set_file_name(&fname)
                .add_filter(&filter, &exts_ref)
                .save_file()
            else {
                return None; // user cancelled
            };

            // Enforce the extension if the platform omitted it.
            if let Some(expected) = exts_ref.first()
                && path.extension().and_then(|e| e.to_str()) != Some(expected)
            {
                path.set_extension(expected);
            }

            if fs::write(&path, &bytes).is_ok() {
                Some(path)
            } else {
                None
            }
        })
        .await
        .ok()
        .flatten()
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Vec<Cell> {
        vec![
            Cell::new(Value::Null),
            Cell::text(""),
            Cell::text("NULL"),
            Cell::text("a,\"b\""),
            Cell::new(Value::Int(42)),
            Cell::new(Value::Bytes(vec![0xde, 0xad].into())),
        ]
    }

    fn headers() -> Vec<String> {
        ["n", "e", "t", "q", "i", "b"].map(String::from).to_vec()
    }

    #[test]
    fn csv_keeps_null_and_empty_apart() {
        let csv = String::from_utf8(to_csv(&headers(), &[row()]).unwrap()).unwrap();
        assert_eq!(
            csv,
            "n,e,t,q,i,b\r\n,\"\",NULL,\"a,\"\"b\"\"\",42,\\xdead\r\n"
        );
    }

    #[test]
    fn json_keeps_types() {
        let json: serde_json::Value = serde_json::from_str(&to_json(&headers(), &[row()])).unwrap();
        assert_eq!(
            json,
            serde_json::json!([{
                "n": null, "e": "", "t": "NULL", "q": "a,\"b\"", "i": 42, "b": "\\xdead"
            }])
        );
    }

    #[test]
    fn xlsx_writes() {
        assert!(!to_xlsx(&headers(), &[row()]).unwrap().is_empty());
    }
}
