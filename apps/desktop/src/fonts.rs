//! Bundled JetBrains Mono and IBM Plex Mono (dev phase — no license files yet).

use std::borrow::Cow;

use gpui::App;

/// Register bundled monospace faces with the GPUI text system.
pub fn register_bundled_fonts(cx: &App) {
    let blobs: [&[u8]; 4] = [
        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
        include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf"),
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
        include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
    ];
    let fonts: Vec<Cow<'static, [u8]>> = blobs.into_iter().map(Cow::Borrowed).collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        log::warn!("register bundled fonts: {err:#}");
    }
}
