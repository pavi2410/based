//! Export popover — CSV / Excel download trigger for any tabular data view.

use gpui::{prelude::*, *};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    popover::Popover,
    v_flex,
};

use crate::widgets::export;

/// Trigger button + popover menu for CSV and Excel export.
///
/// `id_prefix` is used as a namespace for all element IDs inside the popover
/// (e.g. `"pg-qe"` → `"pg-qe-export-popover"`, `"pg-qe-export-csv"`, …).
pub fn export_popover(
    id_prefix: &'static str,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
) -> impl IntoElement {
    let (h, r) = (headers.clone(), rows.clone());
    let (h2, r2) = (headers, rows);
    Popover::new(SharedString::from(format!("{id_prefix}-export-popover")))
        .trigger(
            Button::new(SharedString::from(format!("{id_prefix}-export-trigger")))
                .ghost()
                .small()
                .label("Export"),
        )
        .content(move |_, _, _| {
            let (hc, rc) = (h.clone(), r.clone());
            let (hx, rx) = (h2.clone(), r2.clone());
            v_flex()
                .gap(px(2.0))
                .p(px(4.0))
                .child(
                    Button::new(SharedString::from(format!("{id_prefix}-export-csv")))
                        .ghost()
                        .small()
                        .label("CSV")
                        .on_click(move |_, _, cx| {
                            let (hc, rc) = (hc.clone(), rc.clone());
                            cx.spawn(async move |cx| {
                                if let Ok(bytes) = export::to_csv(&hc, &rc)
                                    && let Some(path) =
                                        export::save_bytes(cx, "export.csv", "CSV", &["csv"], bytes)
                                            .await
                                {
                                    cx.update(|app| {
                                        crate::workspace::notify::push_export_success(app, &path)
                                    });
                                }
                            })
                            .detach();
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("{id_prefix}-export-xlsx")))
                        .ghost()
                        .small()
                        .label("Excel (.xlsx)")
                        .on_click(move |_, _, cx| {
                            let (hx, rx) = (hx.clone(), rx.clone());
                            cx.spawn(async move |cx| {
                                if let Ok(bytes) = export::to_xlsx(&hx, &rx)
                                    && let Some(path) = export::save_bytes(
                                        cx,
                                        "export.xlsx",
                                        "Excel",
                                        &["xlsx"],
                                        bytes,
                                    )
                                    .await
                                {
                                    cx.update(|app| {
                                        crate::workspace::notify::push_export_success(app, &path)
                                    });
                                }
                            })
                            .detach();
                        }),
                )
        })
}
