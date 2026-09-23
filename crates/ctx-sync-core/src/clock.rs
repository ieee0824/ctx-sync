//! Current time, overridable for tests.
//!
//! This is the only place in the core that reads the system clock. Other
//! modules receive the time as an argument.

use chrono::{DateTime, FixedOffset, Local};

use crate::{Error, Result};

/// Environment variable that fixes the current time (RFC 3339).
pub const NOW_ENV: &str = "CTX_SYNC_NOW";

/// Returns `CTX_SYNC_NOW` when it is set, otherwise the local time.
pub fn now() -> Result<DateTime<FixedOffset>> {
    let value = std::env::var(NOW_ENV).ok();
    parse_now(value.as_deref())
}

/// Returns the given RFC 3339 time, or the local time when `value` is `None`.
pub fn parse_now(value: Option<&str>) -> Result<DateTime<FixedOffset>> {
    match value {
        None => Ok(Local::now().fixed_offset()),
        Some(v) => DateTime::parse_from_rfc3339(v.trim()).map_err(|e| {
            Error::InvalidConfig(format!("{NOW_ENV} must be an RFC 3339 time: {v:?} ({e})"))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixed_time() {
        let t = parse_now(Some("2026-09-23T18:00:00+09:00")).unwrap();
        assert_eq!(t.to_rfc3339(), "2026-09-23T18:00:00+09:00");
    }

    #[test]
    fn rejects_invalid_time() {
        let err = parse_now(Some("x")).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
    }

    #[test]
    fn defaults_to_local_time() {
        assert!(parse_now(None).is_ok());
    }
}
