use crate::{
    AgentId, Citizen, CitizenAction, Coins, Quantity, SimulationError,
    marketplace::{Good, Trade},
    production,
};
use imbl::Vector;

pub const COMPLETED_DAYS: usize = 30;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActivityTimes {
    pub production_ms: u64,
    pub travel_ms: u64,
    pub sleep_ms: u64,
    pub trade_ms: u64,
    pub eating_ms: u64,
    pub idle_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DailyCitizenHistory {
    pub start_ms: u64,
    pub end_ms: u64,
    pub elapsed_ms: u64,
    pub activity: ActivityTimes,
    pub food_goal_ms: u64,
    pub produced: [Quantity; Good::COUNT],
    pub bought: [Quantity; Good::COUNT],
    pub sold: [Quantity; Good::COUNT],
    pub consumed: [Quantity; Good::COUNT],
    pub coins_earned: Coins,
    pub coins_spent: Coins,
    pub worst_hunger: f64,
    pub worst_tiredness: f64,
    pub worst_clothing_need: f64,
    mean_wellbeing: f64,
}

impl DailyCitizenHistory {
    fn new(now: u64, citizen: &Citizen) -> Self {
        Self {
            start_ms: now,
            end_ms: now,
            elapsed_ms: 0,
            activity: ActivityTimes::default(),
            food_goal_ms: 0,
            produced: [0; Good::COUNT],
            bought: [0; Good::COUNT],
            sold: [0; Good::COUNT],
            consumed: [0; Good::COUNT],
            coins_earned: 0,
            coins_spent: 0,
            worst_hunger: citizen.hunger(),
            worst_tiredness: citizen.tiredness(),
            worst_clothing_need: citizen.clothing_need(),
            mean_wellbeing: 0.0,
        }
    }

    pub fn average_wellbeing(&self) -> Option<f64> {
        (self.elapsed_ms > 0).then_some(self.mean_wellbeing)
    }

    fn observe(&mut self, citizen: &Citizen) {
        self.worst_hunger = self.worst_hunger.max(citizen.hunger());
        self.worst_tiredness = self.worst_tiredness.max(citizen.tiredness());
        self.worst_clothing_need = self.worst_clothing_need.max(citizen.clothing_need());
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CitizenHistory {
    pub current: DailyCitizenHistory,
    pub completed: Vector<DailyCitizenHistory>,
}

impl CitizenHistory {
    pub(crate) fn new(now: u64, citizen: &Citizen) -> Self {
        Self {
            current: DailyCitizenHistory::new(now, citizen),
            completed: Vector::new(),
        }
    }

    pub(crate) fn start_action(&mut self, citizen: &Citizen) -> Result<(), SimulationError> {
        if let Some(active) = citizen.active_action()
            && active.action() == CitizenAction::Eat
        {
            for good in Good::FOOD {
                self.current.consumed[good as usize] = self.current.consumed[good as usize]
                    .checked_add(active.meal_units[good as usize])
                    .ok_or(SimulationError::InventoryOverflow)?;
            }
        }
        self.current.observe(citizen);
        Ok(())
    }

    pub(crate) fn advance(
        &mut self,
        before: &Citizen,
        after: &Citizen,
        elapsed: u64,
        end: u64,
    ) -> Result<(), SimulationError> {
        let active = before.active_action();
        let action = active.map(|a| a.action());
        let target = match action {
            Some(CitizenAction::Produce(_)) => &mut self.current.activity.production_ms,
            Some(CitizenAction::Travel(_)) => &mut self.current.activity.travel_ms,
            Some(CitizenAction::Sleep) => &mut self.current.activity.sleep_ms,
            Some(CitizenAction::Eat) => &mut self.current.activity.eating_ms,
            Some(
                CitizenAction::BuyFood(_)
                | CitizenAction::Buy(_)
                | CitizenAction::BuyAt { .. }
                | CitizenAction::ListExcess
                | CitizenAction::List(..)
                | CitizenAction::Withdraw(..),
            ) => &mut self.current.activity.trade_ms,
            _ => &mut self.current.activity.idle_ms,
        };
        *target += elapsed;
        if before.active_plan().is_some_and(|plan| {
            plan.plan().goals().iter().any(|goal| {
                goal.actions.contains(&plan.action_index())
                    && matches!(
                        goal.goal,
                        crate::planning::goals::Effect::ReduceHunger
                            | crate::planning::goals::Effect::ReplenishReserves
                    )
            })
        }) {
            self.current.food_goal_ms += elapsed;
        }
        self.current.elapsed_ms += elapsed;
        self.current.end_ms = end;
        self.current.observe(before);
        let mut endpoint = before.clone();
        endpoint.advance_needs(elapsed, active)?;
        if let Some(mut action) = endpoint.active_action {
            action.remaining_ms = action.remaining_ms.saturating_sub(elapsed);
            endpoint.active_action = Some(action);
        }
        self.current.observe(&endpoint);
        self.current.observe(after);
        let weight = elapsed as f64 / self.current.elapsed_ms as f64;
        self.current.mean_wellbeing = self.current.mean_wellbeing * (1.0 - weight)
            + wellbeing_average(before, &endpoint, elapsed)? * weight;
        if !self.current.mean_wellbeing.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        if active.is_some_and(|a| a.remaining_ms() == elapsed) {
            match action {
                Some(CitizenAction::Produce(recipe)) => {
                    for &(good, units) in recipe.inputs() {
                        self.current.consumed[good as usize] = self.current.consumed[good as usize]
                            .checked_add(units)
                            .ok_or(SimulationError::InventoryOverflow)?;
                    }
                    for &(good, _) in recipe.outputs() {
                        let inputs = recipe
                            .inputs()
                            .iter()
                            .filter(|(g, _)| *g == good)
                            .map(|(_, u)| *u)
                            .sum::<u64>();
                        self.current.produced[good as usize] = self.current.produced[good as usize]
                            .checked_add(after.units(good) - (before.units(good) - inputs))
                            .ok_or(SimulationError::InventoryOverflow)?;
                    }
                }
                Some(CitizenAction::EquipClothing) => {
                    self.current.consumed[Good::FlaxGarment as usize] = self.current.consumed
                        [Good::FlaxGarment as usize]
                        .checked_add(1)
                        .ok_or(SimulationError::InventoryOverflow)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn trade(&mut self, id: AgentId, trade: &Trade) -> Result<(), SimulationError> {
        if trade.buyer == id {
            self.current.bought[trade.good as usize] = self.current.bought[trade.good as usize]
                .checked_add(trade.units)
                .ok_or(SimulationError::InventoryOverflow)?;
            self.current.coins_spent = self
                .current
                .coins_spent
                .checked_add(trade.coins)
                .ok_or(SimulationError::WealthOverflow)?;
        }
        if trade.seller == id {
            self.current.sold[trade.good as usize] = self.current.sold[trade.good as usize]
                .checked_add(trade.units)
                .ok_or(SimulationError::InventoryOverflow)?;
            self.current.coins_earned = self
                .current
                .coins_earned
                .checked_add(trade.coins)
                .ok_or(SimulationError::WealthOverflow)?;
        }
        Ok(())
    }

    pub(crate) fn close_day(&mut self, now: u64, citizen: &Citizen) {
        if production::work_period(now) != production::work_period(self.current.start_ms) {
            let next = DailyCitizenHistory::new(now, citizen);
            self.completed
                .push_back(std::mem::replace(&mut self.current, next));
            if self.completed.len() > COMPLETED_DAYS {
                self.completed.pop_front();
            }
        }
    }
}

// Wellbeing is piecewise linear between resource changes; split at its need/reserve clamps.
fn wellbeing_average(start: &Citizen, end: &Citizen, elapsed: u64) -> Result<f64, SimulationError> {
    let mut fractions = vec![0.0, 1.0];
    let mut split = |from: f64, to: f64, thresholds: &[f64]| {
        for threshold in thresholds {
            let fraction = (threshold - from) / (to - from);
            if fraction > 0.0 && fraction < 1.0 {
                fractions.push(fraction);
            }
        }
    };
    split(start.hunger(), end.hunger(), &[-100.0, 0.0, 100.0]);
    split(start.tiredness(), end.tiredness(), &[0.0]);
    let reserve = |citizen: &Citizen| {
        citizen.food_nutrition()
            + citizen.active_action.map_or(0.0, |a| {
                a.meal_nutrition * a.remaining_ms as f64 / a.duration_ms as f64
            })
    };
    split(
        reserve(start),
        reserve(end),
        &[
            crate::FOOD_RESERVE_FULL_BONUS_NUTRITION,
            crate::FOOD_RESERVE_CAP_NUTRITION,
        ],
    );
    if let Some(condition) = start.garment_condition {
        split(
            condition,
            condition - elapsed as f64 / crate::GARMENT_LIFETIME_MS as f64,
            &[0.0],
        );
    }
    fractions.sort_by(f64::total_cmp);
    let wellbeing = |fraction: f64| -> Result<f64, SimulationError> {
        let mut citizen = start.clone();
        citizen.hunger = start.hunger + (end.hunger - start.hunger) * fraction;
        citizen.tiredness = start.tiredness + (end.tiredness - start.tiredness) * fraction;
        citizen.garment_condition = start.garment_condition.map(|value| {
            (value - elapsed as f64 * fraction / crate::GARMENT_LIFETIME_MS as f64).max(0.0)
        });
        // Keep interpolation fractional so millisecond rounding cannot bias the daily average.
        let mut value = citizen.personal_wellbeing()?;
        if let Some(active) = start.active_action {
            let remaining = active.remaining_ms as f64 - elapsed as f64 * fraction;
            let ratio = remaining.max(0.0) / active.duration_ms as f64;
            let initial_ratio = active.remaining_ms as f64 / active.duration_ms as f64;
            let meal_value: f64 = Good::FOOD
                .into_iter()
                .map(|good| {
                    citizen
                        .prices
                        .value(good, active.meal_units[good as usize])
                        .unwrap_or(0.0)
                })
                .sum();
            value += meal_value * (ratio - initial_ratio) * crate::WEALTH_WELLBEING_PER_COIN;
            let nutrition = citizen.food_nutrition() + active.meal_nutrition * ratio;
            let bonus = 0.10 * nutrition.min(crate::FOOD_RESERVE_FULL_BONUS_NUTRITION)
                + 0.02
                    * (nutrition.min(crate::FOOD_RESERVE_CAP_NUTRITION)
                        - crate::FOOD_RESERVE_FULL_BONUS_NUTRITION)
                        .max(0.0);
            value += bonus - citizen.food_reserve_wellbeing();
        }
        Ok(value)
    };
    let mut total = 0.0;
    for pair in fractions.windows(2) {
        total += (wellbeing(pair[0])? * 0.5 + wellbeing(pair[1])? * 0.5) * (pair[1] - pair[0]);
    }
    Ok(total)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DailyPlanningHistory {
    pub start_ms: u64,
    pub end_ms: u64,
    pub request_ms: f64,
    pub wait_ms: f64,
    pub requests: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanningHistory {
    pub current: DailyPlanningHistory,
    pub completed: Vector<DailyPlanningHistory>,
}

impl PlanningHistory {
    pub(crate) fn new(now: u64) -> Self {
        Self {
            current: DailyPlanningHistory {
                start_ms: now,
                end_ms: now,
                ..Default::default()
            },
            completed: Vector::new(),
        }
    }

    pub(crate) fn advance(&mut self, now: u64) {
        while production::work_period(self.current.start_ms) < production::work_period(now) {
            self.current.end_ms =
                production::period_start(production::work_period(self.current.start_ms) + 1);
            let next = DailyPlanningHistory {
                start_ms: self.current.end_ms,
                end_ms: self.current.end_ms,
                ..Default::default()
            };
            self.completed
                .push_back(std::mem::replace(&mut self.current, next));
            if self.completed.len() > COMPLETED_DAYS {
                self.completed.pop_front();
            }
        }
        self.current.end_ms = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentKind, TRADE_DURATION_MS, Universe, marketplace::ShoppingList};

    #[test]
    fn seller_rounding_gain_is_not_applied_before_the_sale_or_by_id_order() {
        for buyer_first in [true, false] {
            let mut buyer = Citizen::new(0.0).unwrap().with_coins(100).unwrap();
            let mut seller = Citizen::new(0.0).unwrap().with_berries(1000).unwrap();
            buyer.id = AgentId(uuid::Uuid::from_u128(if buyer_first { 1 } else { 2 }));
            seller.id = AgentId(uuid::Uuid::from_u128(if buyer_first { 2 } else { 1 }));
            let (world, buyer) = Universe::with_map(crate::locations::Map::default())
                .with_citizen("Buyer", buyer)
                .unwrap();
            let (world, seller) = world.with_citizen("Seller", seller).unwrap();
            let world = world
                .start_action(seller, CitizenAction::List(Good::Berries, 1000))
                .unwrap()
                .advance(TRADE_DURATION_MS)
                .unwrap();
            let world = world
                .start_action(
                    buyer,
                    CitizenAction::Buy(ShoppingList::single(Good::Berries, 100)),
                )
                .unwrap();
            let whole = world.advance(TRADE_DURATION_MS).unwrap();
            let split = world
                .advance(TRADE_DURATION_MS / 2)
                .unwrap()
                .advance(TRADE_DURATION_MS / 2)
                .unwrap();
            let average = |u: &Universe| {
                u.citizen_history(seller)
                    .unwrap()
                    .current
                    .average_wellbeing()
                    .unwrap()
            };
            assert!((average(&whole) - average(&split)).abs() < 1e-10);
            assert_eq!(
                whole.citizen_history(seller).unwrap().current.coins_earned,
                1
            );
        }
    }

    #[test]
    fn planned_meals_count_time_towards_food_goals_without_double_counting_activity() {
        let citizen = Citizen::new(80.0).unwrap().with_berries(310).unwrap();
        let (world, id) = Universe::with_map(citizen.map())
            .with_citizen("Hungry", citizen)
            .unwrap();
        let started = world.start_planning(id).unwrap();
        let AgentKind::Citizen(citizen) = &started.agents()[&id].kind;
        assert_eq!(
            citizen.active_action().unwrap().action(),
            CitizenAction::Eat
        );
        let next = started.advance(1000).unwrap();
        let day = &next.citizen_history(id).unwrap().current;
        assert_eq!(day.food_goal_ms, 1000);
        assert_eq!(day.activity.eating_ms, 1000);
        assert_eq!(day.elapsed_ms, 1000);
    }
}
