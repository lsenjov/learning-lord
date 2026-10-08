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
            Self::BreadRoll => [0.0, 10.0, 0.0, 0.0],
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

    fn meal_wellbeing(&self, meal_units: [crate::Quantity; Good::COUNT]) -> Option<f64> {
        let mut predicted = self.clone();
        predicted.active_action = None;
        let mut meal_ms = 0;
        let mut meal_nutrition = 0.0;
        for good in Good::FOOD {
            let units = meal_units[good as usize];
            predicted.inventory.insert(good, self.units(good) - units);
            meal_ms += units * good.eating_ms_per_unit()?;
            meal_nutrition += units as f64 * good.nutrition_per_unit()?;
        }
        let drink_units = self.drink_units();
        predicted
            .inventory
            .insert(Good::Water, self.units(Good::Water) - drink_units);
        let duration_ms =
            meal_ms.max(drink_units * crate::DRINK_DURATION_MS / crate::DRINK_WATER_GRAMS);
        if duration_ms == 0 {
            return None;
        }
        let action = ActiveAction {
            action: crate::CitizenAction::Eat,
            remaining_ms: duration_ms,
            duration_ms,
            meal_duration_ms: meal_ms,
            drink_units,
            meal_nutrition,
            meal_units,
            travel_origin: self.position,
            travel_destination: None,
        };
        predicted.advance_needs(duration_ms, Some(action)).ok()?;
        predicted.personal_wellbeing().ok()
    }

    pub(crate) fn preferred_meal(&self) -> ([crate::Quantity; Good::COUNT], f64) {
        let mut candidates = Vec::new();
        self.mixed_meals(0, [0; Good::COUNT], 0.0, &mut candidates);
        let complete = candidates
            .iter()
            .any(|(_, nutrition)| *nutrition >= crate::MEAL_NOURISHMENT);
        let partial_nutrition = candidates
            .iter()
            .map(|(_, nutrition)| *nutrition)
            .fold(0.0, f64::max);
        candidates
            .into_iter()
            .filter(|(_, nutrition)| {
                if complete {
                    *nutrition >= crate::MEAL_NOURISHMENT
                } else {
                    *nutrition == partial_nutrition
                }
            })
            .filter_map(|(units, nutrition)| Some((units, nutrition, self.meal_wellbeing(units)?)))
            .max_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0)))
            .map_or(([0; Good::COUNT], 0.0), |(units, nutrition, _)| {
                (units, nutrition)
            })
    }

    fn mixed_meals(
        &self,
        index: usize,
        mut units: [crate::Quantity; Good::COUNT],
        nutrition: f64,
        candidates: &mut Vec<([crate::Quantity; Good::COUNT], f64)>,
    ) {
        const COUNTED: [Good; 4] = [Good::BreadRoll, Good::Fish, Good::Cabbage, Good::BerryPie];
        if index == COUNTED.len() {
            let berries = self.available_units(Good::Berries).min(
                ((crate::MEAL_NOURISHMENT - nutrition).max(0.0) / crate::BERRY_NUTRITION_PER_GRAM)
                    .ceil() as crate::Quantity,
            );
            units[Good::Berries as usize] = berries;
            let total = nutrition + berries as f64 * crate::BERRY_NUTRITION_PER_GRAM;
            if total > 0.0 {
                candidates.push((units, total));
            }
            return;
        }
        let good = COUNTED[index];
        let per_unit = good.nutrition_per_unit().unwrap();
        let limit = self
            .available_units(good)
            .min((crate::MEAL_NOURISHMENT / per_unit).ceil() as crate::Quantity);
        for count in 0..=limit {
            let total = nutrition + count as f64 * per_unit;
            units[good as usize] = count;
            if total > crate::MEAL_NOURISHMENT
                && COUNTED.into_iter().any(|food| {
                    units[food as usize] > 0
                        && total - food.nutrition_per_unit().unwrap() >= crate::MEAL_NOURISHMENT
                })
            {
                break;
            }
            self.mixed_meals(index + 1, units, total, candidates);
        }
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
            (Good::BreadRoll, 5),
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
            .with_good(Good::BreadRoll, 5)
            .unwrap()
            .with_good(Good::Fish, 2)
            .unwrap();
        source.diet = [0.0, 100.0, 0.0, 0.0];
        let chosen = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(chosen.units(Good::Fish), 0);
        assert_eq!(chosen.units(Good::BreadRoll), 5);
        source.prices = source.prices.with_price(Good::Fish, 100.0).unwrap();
        let economical = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(economical.units(Good::Fish), 2);
        assert_eq!(economical.units(Good::BreadRoll), 0);
    }

    #[test]
    fn mixed_whole_food_supplies_a_complete_meal() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::Fish, 1)
            .unwrap()
            .with_good(Good::Cabbage, 1)
            .unwrap();
        assert!(source.has_complete_meal());
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let active = started.active_action().unwrap();
        assert_eq!(active.meal_nutrition, 50.0);
        assert_eq!(active.meal_units[Good::Fish as usize], 1);
        assert_eq!(active.meal_units[Good::Cabbage as usize], 1);
        assert_eq!(source.units(Good::Fish), 1);
    }

    #[test]
    fn mixed_rolls_fish_and_berries_fill_the_remaining_nutrition() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::Fish, 1)
            .unwrap()
            .with_good(Good::BreadRoll, 2)
            .unwrap()
            .with_good(Good::Berries, 16)
            .unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let active = started.active_action().unwrap();
        assert_eq!(active.meal_units[Good::Fish as usize], 1);
        assert_eq!(active.meal_units[Good::BreadRoll as usize], 2);
        assert_eq!(active.meal_units[Good::Berries as usize], 16);
        close(
            active.meal_nutrition,
            45.0 + 16.0 * crate::BERRY_NUTRITION_PER_GRAM,
        );
    }

    #[test]
    fn minimal_whole_food_overfill_remains_a_complete_meal() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::Fish, 1)
            .unwrap()
            .with_good(Good::BreadRoll, 3)
            .unwrap();
        assert!(source.has_complete_meal());
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.active_action().unwrap().meal_nutrition, 55.0);
    }

    #[test]
    fn mixed_partial_meal_uses_available_food_without_phantom_nutrition() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::Fish, 1)
            .unwrap()
            .with_good(Good::Berries, 10)
            .unwrap();
        assert!(!source.has_complete_meal());
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let active = started.active_action().unwrap();
        assert_eq!(active.meal_units[Good::Fish as usize], 1);
        assert_eq!(active.meal_units[Good::Berries as usize], 10);
        close(
            active.meal_nutrition,
            25.0 + 10.0 * crate::BERRY_NUTRITION_PER_GRAM,
        );
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
