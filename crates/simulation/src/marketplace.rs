use crate::SimulationError;
use rand::{RngExt, SeedableRng, rngs::SmallRng};

pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub const UPDATE_TIME_MS: u64 = 4 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Good {
    Berries,
    Pebbles,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prices {
    berries: f64,
    pebbles: f64,
}

impl Default for Prices {
    fn default() -> Self {
        Self {
            berries: 1.0,
            pebbles: 2.0,
        }
    }
}

impl Prices {
    pub fn new(berries: f64, pebbles: f64) -> Result<Self, SimulationError> {
        if !berries.is_finite() || berries <= 0.0 || !pebbles.is_finite() || pebbles <= 0.0 {
            return Err(SimulationError::InvalidPrices);
        }
        Ok(Self { berries, pebbles })
    }

    pub fn coins_per_kg(self, good: Good) -> f64 {
        match good {
            Good::Berries => self.berries,
            Good::Pebbles => self.pebbles,
        }
    }

    pub fn value(self, good: Good, grams: f64) -> f64 {
        grams / 1000.0 * self.coins_per_kg(good)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Market {
    seed: u64,
    pub prices: Prices,
}

impl Default for Market {
    fn default() -> Self {
        let mut market = Self {
            seed: rand::make_rng::<SmallRng>().random(),
            prices: Prices::default(),
        };
        market.update(0);
        market
    }
}

impl Market {
    pub fn update(&mut self, time_ms: u64) {
        let period = if time_ms < UPDATE_TIME_MS {
            0
        } else {
            1 + (time_ms - UPDATE_TIME_MS) / DAY_MS
        };
        // Date-addressed draws preserve branching and let empty universes skip unobserved days.
        let mut rng = SmallRng::seed_from_u64(
            self.seed
                .wrapping_add(period.wrapping_mul(0x9e3779b97f4a7c15)),
        );
        self.prices = Prices {
            berries: rng.random_range(1.0..=2.0),
            pebbles: rng.random_range(1.0..=4.0),
        };
    }
}

pub(crate) fn until_update(time_ms: u64) -> u64 {
    let time_of_day = time_ms % DAY_MS;
    if time_of_day < UPDATE_TIME_MS {
        UPDATE_TIME_MS - time_of_day
    } else {
        DAY_MS - time_of_day + UPDATE_TIME_MS
    }
}
