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
            Self::Beer => [0.0, 0.05, 0.0, 0.0],
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
        if let Some(action) = active {
            let elapsed = action.duration_ms - action.remaining_ms;
            if action.meal_duration_ms > elapsed {
                for good in Good::FOOD {
                    for (rate, intake) in rates.iter_mut().zip(good.diet_per_unit()) {
                        *rate += intake * action.meal_units[good as usize] as f64
                            / action.meal_duration_ms as f64;
                    }
                }
            }
            if action.drink_duration_ms() > elapsed {
                for (rate, intake) in rates.iter_mut().zip(action.drink_good.diet_per_unit()) {
                    *rate += intake * action.drink_units as f64 / action.drink_duration_ms() as f64;
                }
            }
        }
        rates
    }

    pub(crate) fn advance_diet(&mut self, elapsed_ms: u64, active: Option<ActiveAction>) {
        let decay = DIET_DECAY_PER_DAY / crate::marketplace::DAY_MS as f64;
        let mut elapsed = 0;
        while elapsed < elapsed_ms {
            let current = active.map(|mut action| {
                action.remaining_ms = action.remaining_ms.saturating_sub(elapsed);
                action
            });
            let step = current.map_or(elapsed_ms - elapsed, |a| {
                a.next_boundary_ms().min(elapsed_ms - elapsed)
            });
            if step == 0 {
                break;
            }
            let rates = self.diet_rates(current);
            for (value, rate) in self.diet.iter_mut().zip(rates) {
                *value = (*value + rate * step as f64).clamp(0.0, 100.0);
            }
            elapsed += step;
            if current.is_some_and(|a| step == a.remaining_ms) && elapsed < elapsed_ms {
                for value in &mut self.diet {
                    *value = (*value - decay * (elapsed_ms - elapsed) as f64).max(0.0);
                }
                break;
            }
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
        for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
            forecast.inventory.insert(good, available[good as usize]);
            forecast.tax_reserved.remove(&good);
        }
        let mut retained = forecast.food_forecast(6).units;
        retained[Good::Water as usize] = 0;
        let mut nutrition: f64 = Good::FOOD
            .into_iter()
            .map(|good| retained[good as usize] as f64 * good.nutrition_per_unit().unwrap())
            .sum::<f64>()
            + retained[Good::Beer as usize] as f64 * 0.05;
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
        if let Some(active) = self.active_action.filter(|a| {
            matches!(
                a.action,
                crate::CitizenAction::Eat | crate::CitizenAction::Drink
            )
        }) {
            let duration = active.remaining_ms.min(horizon);
            benefit +=
                future.forecast_consumption_interval(duration, Some(active), &mut consumed_value);
            elapsed = duration;
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
            let waiting = event.saturating_sub(elapsed);
            benefit += future.forecast_consumption_interval(waiting, None, &mut consumed_value);
            elapsed += waiting;
            let action = future.consumption_action(crate::CitizenAction::Eat);
            if action.duration_ms == 0 {
                continue;
            }
            for good in Good::FOOD {
                let portion = action.meal_units[good as usize];
                units[good as usize] += portion;
                future.inventory.insert(good, future.units(good) - portion);
            }
            units[action.drink_good as usize] += action.drink_units;
            future.inventory.insert(
                action.drink_good,
                future.units(action.drink_good) - action.drink_units,
            );
            let duration = action.duration_ms.min(horizon - elapsed);
            benefit +=
                future.forecast_consumption_interval(duration, Some(action), &mut consumed_value);
            elapsed += duration;
        }
        benefit +=
            future.forecast_consumption_interval(horizon - elapsed, None, &mut consumed_value);
        FoodForecast {
            benefit: ((benefit - passive) / horizon as f64).max(0.0),
            units,
        }
    }

    fn forecast_consumption_interval(
        &mut self,
        duration: u64,
        mut active: Option<ActiveAction>,
        consumed_value: &mut f64,
    ) -> f64 {
        let mut remaining = duration;
        let mut benefit = 0.0;
        while remaining > 0 {
            let step = active.map_or(remaining, |a| a.next_boundary_ms().min(remaining));
            if step == 0 {
                break;
            }
            let rates = self.diet_rates(active);
            for (value, rate) in self.diet.iter_mut().zip(rates) {
                let (integral, end) = diet_integral(*value, rate, step);
                benefit += integral;
                *value = end;
            }
            let mut value = 0.0;
            if let Some(action) = active {
                let passed = action.duration_ms - action.remaining_ms;
                let fraction = |component: u64| {
                    step.min(component.saturating_sub(passed)) as f64 / component.max(1) as f64
                };
                value += Good::FOOD
                    .into_iter()
                    .map(|g| {
                        self.prices
                            .value(g, action.meal_units[g as usize])
                            .unwrap_or(0.0)
                    })
                    .sum::<f64>()
                    * fraction(action.meal_duration_ms);
                value += self
                    .prices
                    .value(action.drink_good, action.drink_units)
                    .unwrap_or(0.0)
                    * fraction(action.drink_duration_ms());
                self.hunger -= action.meal_nutrition * fraction(action.meal_duration_ms)
                    + action.drink_nutrition() * fraction(action.drink_duration_ms());
                self.thirst -=
                    action.drink_units as f64 / 4.0 * fraction(action.drink_duration_ms());
                active = if step == action.remaining_ms {
                    None
                } else {
                    Some(ActiveAction {
                        remaining_ms: action.remaining_ms - step,
                        ..action
                    })
                };
            }
            value *= crate::WEALTH_WELLBEING_PER_COIN;
            benefit -= (*consumed_value + value * 0.5) * step as f64;
            *consumed_value += value;
            self.hunger += self.hunger_per_hour * step as f64 / 3_600_000.0;
            self.thirst =
                (self.thirst + crate::THIRST_PER_HOUR * step as f64 / 3_600_000.0).max(-100.0);
            remaining -= step;
        }
        benefit
    }

    #[cfg(test)]
    fn meal_wellbeing(&self, meal_units: [crate::Quantity; Good::COUNT]) -> Option<f64> {
        let (good, units) = self.preferred_drink();
        self.consumption_wellbeing(meal_units, good, units)
    }

    pub(crate) fn consumption_wellbeing(
        &self,
        meal_units: [crate::Quantity; Good::COUNT],
        drink_good: Good,
        drink_units: crate::Quantity,
    ) -> Option<f64> {
        let mut predicted = self.clone();
        predicted.active_action = None;
        let meal_duration_ms = Good::FOOD
            .into_iter()
            .map(|g| meal_units[g as usize] * g.eating_ms_per_unit().unwrap())
            .sum::<u64>();
        let meal_nutrition = Good::FOOD
            .into_iter()
            .map(|g| meal_units[g as usize] as f64 * g.nutrition_per_unit().unwrap())
            .sum();
        let duration_ms =
            meal_duration_ms.max(drink_units * crate::DRINK_DURATION_MS / crate::DRINK_WATER_GRAMS);
        if duration_ms == 0 {
            return None;
        }
        let action = ActiveAction {
            action: crate::CitizenAction::Eat,
            remaining_ms: duration_ms,
            duration_ms,
            meal_duration_ms,
            meal_nutrition,
            meal_units,
            drink_good,
            drink_units,
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
        let drink_value = self.prices.value(drink_good, drink_units).unwrap_or(0.0);
        let garment_value = self.prices.price(Good::FlaxGarment).unwrap_or(0.0)
            * (self.garment_condition.unwrap_or(0.0) - predicted.garment_condition.unwrap_or(0.0));
        let score = predicted.needs_wellbeing()
            - (food_value + drink_value + garment_value) * crate::WEALTH_WELLBEING_PER_COIN
            + crate::water_reserve_bonus((self.available_drink_units() - drink_units) as f64)
            - crate::water_reserve_bonus(self.available_drink_units() as f64);
        score.is_finite().then_some(score)
    }

    pub(crate) fn preferred_consumption(
        &self,
    ) -> ([crate::Quantity; Good::COUNT], f64, Good, crate::Quantity) {
        let mut candidates = Vec::new();
        let has_drink = [Good::Water, Good::Beer]
            .into_iter()
            .any(|good| self.drink_units_for(good) > 0);
        for drink_good in [Good::Water, Good::Beer] {
            let drink_units = self.drink_units_for(drink_good);
            if has_drink && drink_units == 0 {
                continue;
            }
            let drink_nutrition = if drink_good == Good::Beer {
                drink_units as f64 * 0.05
            } else {
                0.0
            };
            let mut foods = Vec::new();
            self.mixed_meals(0, [0; Good::COUNT], drink_nutrition, &mut foods);
            for (units, nutrition) in foods {
                candidates.push((
                    units,
                    nutrition - drink_nutrition,
                    drink_good,
                    drink_units,
                    nutrition,
                ));
            }
        }
        let maximum = candidates.iter().map(|c| c.4).fold(0.0, f64::max);
        let complete = maximum >= crate::MEAL_NOURISHMENT;
        candidates
            .into_iter()
            .filter(|c| {
                if complete {
                    c.4 >= crate::MEAL_NOURISHMENT
                } else {
                    c.4 == maximum
                }
            })
            .filter_map(|c| Some((c, self.consumption_wellbeing(c.0, c.2, c.3)?)))
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| a.0.0.cmp(&b.0.0))
                    .then_with(|| (b.0.2 as usize).cmp(&(a.0.2 as usize)))
            })
            .map_or(([0; Good::COUNT], 0.0, Good::Water, 0), |(c, _)| {
                (c.0, c.1, c.2, c.3)
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
    fn beer_drinking_progresses_nutrition_thirst_carbs_and_wealth() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_thirst(80.0)
            .unwrap()
            .with_good(Good::Beer, 200)
            .unwrap();
        let started = source.start_action(CitizenAction::Drink).unwrap();
        assert_eq!(started.units(Good::Beer), 0);
        close(started.wealth().unwrap(), source.wealth().unwrap());
        close(
            started.food_reserve_wellbeing(),
            source.food_reserve_wellbeing(),
        );
        close(started.drinking_water_reserve(), 200.0);
        let half = started.advance(30_000).unwrap();
        let elapsed_hours = 30_000.0 / 3_600_000.0;
        close(
            half.hunger(),
            80.0 + crate::HUNGER_PER_HOUR * elapsed_hours - 5.0,
        );
        close(
            half.thirst(),
            80.0 + crate::THIRST_PER_HOUR * elapsed_hours - 25.0,
        );
        close(
            half.diet(DietCategory::Carbs),
            5.0 - DIET_DECAY_PER_DAY * 30_000.0 / DAY_MS as f64,
        );
        close(half.drinking_water_reserve(), 100.0);
        close(
            half.wealth().unwrap(),
            source.wealth().unwrap() - source.prices.value(Good::Beer, 100).unwrap(),
        );
        let done = started.advance(60_000).unwrap();
        assert!(done.active_action().is_none());
        close(done.food_reserve_wellbeing(), 0.0);
        close(done.drinking_water_reserve(), 0.0);
    }

    #[test]
    fn beer_meals_share_forecast_selection_and_separate_component_timelines() {
        for beer in [100, 200] {
            let source = Citizen::new(80.0)
                .unwrap()
                .with_thirst(80.0)
                .unwrap()
                .with_good(Good::Beer, beer)
                .unwrap()
                .with_good(Good::BreadRoll, 10)
                .unwrap();
            let forecast = source.food_forecast(1);
            let started = source.start_action(CitizenAction::Eat).unwrap();
            let action = started.active_action().unwrap();
            assert_eq!(action.drink_good, Good::Beer);
            assert_eq!(action.meal_nutrition, if beer == 200 { 40.0 } else { 50.0 });
            assert_eq!(
                forecast.units[Good::BreadRoll as usize],
                action.meal_units[Good::BreadRoll as usize]
            );
            assert_eq!(forecast.units[Good::Beer as usize], beer);
            assert_eq!(
                source.action_duration_ms(CitizenAction::Eat).unwrap(),
                action.duration_ms
            );
            let whole = started.advance(action.duration_ms).unwrap();
            let split = started
                .advance(10_000)
                .unwrap()
                .advance(action.drink_duration_ms() - 10_000)
                .unwrap()
                .advance(action.duration_ms - action.drink_duration_ms())
                .unwrap();
            close(whole.hunger(), split.hunger());
            close(whole.thirst(), split.thirst());
            close(
                whole.diet(DietCategory::Carbs),
                split.diet(DietCategory::Carbs),
            );
            close(
                whole.personal_wellbeing().unwrap(),
                split.personal_wellbeing().unwrap(),
            );
            let drink_finished = started.advance(action.drink_duration_ms()).unwrap();
            close(drink_finished.drinking_water_reserve(), 0.0);
            let near_done = started.advance(action.duration_ms - 1).unwrap();
            assert!(
                (near_done.food_forecast(6).benefit - whole.food_forecast(6).benefit).abs() < 0.001
            );
        }
    }

    #[test]
    fn beer_selection_considers_cost_diet_and_thirst_room() {
        let mut source = Citizen::new(80.0)
            .unwrap()
            .with_thirst(80.0)
            .unwrap()
            .with_good(Good::BreadRoll, 10)
            .unwrap()
            .with_good(Good::Water, 200)
            .unwrap()
            .with_good(Good::Beer, 200)
            .unwrap();
        source.prices = source.prices.with_price(Good::Beer, 100.0).unwrap();
        assert_eq!(
            source.consumption_action(CitizenAction::Eat).drink_good,
            Good::Water
        );
        source.prices = source.prices.with_price(Good::Beer, 0.01).unwrap();
        assert_eq!(
            source.consumption_action(CitizenAction::Eat).drink_good,
            Good::Beer
        );
        source.thirst = -100.0;
        assert_eq!(source.consumption_action(CitizenAction::Eat).drink_units, 0);
        assert_eq!(
            source.consumption_action(CitizenAction::Eat).meal_nutrition,
            50.0
        );
        let beer_only = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::Beer, 10_000)
            .unwrap();
        assert!(!beer_only.has_complete_meal());
    }

    #[test]
    fn beer_consumption_excludes_tax_and_town_stock_and_retains_shared_reserves() {
        let mut source = Citizen::new(80.0)
            .unwrap()
            .with_thirst(80.0)
            .unwrap()
            .with_good(Good::Beer, 250)
            .unwrap();
        source.tax_reserved.insert(Good::Beer, 200);
        source.town_inventory.insert(Good::Beer, 1000);
        let started = source.start_action(CitizenAction::Drink).unwrap();
        assert_eq!(started.active_action().unwrap().drink_units, 50);
        assert_eq!(started.units(Good::Beer), 200);
        assert_eq!(started.town_inventory[&Good::Beer], 1000);
        close(started.drinking_water_reserve(), 50.0);
        let reserves = source.reserved_goods().unwrap();
        assert!(reserves.units(Good::Beer) <= 50);
        source.tax_reserved.insert(Good::Beer, 250);
        assert!(
            source
                .start_action(CitizenAction::Drink)
                .unwrap()
                .active_action()
                .is_none()
        );
        close(source.drinking_water_reserve(), 0.0);
        let mixed = Citizen::new(80.0)
            .unwrap()
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 200)
            .unwrap()
            .with_good(Good::Beer, 200)
            .unwrap();
        let retained = mixed.reserved_goods().unwrap();
        assert_eq!(
            retained.units(Good::Water) + retained.units(Good::Beer),
            400
        );
        assert_eq!(retained.units(Good::Beer), 200);
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
    fn meal_scores_match_consumption_predictions_without_food_reserve_changes() {
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
                drink_good: Good::Water,
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
                    - predicted.personal_wellbeing().unwrap()
                    + predicted.food_reserve_wellbeing()
                    - source.food_reserve_wellbeing())
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
