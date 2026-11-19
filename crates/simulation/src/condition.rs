//! Fixed-point condition and elapsed wear, including fractional object wear.
use crate::{SimulationError, marketplace::DAY_MS};

pub const CONDITION_LIFETIME_MS: u64 = 1_000 * DAY_MS;
// Keep the stored units for condition and repair damage stable across wear-rate changes.
const CONDITION_UNIT_SCALE: u128 = 200 * DAY_MS as u128;
pub const MAX_CONDITION_UNITS: u128 = CONDITION_UNIT_SCALE.pow(2);
const LIFETIME: u128 = CONDITION_LIFETIME_MS as u128;
const BASE_WEAR_PER_MS: u128 = MAX_CONDITION_UNITS / LIFETIME;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Condition {
    remaining_units: u128,
}

impl Default for Condition {
    fn default() -> Self {
        Self {
            remaining_units: MAX_CONDITION_UNITS,
        }
    }
}

impl Condition {
    pub fn from_units(remaining_units: u128) -> Result<Self, SimulationError> {
        if remaining_units > MAX_CONDITION_UNITS {
            return Err(SimulationError::InvalidQuantity);
        }
        Ok(Self { remaining_units })
    }
    pub fn condition_units(self) -> u128 {
        self.remaining_units
    }
    pub fn damage_units(self) -> u128 {
        MAX_CONDITION_UNITS - self.remaining_units
    }
    pub fn fraction(self) -> f64 {
        self.remaining_units as f64 / MAX_CONDITION_UNITS as f64
    }
    pub fn needs_repair(self) -> bool {
        self.damage_units() >= MAX_CONDITION_UNITS / 20
    }
    pub(crate) fn valid(self) -> bool {
        self.remaining_units <= MAX_CONDITION_UNITS
    }
    pub(crate) fn advance(&mut self, wear: u128, repaired: u128) {
        self.remaining_units = self
            .remaining_units
            .saturating_add(repaired)
            .saturating_sub(wear)
            .min(MAX_CONDITION_UNITS);
    }
}

// Rare capped integrals can overflow a product even though its quotient fits.
fn mul_div(a: u128, b: u128, divisor: u128) -> u128 {
    if let Some(product) = a.checked_mul(b) {
        return product / divisor;
    }
    let whole = a / divisor;
    let part = a % divisor;
    let mut result = 0;
    let mut remainder = 0;
    for bit in (0..128).rev() {
        result *= 2;
        remainder *= 2;
        if b & (1u128 << bit) != 0 {
            result += whole;
            remainder += part;
        }
        result += remainder / divisor;
        remainder %= divisor;
    }
    result
}

// Round the cumulative extra wear so the difference across any interval
// telescopes over split advances. Split the square to keep products in u128.
fn cumulative_damage_wear(damage: u128) -> u128 {
    let capped = damage.min(MAX_CONDITION_UNITS);
    let whole = capped / CONDITION_UNIT_SCALE;
    let remainder = capped % CONDITION_UNIT_SCALE;
    2 * whole.pow(2)
        + (4 * whole * remainder * CONDITION_UNIT_SCALE + 2 * remainder.pow(2))
            / MAX_CONDITION_UNITS
        + 4 * (damage - capped)
}

/// Object wear while the room's condition changes linearly over this interval.
pub(crate) fn room_object_wear(start: Condition, elapsed_ms: u64, repaired: u128) -> u128 {
    if elapsed_ms == 0 {
        return 0;
    }
    let natural = base_wear(elapsed_ms);
    if repaired == 0 {
        let damage = start.damage_units();
        return natural + cumulative_damage_wear(damage + natural) - cumulative_damage_wear(damage);
    }
    let elapsed = u128::from(elapsed_ms);
    let repaired = repaired.min(MAX_CONDITION_UNITS);
    let units = start.condition_units();
    let funded = units + repaired;
    if funded <= natural {
        let decay = natural - repaired;
        let reduction = if units == 0 {
            0
        } else {
            mul_div(mul_div(2 * units, units, LIFETIME), elapsed, decay)
        };
        5 * natural - reduction
    } else if funded - natural > MAX_CONDITION_UNITS {
        let rise = repaired - natural;
        let damage = start.damage_units();
        natural + mul_div(mul_div(2 * damage, damage, LIFETIME), elapsed, rise)
    } else {
        let end = funded - natural;
        5 * natural - mul_div(2 * (units + end), elapsed, LIFETIME)
    }
}

