//! Built-in workspace panels (not engine-specific).

pub mod connection_wizard;
#[cfg(any(test, feature = "test-support"))]
pub mod data_view;
pub mod data_viewer;
pub mod home;
pub mod inspector;
pub mod object_info;
pub mod query_ai;
pub mod query_editor;
#[cfg(any(test, feature = "test-support"))]
pub mod query_view;
pub mod release_notes;
pub mod table_inspector;

pub use connection_wizard::ConnectionWizardPanel;
pub use home::HomePanel;
