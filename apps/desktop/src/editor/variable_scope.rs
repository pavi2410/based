//! Typed variable scope for editor autocomplete and query substitution.

use std::collections::HashMap;

/// The data type of a variable, used to drive autocomplete hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    /// Arbitrary JSON — type is unknown at the scope level.
    Json,
}

/// A single named variable available in the editor.
#[derive(Debug, Clone)]
pub struct ScopedVar {
    pub name: String,
    pub value: String,
    pub var_type: VarType,
    /// Shown in the autocomplete popover.
    pub description: Option<String>,
}

/// Collection of variables visible in a query editor.
///
/// Built from project-level `.env`/`vars.toml` plus any connection-level
/// overrides. Future: per-tab local variables.
#[derive(Debug, Clone, Default)]
pub struct VariableScope {
    vars: HashMap<String, ScopedVar>,
}

impl VariableScope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from a map of name → string value (as loaded from `.env`).
    pub fn from_string_map(map: &HashMap<String, String>) -> Self {
        let vars = map
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    ScopedVar {
                        name: k.clone(),
                        value: v.clone(),
                        var_type: VarType::String,
                        description: None,
                    },
                )
            })
            .collect();
        Self { vars }
    }

    pub fn get(&self, name: &str) -> Option<&ScopedVar> {
        self.vars.get(name)
    }

    pub fn all(&self) -> impl Iterator<Item = &ScopedVar> {
        self.vars.values()
    }

    /// Returns variable names in sorted order (stable for autocomplete lists).
    pub fn names_sorted(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.vars.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}
