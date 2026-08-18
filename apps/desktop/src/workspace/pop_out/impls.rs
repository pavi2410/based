//! Blanket [`PopOutWindowTitle`] impls for panels that use the default `panel_name` title.

use super::PopOutWindowTitle;
use crate::mongodb::change_stream::ChangeStreamPanel;
use crate::mongodb::document_editor::DocumentEditorPanel;
use crate::mongodb::document_viewer::DocumentViewerPanel;
use crate::mongodb::inspector::CollectionInspectorPanel;
use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::mongodb::tree::CollectionsTreePanel;
use crate::postgres::inspector::TableInspectorPanel as PgInspectorPanel;
use crate::postgres::tree::SchemaTreePanel as PgSchemaTreePanel;
use crate::sqlite::fts_console::FtsConsolePanel;
use crate::sqlite::inspector::TableInspectorPanel as SqliteInspectorPanel;
use crate::sqlite::tree::SchemaTreePanel as SqliteSchemaTreePanel;
use crate::workspace::panels::connection_wizard::ConnectionWizardPanel;
use crate::workspace::panels::home::HomePanel;
use crate::workspace::panels::object_info::{ConnectionDashboardPanel, ObjectInfoPanel};
use crate::workspace::panels::release_notes::ReleaseNotesPanel;

impl PopOutWindowTitle for HomePanel {}
impl PopOutWindowTitle for ConnectionWizardPanel {}
impl PopOutWindowTitle for ReleaseNotesPanel {}
impl PopOutWindowTitle for ConnectionDashboardPanel {}
impl PopOutWindowTitle for ObjectInfoPanel {}
impl PopOutWindowTitle for ChangeStreamPanel {}
impl PopOutWindowTitle for DocumentEditorPanel {}
impl PopOutWindowTitle for DocumentViewerPanel {}
impl PopOutWindowTitle for CollectionInspectorPanel {}
impl PopOutWindowTitle for PipelineBuilderPanel {}
impl PopOutWindowTitle for CollectionsTreePanel {}
impl PopOutWindowTitle for PgInspectorPanel {}
impl PopOutWindowTitle for PgSchemaTreePanel {}
impl PopOutWindowTitle for FtsConsolePanel {}
impl PopOutWindowTitle for SqliteInspectorPanel {}
impl PopOutWindowTitle for SqliteSchemaTreePanel {}
