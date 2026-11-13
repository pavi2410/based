//! A credential held in memory that never prints itself.

use std::fmt;

/// A password or passphrase. `Debug` shows `***`; there is no `Display`,
/// `Deref` or `Serialize`, so the value leaves only through [`Secret::expose`].
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The plain value, for the driver, the URI builder or the `.env` writer.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            f.write_str("Secret(<empty>)")
        } else {
            f.write_str("Secret(***)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Secret;

    #[test]
    fn debug_hides_the_value() {
        let s = Secret::new("hunter2");
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert_eq!(format!("{:?}", Secret::default()), "Secret(<empty>)");
        assert_eq!(s.expose(), "hunter2");
    }
}
