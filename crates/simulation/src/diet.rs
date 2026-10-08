use crate::{ActiveAction, Citizen, marketplace::Good};

pub const DIET_DECAY_PER_DAY: f64 = 50.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DietCategory {
    Meat,
    Carbs,
    Fibre,
    Sweets,
}

impl DietCategory {
    pub const ALL: [Self; 4] = [Self::Meat, Self::Carbs, Self::Fibre, Self::Sweets];

    pub fn name(self) -> &'static str {
        match self {
            Self::Meat => "Meat",
            Self::Carbs => "Carbs",
            Self::Fibre => "Fibre",
            Self::Sweets => "Sweets",
        }
    }
}

pub fn category_bonus(value: f64) -> f64 {
    0.10 * value.clamp(0.0, 50.0) + 0.04 * (value.clamp(0.0, 100.0) - 50.0).max(0.0)
}

impl Good {
    pub fn diet_per_unit(self) -> [f64; 4] {
        match self {
            Self::Bread => [0.0, 50.0, 0.0, 0.0],
            Self::Berries => [0.0, 0.0, 0.0, crate::BERRY_NUTRITION_PER_GRAM],
            Self::BerryPie => [0.0, 45.0, 0.0, 15.0],
            Self::Fish => [25.0, 0.0, 0.0, 0.0],
            Self::Cabbage => [0.0, 0.0, 25.0, 0.0],
            _ => [0.0; 4],
        }
    }
}

impl Citizen {
    pub fn diet(&self, category: DietCategory) -> f64 {
        self.diet[category as usize]
    }

    pub fn dietary_wellbeing(&self) -> f64 {
        self.diet.into_iter().map(category_bonus).sum()
    }

    pub(crate) fn diet_rates(&self, active: Option<ActiveAction>) -> [f64; 4] {
        let decay = DIET_DECAY_PER_DAY / crate::marketplace::DAY_MS as f64;
        let mut rates = [-decay; 4];
        if let Some(action) = active
            && action.meal_duration_ms > action.duration_ms - action.remaining_ms
        {
            for good in Good::FOOD {
                for (rate, intake) in rates.iter_mut().zip(good.diet_per_unit()) {
                    *rate += intake * action.meal_units[good as usize] as f64
                        / action.meal_duration_ms as f64;
                }
            }
        }
        rates
    }

    pub(crate) fn advance_diet(&mut self, elapsed_ms: u64, active: Option<ActiveAction>) {
        let eating_ms = active.map_or(0, |action| {
            elapsed_ms.min(
                action
                    .meal_duration_ms
                    .saturating_sub(action.duration_ms - action.remaining_ms),
            )
        });
        let rates = self.diet_rates(active);
        let idle_ms = elapsed_ms - eating_ms;
        let decay = DIET_DECAY_PER_DAY / crate::marketplace::DAY_MS as f64;
        for (value, rate) in self.diet.iter_mut().zip(rates) {
            *value = ((*value + rate * eating_ms as f64).clamp(0.0, 100.0)
                - decay * idle_ms as f64)
                .clamp(0.0, 100.0);
        }
    }

    fn meal_wellbeing(&self, good: Good, units: crate::Quantity) -> Option<f64> {
        let mut predicted = self.clone();
        predicted.active_action = None;
        predicted.inventory.insert(good, self.units(good) - units);
        let meal_ms = units * good.eating_ms_per_unit()?;
        let drink_units = self.drink_units();
        predicted
            .inventory
            .insert(Good::Water, self.units(Good::Water) - drink_units);
        let duration_ms =
            meal_ms.max(drink_units * crate::DRINK_DURATION_MS / crate::DRINK_WATER_GRAMS);
        let mut meal_units = [0; Good::COUNT];
        meal_units[good as usize] = units;
        let action = ActiveAction {
            action: crate::CitizenAction::Eat,
            remaining_ms: duration_ms,
            duration_ms,
            meal_duration_ms: meal_ms,
            drink_units,
            meal_nutrition: units as f64 * good.nutrition_per_unit()?,
            meal_units,
            travel_origin: self.position,
            travel_destination: None,
        };
        predicted.advance_needs(duration_ms, Some(action)).ok()?;
        predicted.personal_wellbeing().ok()
    }

