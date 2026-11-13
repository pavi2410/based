use gpui_kit::Context;

use super::CommandPalette;
use super::types::{PaletteEvent, PaletteResult};

pub fn event_for_selection(entry: &PaletteResult) -> PaletteEvent {
    entry.action.clone()
}

pub fn emit_selection(entry: &PaletteResult, cx: &mut Context<CommandPalette>) {
    cx.emit(event_for_selection(entry));
}

#[cfg(test)]
mod tests {
    use crate::connection::ConnectionId;
    use crate::workspace::TabSpec;

    use super::super::types::{PaletteEvent, PaletteResult, WorkspacePaletteAction};
    use super::event_for_selection;

    fn result(action: PaletteEvent) -> PaletteResult {
        PaletteResult {
            label: "users".into(),
            sublabel: "table · local".into(),
            conn_label: String::new(),
            action,
        }
    }

    #[test]
    fn picking_a_row_emits_its_action() {
        for action in [
            PaletteEvent::OpenTab(TabSpec::DataViewer {
                conn_id: ConnectionId::parse("local"),
                object: based_core::ObjectRef::bare("users"),
                filter: None,
            }),
            PaletteEvent::InjectSql {
                conn_id: ConnectionId::parse("local"),
                sql: "SELECT 1".into(),
            },
            PaletteEvent::OpenProjectQuery("queries/active".into()),
            PaletteEvent::WorkspaceAction(WorkspacePaletteAction::OpenHome),
        ] {
            assert_eq!(event_for_selection(&result(action.clone())), action);
        }
    }
}
