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

fn diet_integral(start: f64, rate: f64, duration: u64) -> (f64, f64) {
    let duration = duration as f64;
    let mut boundaries = vec![0.0, duration];
    if rate != 0.0 {
        for threshold in [0.0, 50.0, 100.0] {
            let time = (threshold - start) / rate;
            if time > 0.0 && time < duration {
                boundaries.push(time);
            }
        }
    }
    boundaries.sort_by(f64::total_cmp);
    let integral = boundaries
        .windows(2)
        .map(|pair| {
            let from = category_bonus(start + rate * pair[0]);
            let to = category_bonus(start + rate * pair[1]);
            (from + to) * 0.5 * (pair[1] - pair[0])
        })
        .sum();
    (integral, (start + rate * duration).clamp(0.0, 100.0))
}

#[derive(Default)]
pub(crate) struct FoodForecast {
    pub benefit: f64,
    pub units: [crate::Quantity; Good::COUNT],
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

    pub(crate) fn planning_wellbeing(&self) -> Result<f64, crate::SimulationError> {
        let value = self.personal_wellbeing()? + self.food_forecast(6).benefit;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(crate::SimulationError::WellbeingOverflow)
        }
    }

    pub(crate) fn retained_food(
        &self,
        available: [crate::Quantity; Good::COUNT],
    ) -> [crate::Quantity; Good::COUNT] {
        let mut forecast = self.clone();
        forecast.active_action = None;
        forecast.active_plan = None;
        for good in Good::FOOD {
            forecast.inventory.insert(good, available[good as usize]);
            forecast.tax_reserved.remove(&good);
        }
        let mut retained = forecast.food_forecast(6).units;
        let mut nutrition: f64 = Good::FOOD
            .into_iter()
            .map(|good| retained[good as usize] as f64 * good.nutrition_per_unit().unwrap())
            .sum();
        for good in Good::FOOD {
            let remaining = (crate::FOOD_RESERVE_CAP_NUTRITION - nutrition).max(0.0);
            let added = available[good as usize]
                .saturating_sub(retained[good as usize])
                .min((remaining / good.nutrition_per_unit().unwrap()).ceil() as crate::Quantity);
            retained[good as usize] += added;
            nutrition += added as f64 * good.nutrition_per_unit().unwrap();
        }
        retained
    }

    pub(crate) fn food_forecast(&self, meals: usize) -> FoodForecast {
        let horizon = 3 * crate::marketplace::DAY_MS;
        let decay = DIET_DECAY_PER_DAY / crate::marketplace::DAY_MS as f64;
        let passive: f64 = self
            .diet
            .into_iter()
            .map(|value| diet_integral(value, -decay, horizon).0)
            .sum();
        let mut future = self.clone();
        future.active_action = None;
        future.active_plan = None;
        let mut elapsed = 0;
        let mut benefit = 0.0;
        let mut consumed_value = 0.0;
        let mut units = [0; Good::COUNT];
        if let Some(active) = self.active_action {
            let remaining = active
                .meal_duration_ms
                .saturating_sub(active.duration_ms - active.remaining_ms);
            if remaining > 0 {
                for (value, rate) in future.diet.iter_mut().zip(self.diet_rates(Some(active))) {
                    let (integral, end) = diet_integral(*value, rate, remaining);
                    benefit += integral;
                    *value = end;
                }
                future.hunger += future.hunger_per_hour * remaining as f64 / 3_600_000.0
                    - active.meal_nutrition * remaining as f64 / active.meal_duration_ms as f64;
                consumed_value = Good::FOOD
                    .into_iter()
                    .map(|good| {
                        self.prices
                            .value(good, active.meal_units[good as usize])
                            .unwrap_or(0.0)
                    })
                    .sum::<f64>()
                    * crate::WEALTH_WELLBEING_PER_COIN
                    * remaining as f64
                    / active.meal_duration_ms as f64;
                benefit -= consumed_value * 0.5 * remaining as f64;
                elapsed = remaining;
            }
        }
        let interval = crate::marketplace::DAY_MS / 2;
        let next_meal = self.last_meal_started_ms.map_or(0, |started| {
            interval.saturating_sub(self.market_time_ms.saturating_sub(started))
        });
        for index in 0..meals {
            let event = next_meal + index as u64 * interval;
            if event >= horizon {
                break;
            }
            let waiting = event - elapsed;
            benefit += future.forecast_diet_interval(waiting, [0; Good::COUNT], waiting)
                - consumed_value * waiting as f64;
            future.hunger += future.hunger_per_hour * waiting as f64 / 3_600_000.0;
            let (meal, nutrition) = future.preferred_meal();
            let full_duration: u64 = Good::FOOD
                .into_iter()
                .map(|good| meal[good as usize] * good.eating_ms_per_unit().unwrap())
                .sum();
            let duration = full_duration.min(horizon - event);
            let fraction = duration as f64 / full_duration.max(1) as f64;
            let mut meal_value = 0.0;
            for good in Good::FOOD {
                let portion = meal[good as usize];
                units[good as usize] += portion;
                meal_value += future.prices.value(good, portion).unwrap_or(0.0)
                    * crate::WEALTH_WELLBEING_PER_COIN;
                future.inventory.insert(good, future.units(good) - portion);
            }
            meal_value *= fraction;
            benefit += future.forecast_diet_interval(duration, meal, full_duration)
                - (consumed_value + meal_value * 0.5) * duration as f64;
            consumed_value += meal_value;
            future.hunger +=
                future.hunger_per_hour * duration as f64 / 3_600_000.0 - nutrition * fraction;
            elapsed = event + duration;
        }
        benefit +=
            future.forecast_diet_interval(horizon - elapsed, [0; Good::COUNT], horizon - elapsed)
                - consumed_value * (horizon - elapsed) as f64;
        FoodForecast {
            benefit: ((benefit - passive) / horizon as f64).max(0.0),
            units,
        }
    }

    fn forecast_diet_interval(
        &mut self,
        duration: u64,
        meal: [crate::Quantity; Good::COUNT],
        meal_duration: u64,
    ) -> f64 {
        if duration == 0 {
            return 0.0;
        }
        let mut rates = [-DIET_DECAY_PER_DAY / crate::marketplace::DAY_MS as f64; 4];
        for good in Good::FOOD {
            for (rate, intake) in rates.iter_mut().zip(good.diet_per_unit()) {
                *rate += intake * meal[good as usize] as f64 / meal_duration.max(1) as f64;
            }
        }
        self.diet
            .iter_mut()
            .zip(rates)
            .map(|(value, rate)| {
                let (integral, end) = diet_integral(*value, rate, duration);
                *value = end;
                integral
            })
            .sum()
    }

    fn meal_wellbeing(&self, meal_units: [crate::Quantity; Good::COUNT]) -> Option<f64> {
        let mut predicted = self.clone();
        predicted.active_action = None;
        let mut meal_ms = 0;
        let mut meal_nutrition = 0.0;
        for good in Good::FOOD {
            let units = meal_units[good as usize];
            meal_ms += units * good.eating_ms_per_unit()?;
            meal_nutrition += units as f64 * good.nutrition_per_unit()?;
        }
        let drink_units = self.drink_units();
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
        let food_value: f64 = Good::FOOD
            .into_iter()
            .map(|good| {
                self.prices
                    .value(good, meal_units[good as usize])
                    .unwrap_or(0.0)
            })
            .sum();
        let water_value = self.prices.value(Good::Water, drink_units).unwrap_or(0.0);
        let nutrition = self.food_nutrition();
        let garment_value = self.prices.price(Good::FlaxGarment).unwrap_or(0.0)
            * (self.garment_condition.unwrap_or(0.0) - predicted.garment_condition.unwrap_or(0.0));
        let score = predicted.needs_wellbeing()
            - (food_value + water_value + garment_value) * crate::WEALTH_WELLBEING_PER_COIN
            + crate::food_reserve_bonus(nutrition - meal_nutrition)
            - crate::food_reserve_bonus(nutrition)
            + crate::water_reserve_bonus((self.available_units(Good::Water) - drink_units) as f64)
            - crate::water_reserve_bonus(self.available_units(Good::Water) as f64);
        score.is_finite().then_some(score)
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
    fn lightweight_meal_scores_match_full_consumption_predictions() {
        let mut source = Citizen::with_needs(140.0, 20.0)
            .unwrap()
            .with_good(Good::BreadRoll, 10)
            .unwrap()
            .with_good(Good::Fish, 2)
            .unwrap()
            .with_good(Good::Cabbage, 2)
            .unwrap()
            .with_good(Good::BerryPie, 1)
            .unwrap()
            .with_good(Good::Berries, 200)
            .unwrap()
            .with_good(Good::Water, 300)
            .unwrap();
        source.diet = [95.0, 48.0, 0.0, 52.0];
        source.garment_condition = Some(0.4);
        source.tax_reserved.insert(Good::BreadRoll, 1);
        let mut candidates = Vec::new();
        source.mixed_meals(0, [0; Good::COUNT], 0.0, &mut candidates);
        for (meal, nutrition) in candidates {
            let mut predicted = source.clone();
            let meal_ms: u64 = Good::FOOD
                .into_iter()
                .map(|good| meal[good as usize] * good.eating_ms_per_unit().unwrap())
                .sum();
            for good in Good::FOOD {
                predicted
                    .inventory
                    .insert(good, source.units(good) - meal[good as usize]);
            }
            let drink = source.drink_units();
            predicted
                .inventory
                .insert(Good::Water, source.units(Good::Water) - drink);
            let duration = meal_ms.max(drink * crate::DRINK_DURATION_MS / crate::DRINK_WATER_GRAMS);
            let active = ActiveAction {
                action: CitizenAction::Eat,
                duration_ms: duration,
                remaining_ms: duration,
                meal_duration_ms: meal_ms,
                drink_units: drink,
                meal_nutrition: nutrition,
                meal_units: meal,
                travel_origin: source.position,
                travel_destination: None,
            };
            predicted.advance_needs(duration, Some(active)).unwrap();
            assert!(
                (source.meal_wellbeing(meal).unwrap()
                    + source.wealth().unwrap() * crate::WEALTH_WELLBEING_PER_COIN
                    - predicted.personal_wellbeing().unwrap())
                .abs()
                    < 1e-10
            );
        }
    }

    #[test]
    fn starting_a_meal_does_not_add_a_seventh_forecast_meal() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_prices(
                crate::marketplace::Prices::default()
                    .with_price(Good::BreadRoll, 0.1)
                    .unwrap(),
            )
            .with_good(Good::BreadRoll, 35)
            .unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        close(
            source.food_forecast(6).benefit,
            started.food_forecast(6).benefit,
        );
        assert_eq!(source.food_forecast(6).units[Good::BreadRoll as usize], 30);
        assert_eq!(started.food_forecast(6).units[Good::BreadRoll as usize], 25);
    }

    #[test]
    fn food_forecast_is_continuous_at_meal_completion() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_prices(
                crate::marketplace::Prices::default()
                    .with_price(Good::BreadRoll, 0.1)
                    .unwrap(),
            )
            .with_good(Good::BreadRoll, 35)
            .unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let duration = started.active_action().unwrap().duration_ms();
        let almost = started.advance(duration - 1).unwrap();
        let complete = almost.advance(1).unwrap();
        assert!(
            (almost.food_forecast(6).benefit - complete.food_forecast(6).benefit).abs()
                < started.active_action().unwrap().meal_nutrition / duration as f64 * 0.2,
            "{} != {} at {} -> {}",
            almost.food_forecast(6).benefit,
            complete.food_forecast(6).benefit,
            almost.market_time_ms,
            complete.market_time_ms
        );
        assert_eq!(
            complete.food_forecast(6).units[Good::BreadRoll as usize],
            30
        );
        assert_eq!(complete.units(Good::BreadRoll), 30);
    }

    #[test]
    fn dietary_retention_keeps_variety_and_lists_excess_rolls() {
        let mut source = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::BreadRoll, 70)
            .unwrap()
            .with_good(Good::Fish, 12)
            .unwrap();
        source.diet = [0.0, 100.0, 0.0, 0.0];
        let retained = source.reserved_goods().unwrap();
        assert!(retained.units(Good::Fish) > 0);
        let nutrition: f64 = retained
            .items()
            .filter_map(|(good, units)| Some(good.nutrition_per_unit()? * units as f64))
            .sum();
        assert!((300.0..=360.0).contains(&nutrition));
        assert!(source.excess_goods().unwrap().units(Good::BreadRoll) > 0);
    }

    #[test]
    fn food_forecast_values_future_diet_without_mutating_the_source() {
        let source = Citizen::new(-50.0)
            .unwrap()
            .with_good(Good::BreadRoll, 100)
            .unwrap();
        let before = source.clone();
        let value = source
            .with_prices(source.prices.with_price(Good::BreadRoll, 0.1).unwrap())
            .food_forecast(6)
            .benefit;
        assert!(value > 0.0 && value <= 28.0);
        assert_eq!(source, before);
        let empty = Citizen::new(-50.0).unwrap();
        close(empty.food_forecast(6).benefit, 0.0);
        let taxed = {
            let mut citizen = source.clone();
            citizen.tax_reserved.insert(Good::BreadRoll, 100);
            citizen
        };
        close(taxed.food_forecast(6).benefit, 0.0);
    }

    #[test]
    fn forecast_integration_observes_decay_bonus_thresholds_and_caps() {
        let day = crate::marketplace::DAY_MS;
        let rate = DIET_DECAY_PER_DAY / day as f64;
        let (integral, end) = diet_integral(100.0, -rate, 3 * day);
        close(end, 0.0);
        close(integral / day as f64, 8.5);
        let (integral, end) = diet_integral(90.0, rate, day);
        close(end, 100.0);
        close(integral / day as f64, 6.96);
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