    pub(crate) fn preferred_complete_meal(&self) -> Option<(Good, crate::Quantity)> {
        Good::FOOD
            .into_iter()
            .filter_map(|good| {
                let units = (crate::MEAL_NOURISHMENT / good.nutrition_per_unit()?).ceil()
                    as crate::Quantity;
                (self.available_units(good) >= units)
                    .then(|| (good, units, self.meal_wellbeing(good, units)))
            })
            .filter_map(|(good, units, score)| Some((good, units, score?)))
            .max_by(|a, b| {
                a.2.total_cmp(&b.2)
                    .then_with(|| (b.0 as usize).cmp(&(a.0 as usize)))
            })
            .map(|(good, units, _)| (good, units))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CitizenAction, marketplace::DAY_MS};

    fn close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-9, "{left} != {right}");
    }

    #[test]
    fn categories_start_empty_decay_and_have_diminishing_bonuses() {
        let mut citizen = Citizen::new(0.0).unwrap();
        assert_eq!(citizen.diet, [0.0; 4]);
        for (value, bonus) in [
            (0.0, 0.0),
            (25.0, 2.5),
            (50.0, 5.0),
            (75.0, 6.0),
            (100.0, 7.0),
        ] {
            close(category_bonus(value), bonus);
        }
        citizen.diet = [100.0, 75.0, 50.0, 25.0];
        let next = citizen.advance(DAY_MS).unwrap();
        assert_eq!(next.diet, [50.0, 25.0, 0.0, 0.0]);
        assert_eq!(next.advance(3 * DAY_MS).unwrap().diet, [0.0; 4]);
    }

    #[test]
    fn each_food_contributes_proportionally_during_the_meal() {
        for (good, units) in [
            (Good::Bread, 1),
            (Good::Berries, 155),
            (Good::BerryPie, 1),
            (Good::Fish, 2),
            (Good::Cabbage, 2),
        ] {
            let source = Citizen::new(80.0).unwrap().with_good(good, units).unwrap();
            let started = source.start_action(CitizenAction::Eat).unwrap();
            assert_eq!(started.diet, [0.0; 4]);
            let duration = started.active_action().unwrap().duration_ms();
            let half = started.advance(duration / 2).unwrap();
            let complete = started.advance(duration).unwrap();
            for category in DietCategory::ALL {
                let index = category as usize;
                let intake = good.diet_per_unit()[index] * units as f64;
                let decay = DIET_DECAY_PER_DAY * duration as f64 / DAY_MS as f64;
                close(half.diet(category), ((intake - decay) / 2.0).max(0.0));
                close(complete.diet(category), (intake - decay).max(0.0));
                close(
                    half.advance(duration - duration / 2)
                        .unwrap()
                        .diet(category),
                    complete.diet(category),
                );
            }
        }
    }

    #[test]
    fn meals_consider_diet_cost_and_time_instead_of_only_nutrition_price() {
        let mut source = Citizen::new(40.0)
            .unwrap()
            .with_good(Good::Bread, 1)
            .unwrap()
            .with_good(Good::Fish, 2)
            .unwrap();
        source.diet = [0.0, 100.0, 0.0, 0.0];
        let chosen = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(chosen.units(Good::Fish), 0);
        assert_eq!(chosen.units(Good::Bread), 1);
        source.prices = source.prices.with_price(Good::Fish, 100.0).unwrap();
        let economical = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(economical.units(Good::Fish), 2);
        assert_eq!(economical.units(Good::Bread), 0);
    }

    #[test]
    fn capped_meal_with_a_longer_drink_is_invariant_to_elapsed_chunks() {
        let mut source = Citizen::new(30.0)
            .unwrap()
            .with_good(Good::Berries, 1)
            .unwrap()
            .with_good(Good::Water, 200)
            .unwrap();
        source.diet = [1.0, 50.0, 100.0, 99.9];
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let whole = started.advance(60_000).unwrap();
        let chunks = started
            .advance(500)
            .unwrap()
            .advance(500)
            .unwrap()
            .advance(59_000)
            .unwrap();
        for category in DietCategory::ALL {
            close(whole.diet(category), chunks.diet(category));
        }
        close(
            whole.diet(DietCategory::Sweets),
            100.0 - 50.0 * 59_000.0 / DAY_MS as f64,
        );
    }
}
