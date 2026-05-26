//! Plain-text labels for dock tab strip (`Panel::tab_name`).

use gpui::SharedString;

use super::tab_spec::TabSpec;

pub fn tab_label_for_spec(spec: &TabSpec, dirty: bool) -> SharedString {
    let base = match spec {
        TabSpec::Dashboard(id) => id.0.clone(),
        TabSpec::DataViewer { object, .. } => short_object_name(object),
        TabSpec::QueryEditor { .. } => "Query".to_string(),
        TabSpec::Pipeline { collection, .. } => collection.clone(),
        TabSpec::Inspector { object, .. } => format!("{} (schema)", short_object_name(object)),
        TabSpec::ObjectInfo {
            object_name,
            kind_label,
            ..
        } => format!("{object_name} ({kind_label})"),
        TabSpec::DocumentInsert { collection, .. } => format!("Insert · {collection}"),
    };
    with_dirty_suffix(base, dirty)
}

pub fn with_dirty_suffix(label: impl Into<SharedString>, dirty: bool) -> SharedString {
    let mut s = label.into().to_string();
    if dirty {
        push_dirty_marker(&mut s);
    }
    s.into()
}

fn push_dirty_marker(s: &mut String) {
    const MARKER: &str = " ●";
    if !s.ends_with(MARKER) {
        s.push_str(MARKER);
    }
}

fn short_object_name(object: &str) -> String {
    object
        .rsplit_once('.')
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| object.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

    #[test]
    fn data_viewer_short_name() {
        let spec = TabSpec::DataViewer {
            conn_id: ConnectionId("x".into()),
            object: "public.orders".into(),
        };
        assert_eq!(tab_label_for_spec(&spec, false).as_ref(), "orders");
    }

    #[test]
    fn dirty_marker() {
        let s = with_dirty_suffix("Query", true);
        assert_eq!(s.as_ref(), "Query ●");
    }
}
