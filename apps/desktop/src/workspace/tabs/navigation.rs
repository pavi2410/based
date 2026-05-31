use gpui::EntityId;

/// Visit-order stacks for Go Back / Go Forward (tab strip UI deferred — logic-only phase).
#[derive(Debug, Default)]
pub struct TabNavigationHistory {
    back: Vec<EntityId>,
    forward: Vec<EntityId>,
    current: Option<EntityId>,
}

impl TabNavigationHistory {
    pub fn record_activation(&mut self, panel_id: EntityId) {
        if self.current == Some(panel_id) {
            return;
        }
        if let Some(prev) = self.current {
            self.back.push(prev);
        }
        self.forward.clear();
        self.current = Some(panel_id);
    }

    pub fn go_back(&mut self) -> Option<EntityId> {
        let prev = self.back.pop()?;
        if let Some(cur) = self.current.take() {
            self.forward.push(cur);
        }
        self.current = Some(prev);
        Some(prev)
    }

    pub fn go_forward(&mut self) -> Option<EntityId> {
        let next = self.forward.pop()?;
        if let Some(cur) = self.current.take() {
            self.back.push(cur);
        }
        self.current = Some(next);
        Some(next)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    #[cfg(test)]
    fn panel_id(n: u64) -> EntityId {
        EntityId::from(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_forward_traversal() {
        let mut h = TabNavigationHistory::default();
        h.record_activation(TabNavigationHistory::panel_id(1));
        h.record_activation(TabNavigationHistory::panel_id(2));
        h.record_activation(TabNavigationHistory::panel_id(3));
        assert_eq!(h.go_back(), Some(TabNavigationHistory::panel_id(2)));
        assert_eq!(h.go_forward(), Some(TabNavigationHistory::panel_id(3)));
        assert_eq!(h.go_back(), Some(TabNavigationHistory::panel_id(2)));
        assert_eq!(h.go_back(), Some(TabNavigationHistory::panel_id(1)));
        assert!(h.go_back().is_none());
    }

    #[test]
    fn new_activation_clears_forward() {
        let mut h = TabNavigationHistory::default();
        h.record_activation(TabNavigationHistory::panel_id(1));
        h.record_activation(TabNavigationHistory::panel_id(2));
        h.record_activation(TabNavigationHistory::panel_id(3));
        h.go_back();
        h.record_activation(TabNavigationHistory::panel_id(4));
        assert!(!h.can_go_forward());
    }
}
