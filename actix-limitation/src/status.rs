use std::{convert::TryInto, ops::Add, time::Duration};

use chrono::SubsecRound as _;

use crate::Error as LimitationError;

/// A report for a given key containing the limit status.
#[derive(Debug, Clone)]
pub struct Status {
    pub(crate) limit: usize,
    pub(crate) remaining: usize,
    pub(crate) reset_epoch_utc: usize,
}

impl Status {
    /// Constructs status limit status from parts.
    #[must_use]
    pub(crate) fn new(count: usize, limit: usize, reset_epoch_utc: usize) -> Self {
        let remaining = if count >= limit { 0 } else { limit - count };

        Status {
            limit,
            remaining,
            reset_epoch_utc,
        }
    }

    /// Returns the maximum number of requests allowed in the current period.
    #[must_use]
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Returns how many requests are left in the current period.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.remaining
    }

    /// Returns a UNIX timestamp in UTC approximately when the next period will begin.
    #[must_use]
    pub fn reset_epoch_utc(&self) -> usize {
        self.reset_epoch_utc
    }

    pub(crate) fn epoch_utc_plus(duration: Duration) -> Result<usize, LimitationError> {
        match chrono::Duration::from_std(duration) {
            Ok(value) => Ok(chrono::Utc::now()
                .add(value)
                .round_subsecs(0)
                .timestamp()
                .try_into()
                .unwrap_or(0)),

            Err(_) => Err(LimitationError::Other(
                "Source duration value is out of range for the target type".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_status() {
        let status = Status {
            limit: 100,
            remaining: 0,
            reset_epoch_utc: 1000,
        };

        assert_eq!(status.limit(), 100);
        assert_eq!(status.remaining(), 0);
        assert_eq!(status.reset_epoch_utc(), 1000);
    }

    #[test]
    fn test_build_status() {
        let count = 200;
        let limit = 100;
        let status = Status::new(count, limit, 2000);
        assert_eq!(status.limit(), limit);
        assert_eq!(status.remaining(), 0);
        assert_eq!(status.reset_epoch_utc(), 2000);
    }

    #[test]
    fn test_build_status_limit() {
        let limit = 100;
        let status = Status::new(0, limit, 2000);
        assert_eq!(status.limit(), limit);
        assert_eq!(status.remaining(), limit);
        assert_eq!(status.reset_epoch_utc(), 2000);
    }

    #[test]
    fn test_epoch_utc_plus_zero() {
        let duration = Duration::from_secs(0);
        let seconds = Status::epoch_utc_plus(duration).unwrap();
        assert!(seconds as u64 >= duration.as_secs());
    }

    #[test]
    fn test_epoch_utc_plus() {
        let duration = Duration::from_secs(10);
        let seconds = Status::epoch_utc_plus(duration).unwrap();
        assert!(seconds as u64 >= duration.as_secs() + 10);
    }

    #[test]
    #[should_panic = "Source duration value is out of range for the target type"]
    fn test_epoch_utc_plus_overflow() {
        let duration = Duration::from_secs(10000000000000000000);
        Status::epoch_utc_plus(duration).unwrap();
    }
}