pub(crate) fn base_wear(elapsed_ms: u64) -> u128 {
    u128::from(elapsed_ms) * BASE_WEAR_PER_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_and_room_wear_use_one_thousand_day_lifetime() {
        let elapsed = 10 * DAY_MS;
        assert_eq!(base_wear(elapsed), MAX_CONDITION_UNITS / 100);
        let full = Condition::default();
        assert_eq!(
            room_object_wear(full, elapsed, 0),
            MAX_CONDITION_UNITS * 102 / 10_000
        );
        let ruined = Condition::from_units(0).unwrap();
        assert_eq!(
            room_object_wear(ruined, elapsed, 0),
            MAX_CONDITION_UNITS / 20
        );
    }

    #[test]
    fn room_wear_integrates_condition_and_zero_crossing() {
        let full = Condition::default();
        let lifetime = CONDITION_LIFETIME_MS;
        assert_eq!(room_object_wear(full, lifetime, 0), 3 * MAX_CONDITION_UNITS);
        assert_eq!(
            room_object_wear(full, 2 * lifetime, 0),
            8 * MAX_CONDITION_UNITS
        );
        let half = Condition::from_units(MAX_CONDITION_UNITS / 2).unwrap();
        assert_eq!(
            room_object_wear(half, lifetime / 2, 0),
            2 * MAX_CONDITION_UNITS
        );
        assert_eq!(
            room_object_wear(half, lifetime / 2, MAX_CONDITION_UNITS / 2),
            MAX_CONDITION_UNITS * 3 / 2
        );
    }

    #[test]
    fn wear_is_split_independent_and_preserves_fractional_units() {
        let mut room = Condition::default();
        let whole = room_object_wear(room, 12_345_678, 0);
        let mut split = 0;
        for elapsed in [1, 2_345_677, 10_000_000] {
            split += room_object_wear(room, elapsed, 0);
            room.advance(base_wear(elapsed), 0);
        }
        assert_eq!(whole, split);
        let mut object = Condition::default();
        object.advance(1, 0);
        assert_eq!(object.damage_units(), 1);
        assert!(!object.needs_repair());
        object.advance(MAX_CONDITION_UNITS / 20 - 1, 0);
        assert!(object.needs_repair());
    }

    #[test]
    fn capped_condition_handles_extreme_elapsed_and_repair() {
        let mut condition = Condition::default();
        let wear = room_object_wear(condition, u64::MAX, 0);
        assert_eq!(wear, 5 * base_wear(u64::MAX) - 2 * MAX_CONDITION_UNITS);
        condition.advance(wear, 0);
        assert_eq!(condition.condition_units(), 0);
        condition.advance(0, MAX_CONDITION_UNITS);
        assert_eq!(condition, Condition::default());
        assert!(Condition::from_units(MAX_CONDITION_UNITS + 1).is_err());
        let low = Condition::from_units(MAX_CONDITION_UNITS / 4).unwrap();
        assert!(
            room_object_wear(low, CONDITION_LIFETIME_MS / 4, MAX_CONDITION_UNITS)
                >= base_wear(CONDITION_LIFETIME_MS / 4)
        );
    }

    #[test]
    fn overflowing_intermediate_products_keep_exact_quotients() {
        assert!(
            (2 * MAX_CONDITION_UNITS)
                .checked_mul(MAX_CONDITION_UNITS)
                .is_none()
        );
        assert_eq!(
            mul_div(2 * MAX_CONDITION_UNITS, MAX_CONDITION_UNITS, LIFETIME),
            2 * MAX_CONDITION_UNITS * BASE_WEAR_PER_MS
        );
        let a = 2 * MAX_CONDITION_UNITS + 1;
        let b = MAX_CONDITION_UNITS - 1;
        assert_eq!(
            mul_div(a, b, MAX_CONDITION_UNITS),
            2 * MAX_CONDITION_UNITS - 2
        );
    }
}
