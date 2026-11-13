//! Postgres TLS mode, shared by the project format, the driver and the wizard.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// libpq `sslmode`. Written kebab-case (`verify-full`) in connection files; the
/// snake_case aliases read values an older runtime type wrote.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SslMode {
    Disable,
    #[default]
    Prefer,
    Require,
    #[serde(alias = "verify_ca")]
    VerifyCa,
    #[serde(alias = "verify_full")]
    VerifyFull,
}

impl SslMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

impl fmt::Display for SslMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An `sslmode` value that isn't one of libpq's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSslMode(pub String);

impl fmt::Display for UnknownSslMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown sslmode {:?} (expected disable, prefer, require, verify-ca or verify-full)",
            self.0
        )
    }
}

impl Error for UnknownSslMode {}

impl FromStr for SslMode {
    type Err = UnknownSslMode;

    /// Case-insensitive; `off` means `disable`, and `_` may stand for `-`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "disable" | "off" => Ok(Self::Disable),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-ca" => Ok(Self::VerifyCa),
            "verify-full" => Ok(Self::VerifyFull),
            _ => Err(UnknownSslMode(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_round_trips() {
        for mode in [
            SslMode::Disable,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ] {
            assert_eq!(mode.as_str().parse(), Ok(mode));
        }
        assert_eq!("OFF".parse(), Ok(SslMode::Disable));
        assert_eq!("verify_full".parse(), Ok(SslMode::VerifyFull));
        assert!("strict".parse::<SslMode>().is_err());
    }

    #[test]
    fn serde_is_kebab_and_reads_snake_case() {
        assert_eq!(
            serde_json::to_string(&SslMode::VerifyCa).unwrap(),
            r#""verify-ca""#
        );
        assert_eq!(
            serde_json::from_str::<SslMode>(r#""verify_full""#).unwrap(),
            SslMode::VerifyFull
        );
    }
}
