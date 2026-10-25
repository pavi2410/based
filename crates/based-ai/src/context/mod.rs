//! Schema context sent to the model: engine-agnostic snapshot plus a compact renderer.

mod render;
mod snapshot;

pub use render::{RenderOptions, render};
pub use snapshot::{ContextLevel, Entity, EntityKind, Field, SchemaSnapshot, SchemaSource};
