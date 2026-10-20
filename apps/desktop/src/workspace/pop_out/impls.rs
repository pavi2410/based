//! Blanket [`PopOutWindowTitle`] impls for panels that use the default `panel_name` title.

use super::PopOutWindowTitle;
use crate::mongodb::change_stream::ChangeStreamPanel;
use crate::mongodb::document_editor::DocumentEditorPanel;
use crate::mongodb::document_viewer::DocumentViewerPanel;
use crate::mongodb::inspector::CollectionInspectorPanel;
use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::workspace::panels::connection_wizard::ConnectionWizardPanel;
use crate::workspace::panels::home::HomePanel;
use crate::workspace::panels::object_info::{ConnectionDashboardPanel, ObjectInfoPanel};
use crate::workspace::panels::release_notes::ReleaseNotesPanel;
use crate::workspace::panels::table_inspector::TableInspectorPanel;

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
impl PopOutWindowTitle for TableInspectorPanel {}
