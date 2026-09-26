//! Time-based classification of active workers.

use chrono::{DateTime, FixedOffset, TimeDelta};

use super::Worker;
use crate::{Error, Result};

pub const DEFAULT_STALE_AFTER_HOURS: i64 = 24;

pub fn default_stale_after() -> TimeDelta {
    TimeDelta::hours(DEFAULT_STALE_AFTER_HOURS)
}

/// A worker is stale only after the threshold has elapsed, not at the threshold.
pub fn is_stale(worker: &Worker, now: DateTime<FixedOffset>, stale_after: TimeDelta) -> bool {
    worker.status.is_active() && now.signed_duration_since(worker.last_updated) > stale_after
}

/// Parse a positive integer followed by `m`, `h`, `d`, or `w`.
pub fn parse_duration(s: &str) -> Result<TimeDelta> {
    let value = s.trim();
    let invalid = || Error::InvalidConfig(format!("invalid duration: {s:?}"));
    let unit = value.chars().last().ok_or_else(invalid)?;
    let number = value.strip_suffix(unit).ok_or_else(invalid)?;
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let amount = number
        .parse::<i64>()
        .ok()
        .filter(|amount| *amount > 0)
        .ok_or_else(invalid)?;
    let duration = match unit {
        'm' => TimeDelta::try_minutes(amount),
        'h' => TimeDelta::try_hours(amount),
        'd' => TimeDelta::try_days(amount),
        'w' => TimeDelta::try_weeks(amount),
        _ => None,
    };
    duration.ok_or_else(invalid)
}

/// Format elapsed time to the largest useful whole unit.
pub fn format_age(age: TimeDelta) -> String {
    let seconds = age.num_seconds();
    if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3_600 {
        format!("{}m", age.num_minutes())
    } else if seconds < 86_400 {
        format!("{}h", age.num_hours())
    } else {
        format!("{}d", age.num_days())
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use uuid::Uuid;

    use super::*;
    use crate::model::WorkerStatus;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-09-26T18:00:00+09:00").unwrap()
    }

    #[test]
    fn only_active_workers_older_than_the_threshold_are_stale() {
        let mut worker = Worker::new(Uuid::nil(), "parser", now() - TimeDelta::hours(25));
        assert!(is_stale(&worker, now(), default_stale_after()));
        worker.status = WorkerStatus::Blocked;
        assert!(is_stale(&worker, now(), default_stale_after()));
        worker.status = WorkerStatus::Done;
        assert!(!is_stale(&worker, now(), default_stale_after()));
        worker.status = WorkerStatus::Abandoned;
        assert!(!is_stale(&worker, now(), default_stale_after()));
        worker.status = WorkerStatus::Working;
        worker.last_updated = now() - TimeDelta::hours(24);
        assert!(!is_stale(&worker, now(), default_stale_after()));
        worker.last_updated = now() - TimeDelta::hours(23);
        assert!(!is_stale(&worker, now(), default_stale_after()));
    }

    #[test]
    fn parses_positive_whole_durations_and_rejects_other_values() {
        for (input, expected) in [
            ("90m", TimeDelta::minutes(90)),
            ("24h", TimeDelta::hours(24)),
            ("3d", TimeDelta::days(3)),
            ("1w", TimeDelta::weeks(1)),
            (" 2h ", TimeDelta::hours(2)),
        ] {
            assert_eq!(parse_duration(input).unwrap(), expected);
        }
        for input in [
            "",
            "0h",
            "-1h",
            "+1h",
            "3",
            "3x",
            "1.5h",
            "3日",
            "999999999999999999999w",
        ] {
            assert!(matches!(
                parse_duration(input),
                Err(Error::InvalidConfig(_))
            ));
        }
    }

    #[test]
    fn formats_elapsed_time_without_rounding_up() {
        assert_eq!(format_age(TimeDelta::seconds(30)), "just now");
        assert_eq!(format_age(TimeDelta::minutes(59)), "59m");
        assert_eq!(format_age(TimeDelta::hours(3)), "3h");
        assert_eq!(format_age(TimeDelta::hours(50)), "2d");
    }
}
