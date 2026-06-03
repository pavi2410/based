//! Engine identity — labels, colors, icons, and chips for each database engine.

use gpui::{img, prelude::*, *};
use gpui_component::h_flex;

use crate::app::prefs;
use crate::connection::EngineKind;

pub fn engine_label(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "PG",
        EngineKind::MongoDB => "MDB",
        EngineKind::SQLite => "SQL",
    }
}

pub fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "Postgres",
        EngineKind::MongoDB => "MongoDB",
        EngineKind::SQLite => "SQLite",
    }
}

pub fn engine_color(engine: EngineKind) -> Hsla {
    match engine {
        EngineKind::Postgres => hsla(0.58, 0.78, 0.64, 1.0),
        EngineKind::MongoDB => hsla(0.38, 0.62, 0.54, 1.0),
        EngineKind::SQLite => hsla(0.10, 0.74, 0.59, 1.0),
    }
}

fn engine_icon_path(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "icons/engines/postgresql.svg",
        EngineKind::MongoDB => "icons/engines/mongodb.svg",
        EngineKind::SQLite => "icons/engines/sqlite.svg",
    }
}

/// Devicon colored brand mark (rasterized via `img` so fills are not mono-tinted).
pub fn engine_icon(engine: EngineKind) -> impl IntoElement {
    img(engine_icon_path(engine)).size(px(16.0)).flex_shrink_0()
}

/// Compact engine label for sidebar rows (no border or fill).
pub fn engine_label_inline(engine: EngineKind, cx: &mut App) -> impl IntoElement {
    let color = engine_color(engine);
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .font_family(prefs::ui_font_family(cx))
        .font_weight(prefs::ui_font_weight(cx))
        .text_color(color)
        .child(engine_label(engine))
}

pub fn engine_chip(engine: EngineKind, cx: &mut App) -> impl IntoElement {
    let color = engine_color(engine);
    h_flex()
        .h(px(18.0))
        .px(px(6.0))
        .gap(px(4.0))
        .items_center()
        .rounded(px(4.0))
        .bg(color.opacity(0.13))
        .border_1()
        .border_color(color.opacity(0.18))
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(color.opacity(0.9)),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(color)
                .child(engine_label(engine)),
        )
}
