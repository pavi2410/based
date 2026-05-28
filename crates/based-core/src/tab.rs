use serde::{Deserialize, Serialize};

use crate::connection_id::ConnectionId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId {
    pub conn: ConnectionId,
    pub kind: TabKind,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabKind {
    DataViewer,
    QueryEditor,
    SchemaInspector,
    ExplainView,
    PragmaBrowser,
    EqpViewer,
    FtsConsole,
    PipelineBuilder,
    ChangeStream,
    LiveMonitor,
}
