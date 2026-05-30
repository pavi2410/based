//! Embedded app assets (engine brand SVGs) chained before gpui-component defaults.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
#[include = "icon.png"]
struct BasedEmbedded;

struct BasedEmbeddedSource;

impl AssetSource for BasedEmbeddedSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        Ok(BasedEmbedded::get(path).map(|f| f.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(BasedEmbedded::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

/// Tries Based assets first, then gpui-component bundled icons.
pub struct ChainedAssets {
    local: BasedEmbeddedSource,
    fallback: gpui_component_assets::Assets,
}

impl ChainedAssets {
    pub fn new() -> Self {
        Self {
            local: BasedEmbeddedSource,
            fallback: gpui_component_assets::Assets::new(""),
        }
    }
}

impl AssetSource for ChainedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Ok(Some(data)) = self.local.load(path) {
            return Ok(Some(data));
        }
        self.fallback.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = self.local.list(path)?;
        let mut rest = self.fallback.list(path)?;
        out.append(&mut rest);
        Ok(out)
    }
}
