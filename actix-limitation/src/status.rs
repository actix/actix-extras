use std::time::Duration;

use time::OffsetDateTime;

use crate::Error as LimitationError;

/// A report for a given key containing the limit status.
#[derive(Debug, Clone)]
pub struct Status {
    pub(crate) limit: usize,
    pub(crate) remaining: usize,
    pub(crate) reset_epoch_utc: usize,
}

impl Status {
    /// Constructs a [`Status`] from count, limit, and reset timestamp.
    #[must_use]
    pub(crate) fn new(count: usize, limit: usize, reset_epoch_utc: usize) -> Self {
        let remaining = limit.saturating_sub(count);

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

    pub(crate) fn epoch_utc_plus(
        now: OffsetDateTime,
        duration: Duration,
    ) -> Result<usize, LimitationError> {
        let duration = time::Duration::try_from(duration).map_err(|_| {
            LimitationError::Other(
                "Source duration value is out of range for the target type".to_owned(),
            )
        })?;

        // Use Unix time so long periods can extend beyond the calendar date range.
        let reset_nanos = now.unix_timestamp_nanos() + duration.whole_nanoseconds();

        // Round to the nearest second, with half seconds rounded up.
        let reset_secs = (reset_nanos + 500_000_000).div_euclid(1_000_000_000);

        Ok(reset_secs.try_into().unwrap_or(0))
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
        let now = OffsetDateTime::from_unix_timestamp_nanos(100_499_999_999).unwrap();

        assert_eq!(Status::epoch_utc_plus(now, Duration::ZERO).unwrap(), 100);
    }

    #[test]
    fn test_epoch_utc_plus() {
        let now = OffsetDateTime::from_unix_timestamp_nanos(100_250_000_000).unwrap();
        let duration = Duration::from_millis(10_250);

        assert_eq!(Status::epoch_utc_plus(now, duration).unwrap(), 111);
    }

    #[test]
    fn test_epoch_utc_plus_beyond_calendar_range() {
        let duration = Duration::from_secs(253_402_300_800); // Start of year 10000.

        assert_eq!(
            Status::epoch_utc_plus(OffsetDateTime::UNIX_EPOCH, duration).unwrap(),
            usize::try_from(duration.as_secs()).unwrap_or(0),
        );
    }

    #[test]
    fn test_epoch_utc_plus_overflow() {
        let err = Status::epoch_utc_plus(OffsetDateTime::UNIX_EPOCH, Duration::MAX).unwrap_err();

        assert!(matches!(err, LimitationError::Other(_)));
    }
}
