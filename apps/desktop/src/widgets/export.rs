// widgets/export.rs — serialise RowDelegate snapshots to CSV, Excel, or JSON
// and save them to a user-chosen path via an rfd save-file dialog.

use anyhow::Context as _;
use gpui::AsyncApp;
use rust_xlsxwriter::{Format, Workbook};

use crate::widgets::virtual_table::NULL_CELL_DISPLAY;

// ── Serialisers ──────────────────────────────────────────────────────────────

/// Serialise tabular data to CSV bytes.
///
/// Cells that display as `"NULL"` (or are empty) are written as empty fields
/// so spreadsheet tools don't receive the literal string "NULL".
pub fn to_csv(headers: &[String], rows: &[Vec<String>]) -> anyhow::Result<Vec<u8>> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(headers)?;
    for row in rows {
        let cells: Vec<&str> = row
            .iter()
            .map(|c| {
                if c.is_empty() || c == NULL_CELL_DISPLAY {
                    ""
                } else {
                    c.as_str()
                }
            })
            .collect();
        wtr.write_record(&cells)?;
    }
    wtr.into_inner().context("csv flush")
}

/// Serialise tabular data to an `.xlsx` workbook buffer.
///
/// Row 0 is a bold header row; data starts at row 1. NULL cells are written
/// as empty strings.
pub fn to_xlsx(headers: &[String], rows: &[Vec<String>]) -> anyhow::Result<Vec<u8>> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    let bold = Format::new().set_bold();
    for (col_ix, header) in headers.iter().enumerate() {
        sheet
            .write_with_format(0, col_ix as u16, header.as_str(), &bold)
            .context("write header")?;
    }

    for (row_ix, row) in rows.iter().enumerate() {
        for (col_ix, cell) in row.iter().enumerate() {
            let val = if cell.is_empty() || cell == NULL_CELL_DISPLAY {
                ""
            } else {
                cell.as_str()
            };
            sheet
                .write(row_ix as u32 + 1, col_ix as u16, val)
                .context("write cell")?;
        }
    }

    workbook.save_to_buffer().context("save xlsx buffer")
}

/// Serialise tabular data to a pretty-printed JSON array of objects.
///
/// Cells that display as `"NULL"` (or are empty) are written as
/// `null` values rather than the string `"NULL"`.
pub fn to_json(headers: &[String], rows: &[Vec<String>]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (col_ix, header) in headers.iter().enumerate() {
                let val = match row.get(col_ix) {
                    Some(c) if !c.is_empty() && c != NULL_CELL_DISPLAY => {
                        serde_json::Value::String(c.clone())
                    }
                    _ => serde_json::Value::Null,
                };
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
) -> Option<std::path::PathBuf> {
    let fname = filename.to_string();
    let filter = filter_name.to_string();
    let exts: Vec<String> = extensions.iter().map(|s| s.to_string()).collect();

    crate::db::run_infallible(cx, async move {
        tokio::task::spawn_blocking(move || -> Option<std::path::PathBuf> {
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
                && path.extension().and_then(|e| e.to_str()) != Some(expected) {
                    path.set_extension(expected);
                }

            if std::fs::write(&path, &bytes).is_ok() {
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
