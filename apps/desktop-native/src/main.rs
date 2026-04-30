// based — native GPUI desktop client
//
// Module layout mirrors the plan's repo structure.  Stubs for every module
// are declared here so `cargo check` validates the tree even before each
// phase fills in real implementations.

mod app;
mod connection;
mod mongodb;
mod postgres;
mod project;
mod settings_window;
mod sqlite;
mod widgets;
mod workspace;

use gpui::*;
use gpui_component::{Root, button::*, *};

// ── Phase-0 hello-world ──────────────────────────────────────────────────────
// This view is replaced by workspace::Workspace in Phase 1.

struct HelloBased;

impl Render for HelloBased {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_4()
            .size_full()
            .items_center()
            .justify_center()
            .child(div().text_xl().font_bold().child("based"))
            .child(div().text_sm().child("Git-Friendly Database Client"))
            .child(
                Button::new("open-project")
                    .primary()
                    .label("Open Project")
                    .on_click(|_, _, _| eprintln!("Open Project — Phase 1 will wire this")),
            )
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(100.0), px(100.0)),
                            size: size(px(1280.0), px(800.0)),
                        })),
                        titlebar: Some(TitlebarOptions {
                            title: Some("based".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|_| HelloBased);
                        cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                    },
                )
                .expect("Failed to open main window");
            })
            .detach();
        });
}
