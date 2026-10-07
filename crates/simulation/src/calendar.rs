use crate::{SimulationError, marketplace::DAY_MS};

pub const WEEK_MS: u64 = 7 * DAY_MS;
pub const SUNDAY_SETTLEMENT_MS: u64 = 4 * 60 * 60 * 1000;
pub const FIRST_WEEKLY_SETTLEMENT_MS: u64 = 6 * DAY_MS + SUNDAY_SETTLEMENT_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub fn at(time_ms: u64) -> Self {
        match (time_ms / DAY_MS) % 7 {
            0 => Self::Monday,
            1 => Self::Tuesday,
            2 => Self::Wednesday,
            3 => Self::Thursday,
            4 => Self::Friday,
            5 => Self::Saturday,
            _ => Self::Sunday,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
            Self::Sunday => "Sunday",
        }
    }
}

/// Returns the next weekly boundary strictly after this time.
pub fn next_weekly_settlement(time_ms: u64) -> Result<u64, SimulationError> {
    if time_ms < FIRST_WEEKLY_SETTLEMENT_MS {
        return Ok(FIRST_WEEKLY_SETTLEMENT_MS);
    }
    let periods = (time_ms - FIRST_WEEKLY_SETTLEMENT_MS) / WEEK_MS + 1;
    periods
        .checked_mul(WEEK_MS)
        .and_then(|elapsed| FIRST_WEEKLY_SETTLEMENT_MS.checked_add(elapsed))
        .ok_or(SimulationError::TimeOverflow)
}
