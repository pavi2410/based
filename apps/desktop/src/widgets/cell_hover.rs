//! One hover card shared by every grid: it follows the cursor instead of re-opening per cell.
//!
//! Cells report the pointer to a global model; a single host view draws the card with its
//! top-left corner at the cursor's tail. Moving from cell to cell updates the card in place.

use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    Anchor, AnyElement, App, Context, Entity, Global, InteractiveElement, IntoElement, Pixels,
    Point, Render, StatefulInteractiveElement, Window, anchored, deferred, div, point, prelude::*,
    px,
};

use crate::widgets::{dt_hover, json_hover};

const SHOW_DELAY: Duration = Duration::from_millis(350);
const LEAVE_GRACE: Duration = Duration::from_millis(90);
/// Card corner sits this far from the pointer so the arrow does not cover it.
const CURSOR_TAIL: (f32, f32) = (4.0, 6.0);

/// What the card shows.
#[derive(Clone, PartialEq, Eq)]
pub enum HoverContent {
    Datetime(String),
    Json(String),
}

impl HoverContent {
    fn body(&self, cx: &App) -> Option<AnyElement> {
        match self {
            Self::Datetime(raw) => dt_hover::body(raw, cx),
            Self::Json(raw) => json_hover::body(raw, cx),
        }
    }

    /// Whether there is anything to show for this cell.
    pub fn available(&self, cx: &App) -> bool {
        self.body(cx).is_some()
    }
}

#[derive(Default)]
pub struct CellHoverModel {
    target: Option<(u64, HoverContent)>,
    pos: Point<Pixels>,
    visible: bool,
    epoch: u64,
    leaving: bool,
}

impl CellHoverModel {
    fn hover(
        &mut self,
        id: u64,
        content: HoverContent,
        pos: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.leaving = false;
        self.pos = pos;
        let changed = self.target.as_ref().map(|(t, _)| *t) != Some(id);
        if changed {
            self.target = Some((id, content));
            self.epoch += 1;
            if !self.visible {
                let epoch = self.epoch;
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(SHOW_DELAY).await;
                    let _ = this.update(cx, |m, cx| {
                        if m.epoch == epoch && m.target.is_some() {
                            m.visible = true;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    fn leave(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.target.as_ref().map(|(t, _)| *t) != Some(id) {
            return;
        }
        self.leaving = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LEAVE_GRACE).await;
            let _ = this.update(cx, |m, cx| {
                if m.leaving && m.target.as_ref().map(|(t, _)| *t) == Some(id) {
                    m.target = None;
                    m.visible = false;
                    m.epoch += 1;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

struct CellHoverGlobal {
    model: Entity<CellHoverModel>,
    host: Entity<CellHoverHost>,
}

impl Global for CellHoverGlobal {}

/// Create the shared model and host view; call once at start-up.
pub fn init(cx: &mut App) {
    let model = cx.new(|_| CellHoverModel::default());
    let host = cx.new(|cx| {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        CellHoverHost {
            model: model.clone(),
        }
    });
    cx.set_global(CellHoverGlobal { model, host });
}

/// The view to place once in a window's element tree; empty when nothing is hovered.
pub fn host(cx: &App) -> Option<AnyElement> {
    cx.try_global::<CellHoverGlobal>()
        .map(|g| g.host.clone().into_any_element())
}

/// Make `cell` report the pointer so the shared card shows `content` for it.
pub fn track<E>(cell: E, id: u64, content: HoverContent) -> AnyElement
where
    E: InteractiveElement + StatefulInteractiveElement + IntoElement + 'static,
{
    cell.on_mouse_move(move |event, _, cx| {
        let Some(model) = cx.try_global::<CellHoverGlobal>().map(|g| g.model.clone()) else {
            return;
        };
        let content = content.clone();
        let position = event.position;
        model.update(cx, |m, cx| m.hover(id, content, position, cx));
    })
    .on_hover(move |hovered, _, cx| {
        if *hovered {
            return;
        }
        if let Some(model) = cx.try_global::<CellHoverGlobal>().map(|g| g.model.clone()) {
            model.update(cx, |m, cx| m.leave(id, cx));
        }
    })
    .into_any_element()
}

struct CellHoverHost {
    model: Entity<CellHoverModel>,
}

impl Render for CellHoverHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let Some((_, content)) = m.target.as_ref().filter(|_| m.visible) else {
            return div().into_any_element();
        };
        let Some(body) = content.body(cx) else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        deferred(
            anchored()
                .position(m.pos)
                .offset(point(px(CURSOR_TAIL.0), px(CURSOR_TAIL.1)))
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.0))
                .child(
                    div()
                        .p_3()
                        .rounded(theme.radius)
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.popover)
                        .text_color(theme.popover_foreground)
                        .shadow_md()
                        .child(body),
                ),
        )
        .with_priority(1000)
        .into_any_element()
    }
}
