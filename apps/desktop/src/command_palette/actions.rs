use gpui::{App, KeyBinding, actions};

actions!(command_palette, [PaletteSelectUp, PaletteSelectDown]);

#[derive(Clone, PartialEq, Eq, serde::Deserialize, gpui::Action)]
#[action(namespace = command_palette, no_json)]
pub(super) struct PaletteConfirm {
    pub secondary: bool,
}

pub fn init(cx: &mut App) {
    let ctx = Some("CommandPalette");
    cx.bind_keys([
        KeyBinding::new("up", PaletteSelectUp, ctx),
        KeyBinding::new("down", PaletteSelectDown, ctx),
        KeyBinding::new("enter", PaletteConfirm { secondary: false }, ctx),
        KeyBinding::new("secondary-enter", PaletteConfirm { secondary: true }, ctx),
    ]);
}
