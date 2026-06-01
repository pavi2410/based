//! Engine descriptor trait and registry.
//!
//! Each engine module exports a zero-size struct implementing [`EngineDescriptor`].
//! The registry is a GPUI global populated at startup — new engines register here
//! without touching any central dispatch file.

use based_core::EngineKind;
use gpui::Global;

/// Metadata describing a database engine family.
pub trait EngineDescriptor: Send + Sync + 'static {
    fn kind(&self) -> EngineKind;
    fn display_name(&self) -> &str;
    /// Short icon identifier matched by the theme asset loader.
    fn icon_name(&self) -> &str;
    /// Default TCP port for new connection forms, `None` for file-based engines.
    fn default_port(&self) -> Option<u16>;
    /// Whether this engine supports a given tab kind label.
    fn supports_tab_kind(&self, kind: &str) -> bool;
}

/// App-level registry of all registered engine descriptors.
pub struct EngineRegistry {
    descriptors: Vec<Box<dyn EngineDescriptor>>,
}

impl Global for EngineRegistry {}

impl EngineRegistry {
    pub fn new() -> Self {
        Self {
            descriptors: vec![],
        }
    }

    pub fn register(&mut self, descriptor: impl EngineDescriptor) {
        self.descriptors.push(Box::new(descriptor));
    }

    pub fn find(&self, kind: EngineKind) -> Option<&dyn EngineDescriptor> {
        self.descriptors
            .iter()
            .find(|d| d.kind() == kind)
            .map(|d| d.as_ref())
    }

    pub fn all(&self) -> &[Box<dyn EngineDescriptor>] {
        &self.descriptors
    }

    pub fn display_name(&self, kind: EngineKind) -> &str {
        self.find(kind)
            .map(|d| d.display_name())
            .unwrap_or("Unknown")
    }

    pub fn icon_name(&self, kind: EngineKind) -> &str {
        self.find(kind).map(|d| d.icon_name()).unwrap_or("")
    }
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}
