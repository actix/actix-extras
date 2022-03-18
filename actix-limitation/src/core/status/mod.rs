use crate::Error as LimitationError;
use chrono::SubsecRound;
use std::{convert::TryInto, ops::Add, time::Duration};

/// A report for a given key containing the limit status.
///
/// The status contains the following information:
///
/// - [`limit`]: the maximum number of requests allowed in the current period
/// - [`remaining`]: how many requests are left in the current period
/// - [`reset_epoch_utc`]: a UNIX timestamp in UTC approximately when the next period will begin
#[derive(Clone, Debug)]
pub struct Status {
    pub(crate) limit: usize,
    pub(crate) remaining: usize,
    pub(crate) reset_epoch_utc: usize,
}

impl Status {
    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }

    pub fn reset_epoch_utc(&self) -> usize {
        self.reset_epoch_utc
    }

    pub(crate) fn build_status(count: usize, limit: usize, reset_epoch_utc: usize) -> Self {
        let remaining = if count >= limit { 0 } else { limit - count };

        Status {
            limit,
            remaining,
            reset_epoch_utc,
        }
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
mod test;
