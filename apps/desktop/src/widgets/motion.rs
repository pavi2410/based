//! Shared motion primitives: a skeleton table, pulsing elements, rise-in, glimmer bar, spring pop.
//! Skeleton blocks and shimmering text come from gpui-kit (`Skeleton`, `ShimmerText`).
//!
//! GPUI divs cannot be transformed, so effects are built from opacity, position
//! offsets and two-stop gradients. All colors derive from the active theme.

use std::time::Duration;

use gpui_kit::component::{ActiveTheme, h_flex, skeleton::Skeleton, v_flex};
use gpui_kit::{
    Animation, AnimationExt, AnyElement, App, ElementId, IntoElement, SpringConfig, Styled, div,
    linear_color_stop, linear_gradient, prelude::*, px, relative, sampled_easing,
};

/// Entry transitions.
pub const RISE: Duration = Duration::from_millis(320);
/// Delay between staggered siblings.
pub const STAGGER_MS: u64 = 45;

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Gentle opacity breathing for in-flight status text ("Connecting…").
pub fn pulse<E>(id: impl Into<ElementId>, el: E) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(1400))
            .repeat()
            .with_easing(gpui_kit::pulsating_between(0.45, 1.0)),
        |el, v| el.opacity(v),
    )
    .into_any_element()
}

/// Fade and rise into place; `index` staggers siblings. Plays once per `id`.
pub fn rise_in(id: impl Into<ElementId>, index: usize, child: impl IntoElement) -> AnyElement {
    let delay = (index as u64 * STAGGER_MS) as f32;
    let total = RISE.as_millis() as f32 + delay;
    div()
        .relative()
        .child(child)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(total as u64)).with_easing(ease_out_cubic),
            move |el, t| {
                let p = ((t * total - delay) / RISE.as_millis() as f32).clamp(0.0, 1.0);
                let p = ease_out_cubic(p);
                el.opacity(p).top(px((1.0 - p) * 6.0))
            },
        )
        .into_any_element()
}

/// Indeterminate progress sliver with a travelling glow. Reserve its 2px height always.
pub fn glimmer_bar(id: impl Into<ElementId>, active: bool, cx: &App) -> AnyElement {
    let track = div().relative().h(px(2.0)).w_full().overflow_hidden();
    if !active {
        return track.into_any_element();
    }
    let color = cx.theme().primary;
    let clear = color.opacity(0.0);
    track
        .child(div().size_full().with_animation(
            id,
            Animation::new(Duration::from_millis(1300)).repeat(),
            move |el, delta| {
                let band = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(-0.35 + delta * 1.35))
                    .w(relative(0.35))
                    .flex()
                    .child(div().h_full().w_1_2().bg(linear_gradient(
                        90.0,
                        linear_color_stop(clear, 0.0),
                        linear_color_stop(color, 1.0),
                    )))
                    .child(div().h_full().w_1_2().bg(linear_gradient(
                        90.0,
                        linear_color_stop(color, 0.0),
                        linear_color_stop(clear, 1.0),
                    )));
                el.child(band)
            },
        ))
        .into_any_element()
}

/// Spring easing with overshoot, settled duration included.
pub fn spring_animation() -> Animation {
    let (dur, ease) = sampled_easing(SpringConfig::new(260.0, 15.0, 1.0), 0.002);
    Animation::new(dur).with_easing(ease)
}

/// Spring scale factor (overshoots 1.0) for use with an `Icon` transformation.
pub fn pop(delta: f32) -> f32 {
    0.4 + 0.6 * delta
}

/// Grid placeholder: a header strip and `rows` rows of shimmering cells.
pub fn skeleton_table(rows: usize, cols: usize, cx: &App) -> impl IntoElement {
    const W: [f32; 5] = [0.7, 0.45, 0.85, 0.55, 0.65];
    let theme = cx.theme();
    v_flex()
        .size_full()
        .bg(theme.background)
        .child(
            div()
                .h(px(30.0))
                .w_full()
                .border_b_1()
                .border_color(theme.border)
                .bg(theme.muted.opacity(0.35)),
        )
        .children((0..rows).map(|r| {
            h_flex()
                .h(px(32.0))
                .px(px(10.0))
                .gap(px(24.0))
                .items_center()
                .children((0..cols).map(move |c| {
                    let w = W[(r + c * 2) % W.len()];
                    div().flex_1().child(
                        Skeleton::new()
                            .w(px(120.0 * w))
                            .h(px(10.0))
                            .rounded(px(4.0)),
                    )
                }))
        }))
}
