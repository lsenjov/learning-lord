use super::{EAT_PLAN_COOLDOWN_MS, REPLAN_MIN_NUTRITION};
use crate::{
    ACTION_DURATION_MS, BERRY_EATING_MS_PER_GRAM, BERRY_NUTRITION_PER_GRAM, Citizen, CitizenAction,
    MEAL_NOURISHMENT, SLEEP_DURATION_MS, SimulationError, TRADE_DURATION_MS, marketplace::Good,
};

pub const GOAL_HORIZON_MS: u64 = 4 * 60 * 60 * 1000;
pub const TRADE_PLAN_COOLDOWN_MS: u64 = 2 * 60 * 60 * 1000;
const WAIT_GOAL_DURATION_MS: u64 = 2 * 60 * 60 * 1000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Food,
    Coins,
    ReduceHunger,
    ReduceThirst,
    ReduceTiredness,
    ReduceClothingNeed,
    ReplenishReserves,
    Production,
    ListExcess,
    Wait,
}

#[derive(Clone, Copy, Debug)]
pub struct Requirement {
    pub resource: Effect,
    pub amount: f64,
    pub coins: Option<crate::Coins>,
}

impl CitizenAction {
    pub fn effects(self) -> &'static [Effect] {
        use Effect::*;
        match self {
            Self::Eat => &[ReduceHunger],
            Self::Drink => &[ReduceThirst],
            Self::Sleep => &[ReduceTiredness],
            Self::Wait => &[Effect::Wait],
            Self::EquipClothing => &[ReduceClothingNeed],
            Self::BuyFood(good) if good.nutrition_per_unit().is_some() => {
                &[Food, ReplenishReserves]
            }
            Self::BuyFood(_) => &[],
            Self::Produce(crate::production::Recipe::FetchWater)
            | Self::ProduceAt {
                recipe: crate::production::Recipe::FetchWater,
                ..
            } => &[ReplenishReserves, Production],
            Self::Withdraw(Good::Water | Good::Beer, _)
            | Self::WithdrawAt {
                good: Good::Water | Good::Beer,
                ..
            } => &[ReplenishReserves],
            Self::Buy(list) | Self::BuyAt { list, .. }
                if (list.units(Good::Water) > 0 || list.units(Good::Beer) > 0)
                    && !list
                        .items()
                        .any(|(good, _)| good.nutrition_per_unit().is_some()) =>
            {
                &[ReplenishReserves]
            }
            Self::Produce(recipe) | Self::ProduceAt { recipe, .. }
                if recipe
                    .outputs()
                    .iter()
                    .any(|(good, _)| good.nutrition_per_unit().is_some()) =>
            {
                &[Food, ReplenishReserves, Production]
            }
            Self::Withdraw(good, _) | Self::WithdrawAt { good, .. }
                if good.nutrition_per_unit().is_some() =>
            {
                &[Food, ReplenishReserves]
            }
            Self::Buy(list) | Self::BuyAt { list, .. }
                if list
                    .items()
                    .any(|(good, _)| good.nutrition_per_unit().is_some()) =>
            {
                &[Food, ReplenishReserves]
            }
            Self::DepositStored { .. } => &[ReplenishReserves],
            Self::Produce(_)
            | Self::ProduceAt { .. }
            | Self::Plant { .. }
            | Self::Tend { .. }
            | Self::Harvest { .. }
            | Self::Clear { .. }
            | Self::CollectBeer { .. } => &[Production],
            Self::Travel(_)
            | Self::ListExcess
            | Self::List(..)
            | Self::ListAt { .. }
            | Self::Buy(..)
            | Self::BuyAt { .. }
            | Self::Withdraw(..)
            | Self::WithdrawAt { .. }
            | Self::CollectStored { .. } => &[],
        }
    }

    /// Resources needed to supply the requested quantity during prediction.
    pub fn input_for(self, citizen: &Citizen, amount: f64) -> Option<Requirement> {
        if self == Self::Eat {
            return Some(Requirement {
                resource: Effect::Food,
                amount,
                coins: None,
            });
        }
        let coins = match self {
            Self::BuyFood(good) => citizen.market().purchase_cost(
                citizen.market_party(),
                good,
                ((amount - citizen.available_food_nutrition()).max(0.0)
                    / good.nutrition_per_unit()?)
                .ceil() as crate::Quantity,
            ),
            Self::Buy(list) => list.items().try_fold(0_i64, |sum, (good, units)| {
                sum.checked_add(citizen.market().purchase_cost(
                    citizen.market_party(),
                    good,
                    units,
                )?)
            }),
            Self::BuyAt { place, list } => list.items().try_fold(0_i64, |sum, (good, units)| {
                sum.checked_add(citizen.market().purchase_cost_at(
                    citizen.market_party(),
                    place,
                    good,
                    units,
                )?)
            }),
            _ => return None,
        };
        Some(Requirement {
            resource: Effect::Coins,
            amount: coins.map_or(f64::INFINITY, |coins| coins as f64),
            coins,
        })
    }

    fn predicted_duration(self) -> u64 {
        match self {
            Self::Eat => (MEAL_NOURISHMENT / BERRY_NUTRITION_PER_GRAM * BERRY_EATING_MS_PER_GRAM)
                .ceil() as u64,
            Self::Sleep => SLEEP_DURATION_MS,
            Self::Drink => crate::DRINK_DURATION_MS,
            Self::BuyFood(_)
            | Self::ListExcess
            | Self::List(..)
            | Self::ListAt { .. }
            | Self::Buy(..)
            | Self::BuyAt { .. }
            | Self::Withdraw(..)
            | Self::WithdrawAt { .. } => TRADE_DURATION_MS,
            _ => ACTION_DURATION_MS,
        }
    }
}

const ACTIONS: [CitizenAction; 8] = [
    CitizenAction::Eat,
    CitizenAction::Sleep,
    CitizenAction::Produce(crate::production::Recipe::Forage),
    CitizenAction::BuyFood(Good::Berries),
    CitizenAction::BuyFood(Good::BreadRoll),
    CitizenAction::BuyFood(Good::BerryPie),
    CitizenAction::BuyFood(Good::Fish),
    CitizenAction::BuyFood(Good::Cabbage),
];

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Cooldowns {
    eat: u64,
    buy: u64,
    sell: u64,
}

impl Cooldowns {
    fn remaining(self, action: CitizenAction) -> u64 {
        match action {
            CitizenAction::Eat => self.eat,
            CitizenAction::BuyFood(_) | CitizenAction::Buy(..) | CitizenAction::BuyAt { .. } => {
                self.buy
            }
            CitizenAction::ListExcess | CitizenAction::List(..) => self.sell,
            _ => 0,
        }
    }

    fn after(self, action: CitizenAction, duration: u64) -> Self {
        Self {
            eat: if action == CitizenAction::Eat {
                EAT_PLAN_COOLDOWN_MS
            } else {
                self.eat.saturating_sub(duration)
            },
            buy: if matches!(
                action,
                CitizenAction::BuyFood(_) | CitizenAction::Buy(..) | CitizenAction::BuyAt { .. }
            ) {
                TRADE_PLAN_COOLDOWN_MS
            } else {
                self.buy.saturating_sub(duration)
            },
            sell: if matches!(action, CitizenAction::ListExcess | CitizenAction::List(..)) {
                TRADE_PLAN_COOLDOWN_MS
            } else {
                self.sell.saturating_sub(duration)
            },
        }
    }
}

#[derive(Clone)]
pub(super) struct Prediction {
    pub citizen: Citizen,
    pub actions: Vec<CitizenAction>,
    pub commercial_actions: Vec<usize>,
    pub action_durations_ms: Vec<u64>,
    pub elapsed_ms: u64,
    waiting: Citizen,
    score: WeightedWellbeing,
    goal_score: WeightedWellbeing,
    pub cooldowns: Cooldowns,
    last_gathering_order: Option<CitizenAction>,
    production_quote_ceilings: Option<[f64; Good::COUNT]>,
    production_prices: Option<crate::production::ProductionPrices>,
    qualified_production_route: bool,
    pub blocked_requests: std::rc::Rc<std::cell::RefCell<crate::marketplace::ShoppingList>>,
}

impl Prediction {
    pub fn new(citizen: &Citizen, cooldowns: Cooldowns) -> Self {
        let mut waiting = citizen.clone();
        waiting.active_action = None;
        waiting.active_plan = None;
        Self {
            citizen: citizen.clone(),
            waiting,
            actions: Vec::new(),
            commercial_actions: Vec::new(),
            action_durations_ms: Vec::new(),
            elapsed_ms: 0,
            score: WeightedWellbeing::default(),
            goal_score: WeightedWellbeing::default(),
            cooldowns,
            last_gathering_order: None,
            production_quote_ceilings: None,
            production_prices: None,
            qualified_production_route: false,
            blocked_requests: Default::default(),
        }
    }

    fn perform(
        &self,
        action: CitizenAction,
        prior_actions: usize,
    ) -> Result<Option<Self>, SimulationError> {
        let action = match action {
            CitizenAction::BuyAt { place, list } => CitizenAction::BuyAt {
                place,
                list: list.with_purchase_bounds(
                    self.citizen.market(),
                    self.citizen.market_party(),
                    place,
                    self.citizen.coins(),
                )?,
            },
            action => action,
        };
        if self.cooldowns.remaining(action) > 0
            || super::replan_check(&self.citizen, action, prior_actions + self.actions.len())
        {
            return Ok(None);
        }
        let started = match self.citizen.start_action(action) {
            Ok(started) => started,
            Err(SimulationError::MissingInputs) => return Ok(None),
            Err(error) => return Err(error),
        };
        let Some(active) = started.active_action() else {
            return Ok(None);
        };
        let duration = active.duration_ms();
        let citizen = match started.advance_predicted(duration) {
            Ok(citizen) => citizen,
            Err(SimulationError::MissingInputs) => return Ok(None),
            Err(error) => return Err(error),
        };
        let consumption = matches!(action, CitizenAction::Eat | CitizenAction::Drink);
        let (before, after) = if consumption {
            // Forecast benefit is bounded by dietary wellbeing, so only decision scores can overflow.
            (self.citizen.decision_score()?, citizen.decision_score()?)
        } else {
            (
                self.citizen.planning_wellbeing()?,
                citizen.planning_wellbeing()?,
            )
        };
        let mut next = self.clone();
        if matches!(
            action,
            CitizenAction::BuyFood(_) | CitizenAction::Buy(_) | CitizenAction::BuyAt { .. }
        ) && action.effects().contains(&Effect::Food)
            && citizen.available_food_nutrition() <= self.citizen.available_food_nutrition()
        {
            return Ok(None);
        }
        next.citizen = citizen;
        if consumption {
            let mut current = started;
            let mut remaining = duration;
            let mut from = current.planning_wellbeing()?;
            while remaining > 0 {
                let step = current.active_action().unwrap().next_boundary_ms();
                let advanced = current.advance_predicted(step)?;
                let to = advanced.planning_wellbeing()?;
                next.score_step(from, to, step)?;
                current = advanced;
                from = to;
                remaining -= step;
            }
        } else {
            next.score_step(before, after, duration)?;
        }
        if action.production_recipe() != Some(crate::production::Recipe::Forage)
            && !matches!(action, CitizenAction::Travel(_))
        {
            next.last_gathering_order = None;
        }
        next.actions.push(action);
        next.action_durations_ms.push(duration);
        next.elapsed_ms += duration;
        next.cooldowns = self.cooldowns.after(action, duration);
        Ok(Some(next))
    }

    fn score_step(
        &mut self,
        before: f64,
        after: f64,
        duration: u64,
    ) -> Result<(), SimulationError> {
        let waiting_before = self.waiting.planning_wellbeing()?;
        // Keep the counterfactual free of production-trial reconciliation and market changes.
        self.waiting.advance_needs(duration, None)?;
        self.waiting.market_time_ms = self
            .waiting
            .market_time_ms
            .checked_add(duration)
            .ok_or(SimulationError::TimeOverflow)?;
        let waiting_after = self.waiting.planning_wellbeing()?;
        let before = before - waiting_before;
        let after = after - waiting_after;
        if !before.is_finite() || !after.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        self.score.add(before, after, duration)?;
        self.goal_score.add(before, after, duration)
    }

    fn meets_production_margin(
        &self,
        recipe: crate::production::Recipe,
        site: Option<crate::locations::PlaceId>,
    ) -> bool {
        self.qualified_production_route
            || self.production_prices.as_ref().map_or_else(
                || crate::production::meets_production_margin(recipe, &self.citizen, site),
                |prices| prices.meets_margin(recipe, &self.citizen, site),
            )
    }

    fn perform_commercial(
        &self,
        action: CitizenAction,
        prior_actions: usize,
    ) -> Result<Option<Self>, SimulationError> {
        let recipe = match action {
            CitizenAction::Plant { crop, .. } => Some(crop.recipe()),
            action => action.production_recipe(),
        };
        let mut committed = self.clone();
        if let Some(recipe) = recipe {
            if !committed.meets_production_margin(recipe, action.required_place(&committed.citizen))
            {
                return Ok(None);
            }
        }
        committed.citizen.start_trial_production(action)?;
        let Some(mut next) = committed.perform(action, prior_actions)? else {
            return Ok(None);
        };
        if recipe.is_some() {
            next.commercial_actions.push(self.actions.len());
        }
        Ok(Some(next))
    }

    pub fn average(&self) -> Result<f64, SimulationError> {
        Ok(self.score.mean)
    }

    #[cfg(test)]
    fn goal_average(&self) -> Result<f64, SimulationError> {
        Ok(self.goal_score.mean)
    }
}

#[derive(Clone, Copy, Default)]
struct WeightedWellbeing {
    mean: f64,
    elapsed_ms: u64,
}

impl WeightedWellbeing {
    fn add(&mut self, before: f64, after: f64, duration_ms: u64) -> Result<(), SimulationError> {
        let value = before * 0.5 + after * 0.5;
        let total = self.elapsed_ms + duration_ms;
        let weight = duration_ms as f64 / total as f64;
        // Keep the weighted mean directly to avoid overflowing a wellbeing × duration sum.
        self.mean = if self.mean.is_sign_positive() == value.is_sign_positive() {
            self.mean + (value - self.mean) * weight
        } else {
            self.mean * (self.elapsed_ms as f64 / total as f64) + value * weight
        };
        if !self.mean.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        self.elapsed_ms = total;
        Ok(())
    }
}

fn quantity(citizen: &Citizen, resource: Effect) -> f64 {
    match resource {
        Effect::Food => citizen.available_food_nutrition(),
        Effect::Coins => citizen.coins() as f64,
        _ => unreachable!("only inventory resources are prerequisites"),
    }
}

fn suppliers(
    state: &Prediction,
    requirement: Requirement,
) -> Result<Vec<CitizenAction>, SimulationError> {
    let mut actions: Vec<_> = ACTIONS
        .into_iter()
        .filter(|action| action.effects().contains(&requirement.resource))
        .map(|action| match action {
            CitizenAction::BuyFood(good) if requirement.amount > MEAL_NOURISHMENT => {
                CitizenAction::Buy(crate::marketplace::ShoppingList::single(
                    good,
                    ((requirement.amount - state.citizen.available_food_nutrition()).max(0.0)
                        / good.nutrition_per_unit().unwrap())
                    .ceil() as crate::Quantity,
                ))
            }
            _ => action,
        })
        .collect();
    if requirement.resource != Effect::Food {
        return Ok(actions);
    }
    for first in Good::FOOD {
        let order =
            std::iter::once(first).chain(Good::FOOD.into_iter().filter(|good| *good != first));
        let mut remaining =
            (requirement.amount - state.citizen.available_food_nutrition()).max(0.0);
        let mut items = Vec::new();
        for good in order {
            let nutrition = good.nutrition_per_unit().unwrap();
            let units = state
                .citizen
                .market()
                .available_units(state.citizen.market_party(), good)
                .min((remaining / nutrition).ceil() as crate::Quantity);
            if units > 0 {
                items.push((good, units));
                remaining = (remaining - units as f64 * nutrition).max(0.0);
            }
        }
        if items.len() > 1 {
            let action = CitizenAction::Buy(crate::marketplace::ShoppingList::new(items)?);
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
    }
    for recipe in state.citizen.available_recipes() {
        if recipe
            .outputs()
            .iter()
            .any(|(good, _)| good.nutrition_per_unit().is_some())
        {
            let action = CitizenAction::Produce(recipe);
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
    }
    for good in Good::FOOD {
        for (place, units) in state.citizen.listed_sources(good) {
            actions.push(
                state.citizen.withdraw_action(
                    place,
                    good,
                    units.min(
                        ((requirement.amount - state.citizen.available_food_nutrition()).max(0.0)
                            / good.nutrition_per_unit().unwrap())
                        .ceil() as crate::Quantity,
                    ),
                ),
            );
        }
    }
    Ok(actions)
}

fn satisfy(
    state: &Prediction,
    requirement: Requirement,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let available = quantity(&state.citizen, requirement.resource);
    let satisfied = if requirement.resource == Effect::Coins {
        requirement
            .coins
            .is_some_and(|required| state.citizen.coins() >= required)
    } else if requirement.amount == MEAL_NOURISHMENT {
        state.citizen.has_complete_meal()
    } else {
        available >= requirement.amount
    };
    if satisfied {
        return Ok(vec![state.clone()]);
    }
    if state.elapsed_ms + reserved_ms >= GOAL_HORIZON_MS {
        return Ok(Vec::new());
    }
    let mut variants = Vec::new();
    for action in suppliers(state, requirement)? {
        for next in order_variants(
            state,
            action,
            requirement.amount,
            reserved_ms,
            prior_actions,
        )? {
            if quantity(&next.citizen, requirement.resource)
                > quantity(&state.citizen, requirement.resource)
                && next.elapsed_ms + reserved_ms <= GOAL_HORIZON_MS
            {
                variants.extend(satisfy(&next, requirement, reserved_ms, prior_actions)?);
            }
        }
    }
    Ok(variants)
}

fn reserve_units(citizen: &Citizen, good: Good) -> u64 {
    citizen
        .carried_available_units(good)
        .saturating_add(citizen.stored_available_units(citizen.home(), good))
}

fn reserve_food_nutrition(citizen: &Citizen) -> f64 {
    Good::FOOD
        .into_iter()
        .map(|good| reserve_units(citizen, good) as f64 * good.nutrition_per_unit().unwrap())
        .sum::<f64>()
        + reserve_units(citizen, Good::Beer) as f64 * 0.05
}

fn reserve_drink_units(citizen: &Citizen) -> u64 {
    reserve_units(citizen, Good::Water).saturating_add(reserve_units(citizen, Good::Beer))
}

fn deliver_reserves(
    initial: &Prediction,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    let list = crate::marketplace::ShoppingList::new(
        Good::FOOD
            .into_iter()
            .chain([Good::Water, Good::Beer, Good::FlaxGarment])
            .map(|good| (good, initial.citizen.carried_available_units(good))),
    )?;
    if list.items().next().is_none() {
        return Ok(Some(initial.clone()));
    }
    let mut ready = initial.clone();
    let home = ready.citizen.home();
    if ready.citizen.position() != ready.citizen.map().position(home) {
        let Some(next) = ready.perform(CitizenAction::Travel(home), prior_actions)? else {
            return Ok(None);
        };
        ready = next;
    }
    ready.perform(
        CitizenAction::DepositStored { place: home, list },
        prior_actions,
    )
}

fn pantry_categories(citizen: &Citizen) -> [f64; 4] {
    let mut points = [0.0; 4];
    for good in Good::FOOD.into_iter().chain([Good::Beer]) {
        for (total, per_unit) in points.iter_mut().zip(good.diet_per_unit()) {
            *total += reserve_units(citizen, good) as f64 * per_unit;
        }
    }
    points
}

fn provisioning_needed(citizen: &Citizen) -> bool {
    citizen.home_food_nutrition() < 100.0 * citizen.supplied_members() as f64
        || citizen.home_drink_units() < 400 * citizen.supplied_members()
        || reserve_units(citizen, Good::FlaxGarment) < citizen.spare_clothing_target()
}

fn basket_cost(citizen: &Citizen, units: &[crate::Quantity; Good::COUNT]) -> Option<crate::Coins> {
    let list = crate::marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
        let owned = withdrawable_units(citizen, good).saturating_add(
            citizen
                .stored_sources(good)
                .into_iter()
                .filter(|(place, _)| *place != citizen.home())
                .map(|(place, stock)| stock.min(citizen.collectable_reserve_units(place, good)))
                .fold(0, u64::saturating_add),
        );
        (good, units[good as usize].saturating_sub(owned))
    }))
    .ok()?;
    citizen.market().estimated_purchase_cost_from(
        citizen.market_party(),
        &citizen.map(),
        citizen.position(),
        list,
    )
}

fn provisioning_basket(
    citizen: &Citizen,
    omitted: usize,
    basic: bool,
) -> Result<crate::marketplace::ShoppingList, SimulationError> {
    let members = citizen.supplied_members() as f64;
    let mut units = [0; Good::COUNT];
    let mut nutrition = reserve_food_nutrition(citizen);
    let mut drink = reserve_drink_units(citizen) as f64;
    let mut categories = pantry_categories(citizen);
    let food_target = if basic { 100.0 } else { 300.0 } * members;
    let drink_target = if basic { 400.0 } else { 1200.0 } * members;
    for phase in 0..3 {
        if basic && phase == 2 {
            break;
        }
        loop {
            let missing_food = (food_target - nutrition).max(0.0);
            let missing_drink = (drink_target - drink).max(0.0);
            let missing = categories.map(|points| (75.0 * members - points).max(0.0));
            if phase == 0 && missing_food == 0.0
                || phase == 1 && missing_drink == 0.0
                || phase == 2
                    && missing
                        .iter()
                        .enumerate()
                        .all(|(index, points)| index == omitted || *points == 0.0)
            {
                break;
            }
            let old_cost = basket_cost(citizen, &units).unwrap_or(0);
            let mut best: Option<(bool, f64, Good, crate::Quantity)> = None;
            for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
                let diet = good.diet_per_unit();
                let food = good.nutrition_per_unit().unwrap_or(if good == Good::Beer {
                    0.05
                } else {
                    0.0
                });
                let thirst: f64 = if matches!(good, Good::Water | Good::Beer) {
                    1.0
                } else {
                    0.0
                };
                let contribution = match phase {
                    0 => food.min(missing_food),
                    1 => thirst.min(missing_drink),
                    _ => diet
                        .into_iter()
                        .enumerate()
                        .filter(|(index, _)| *index != omitted)
                        .map(|(index, points)| points.min(missing[index]))
                        .sum(),
                };
                if contribution <= 0.0 {
                    continue;
                }
                let count = match phase {
                    0 => (missing_food / food).ceil(),
                    1 => (missing_drink / thirst).ceil(),
                    _ => diet
                        .into_iter()
                        .enumerate()
                        .filter(|(index, points)| *index != omitted && *points > 0.0)
                        .map(|(index, points)| (missing[index] / points).ceil())
                        .fold(0.0, f64::max),
                } as crate::Quantity;
                let mut proposed = units;
                proposed[good as usize] = proposed[good as usize].saturating_add(count);
                let Some(cost) = basket_cost(citizen, &proposed) else {
                    continue;
                };
                let gain = match phase {
                    0 => missing_food.min(count as f64 * food),
                    1 => missing_drink.min(count as f64 * thirst),
                    _ => diet
                        .into_iter()
                        .enumerate()
                        .filter(|(index, _)| *index != omitted)
                        .map(|(index, points)| missing[index].min(points * count as f64))
                        .sum(),
                };
                let available = citizen
                    .market()
                    .available_units(citizen.market_party(), good)
                    .saturating_add(withdrawable_units(citizen, good))
                    .saturating_add(
                        citizen
                            .stored_sources(good)
                            .into_iter()
                            .filter(|(place, _)| *place != citizen.home())
                            .map(|(place, units)| {
                                units.min(citizen.collectable_reserve_units(place, good))
                            })
                            .fold(0, u64::saturating_add),
                    );
                let unavailable = available <= units[good as usize];
                let ratio = (cost - old_cost) as f64 / gain;
                if best.is_none_or(|(old_unavailable, old, _, _)| {
                    (unavailable, ratio) < (old_unavailable, old)
                }) {
                    best = Some((unavailable, ratio, good, count));
                }
            }
            let Some((_, _, good, count)) = best else {
                break;
            };
            units[good as usize] = units[good as usize].saturating_add(count);
            nutrition += count as f64
                * good
                    .nutrition_per_unit()
                    .unwrap_or(if good == Good::Beer { 0.05 } else { 0.0 });
            if matches!(good, Good::Water | Good::Beer) {
                drink += count as f64;
            }
            for (points, per_unit) in categories.iter_mut().zip(good.diet_per_unit()) {
                *points += count as f64 * per_unit;
            }
        }
    }
    crate::marketplace::ShoppingList::new(
        Good::ALL
            .into_iter()
            .map(|good| (good, units[good as usize])),
    )
}

fn acquire_provisioning_list(
    initial: &Prediction,
    list: crate::marketplace::ShoppingList,
    prior_actions: usize,
) -> Result<Prediction, SimulationError> {
    let mut state = initial.clone();
    for (good, wanted) in list.items() {
        let mut remaining = wanted;
        for (place, stock) in state.citizen.stored_sources(good) {
            if place == state.citizen.home() {
                continue;
            }
            let units = remaining
                .min(stock)
                .min(state.citizen.collectable_reserve_units(place, good));
            if units == 0 {
                continue;
            }
            let action = CitizenAction::CollectStored {
                place,
                list: crate::marketplace::ShoppingList::single(good, units),
            };
            if let Some(next) = order_variants(&state, action, 0.0, 0, prior_actions)?.pop() {
                state = next;
                remaining -= units;
            }
        }
        let target = state
            .citizen
            .carried_available_units(good)
            .saturating_add(remaining);
        if let Some(next) = collect_listed_inputs(&state, good, target, 0, prior_actions)? {
            state = next;
        }
    }
    let missing = crate::marketplace::ShoppingList::new(list.items().map(|(good, units)| {
        (
            good,
            units.saturating_sub(
                reserve_units(&state.citizen, good)
                    .saturating_sub(reserve_units(&initial.citizen, good)),
            ),
        )
    }))?;
    if missing.items().next().is_none() {
        return Ok(state);
    }
    record_blocked_purchase(&state, missing)?;
    let alternatives = shopping_trip_from_mode(&state, missing, 0, prior_actions, true)?;
    Ok(alternatives
        .into_iter()
        .max_by(|a, b| {
            let gain = |candidate: &Prediction| {
                list.items()
                    .map(|(good, wanted)| {
                        reserve_units(&candidate.citizen, good)
                            .saturating_sub(reserve_units(&state.citizen, good))
                            .min(wanted) as f64
                            / wanted as f64
                    })
                    .sum::<f64>()
            };
            gain(a)
                .total_cmp(&gain(b))
                .then_with(|| b.elapsed_ms.cmp(&a.elapsed_ms))
        })
        .unwrap_or(state))
}

fn provision_household(
    initial: &Prediction,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if !provisioning_needed(&initial.citizen) {
        return Ok(Vec::new());
    }
    let mut state = initial.clone();
    state.blocked_requests = Default::default();
    let basic = provisioning_basket(&state.citizen, 3, true)?;
    state = acquire_provisioning_list(&state, basic, prior_actions)?;
    let basic_drink = 400 * state.citizen.supplied_members();
    if reserve_drink_units(&state.citizen) < basic_drink {
        state = gather_provisions(
            &state,
            crate::production::Recipe::FetchWater,
            Good::Water,
            basic_drink - reserve_drink_units(&state.citizen),
            prior_actions,
        )?;
    }
    let basic_food = 100.0 * state.citizen.supplied_members() as f64;
    if reserve_food_nutrition(&state.citizen)
        < crate::MEAL_NOURISHMENT * state.citizen.supplied_members() as f64
        && reserve_food_nutrition(&state.citizen) < basic_food
    {
        let missing = basic_food - reserve_food_nutrition(&state.citizen);
        state = gather_provisions(
            &state,
            crate::production::Recipe::Forage,
            Good::Berries,
            (missing / crate::BERRY_NUTRITION_PER_GRAM).ceil() as crate::Quantity,
            prior_actions,
        )?;
    }
    let mut candidates = Vec::new();
    let mut pending_baskets = Vec::new();
    for omitted in 0..4 {
        let list = provisioning_basket(&state.citizen, omitted, false)?;
        let mut ready = state.clone();
        ready.blocked_requests =
            std::rc::Rc::new(std::cell::RefCell::new(*state.blocked_requests.borrow()));
        if ready.actions.len() > initial.actions.len() {
            ready.cooldowns.buy = 0;
        }
        let acquired = acquire_provisioning_list(&ready, list, prior_actions)?;
        let units = Good::ALL.map(|good| list.units(good));
        pending_baskets.push((
            basket_cost(&state.citizen, &units).unwrap_or(crate::Coins::MAX),
            omitted,
            *acquired.blocked_requests.borrow(),
        ));
        if let Some(delivered) = deliver_reserves(&acquired, prior_actions)? {
            let categories = pantry_categories(&delivered.citizen);
            let members = delivered.citizen.supplied_members() as f64;
            let covered = categories
                .into_iter()
                .enumerate()
                .filter(|(index, points)| *index != omitted && *points >= 25.0 * members)
                .count();
            let diversity: f64 = categories
                .into_iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, points)| points.min(75.0 * members))
                .sum();
            candidates.push((covered, diversity, omitted, acquired, delivered));
        }
    }
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.total_cmp(&a.1))
            .then_with(|| b.4.citizen.coins().cmp(&a.4.citizen.coins()))
            .then_with(|| a.4.elapsed_ms.cmp(&b.4.elapsed_ms))
            .then_with(|| a.2.cmp(&b.2))
    });
    let Some((_, _, _, acquired, food_delivery)) = candidates.into_iter().next() else {
        pending_baskets.sort_by_key(|(cost, omitted, _)| (*cost, *omitted));
        let pending = pending_baskets
            .first()
            .map_or(*state.blocked_requests.borrow(), |(_, _, request)| *request);
        record_blocked_purchase(initial, pending)?;
        return Ok(Vec::new());
    };
    let mut delivered = food_delivery;
    let missing_spares = acquired
        .citizen
        .spare_clothing_target()
        .saturating_sub(reserve_units(&acquired.citizen, Good::FlaxGarment));
    if missing_spares > 0
        && reserve_food_nutrition(&acquired.citizen) >= basic_food
        && reserve_drink_units(&acquired.citizen) >= basic_drink
    {
        let mut ready = acquired.clone();
        ready.cooldowns.buy = 0;
        let with_spare = acquire_provisioning_list(
            &ready,
            crate::marketplace::ShoppingList::single(Good::FlaxGarment, missing_spares),
            prior_actions,
        )?;
        if let Some(spare_delivery) = deliver_reserves(&with_spare, prior_actions)? {
            delivered = spare_delivery;
        }
    }
    let chosen_requests = *delivered.blocked_requests.borrow();
    record_blocked_purchase(initial, chosen_requests)?;
    Ok(if delivered.actions.len() > initial.actions.len() {
        vec![delivered]
    } else {
        Vec::new()
    })
}

fn gather_provisions(
    initial: &Prediction,
    recipe: crate::production::Recipe,
    good: Good,
    wanted: crate::Quantity,
    prior_actions: usize,
) -> Result<Prediction, SimulationError> {
    let target = reserve_units(&initial.citizen, good).saturating_add(wanted);
    let mut state = initial.clone();
    while reserve_units(&state.citizen, good) < target && state.elapsed_ms < GOAL_HORIZON_MS {
        let prefixes: Vec<_> = order_variants(
            &state,
            CitizenAction::Produce(recipe),
            0.0,
            0,
            prior_actions,
        )?
        .into_iter()
        .filter(|candidate| {
            candidate
                .elapsed_ms
                .saturating_sub(candidate.action_durations_ms.last().copied().unwrap_or(0))
                < GOAL_HORIZON_MS
        })
        .collect();
        let next = prefixes
            .iter()
            .find(|candidate| reserve_units(&candidate.citizen, good) >= target)
            .cloned()
            .or_else(|| {
                prefixes.into_iter().rfind(|candidate| {
                    reserve_units(&candidate.citizen, good) > reserve_units(&state.citizen, good)
                })
            });
        let Some(next) = next else {
            break;
        };
        state = next;
    }
    Ok(state)
}

fn gathering_segments(horizon_ms: u64, duration_ms: u64) -> u64 {
    horizon_ms / duration_ms
}

fn listing_trip(
    initial: &Prediction,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if initial.cooldowns.sell > 0 {
        return Ok(Vec::new());
    }
    let excess = initial.citizen.excess_goods()?;
    let mut state = initial.clone();
    let mut completed = None;
    let baskets = match initial.citizen.listing_baskets(excess) {
        Ok(baskets) => baskets,
        Err(SimulationError::MissingInputs) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    for (place, list) in baskets {
        for (good, units) in list.items() {
            let Some(collected) = collect_stored_inputs(&state, good, units, 0, prior_actions)?
            else {
                return Ok(completed.into_iter().collect());
            };
            state = collected;
        }
        let action = CitizenAction::ListAt { place, list };
        let Some(next) = order_variants(&state, action, 0.0, 0, prior_actions)?.pop() else {
            return Ok(completed.into_iter().collect());
        };
        completed = Some(next.clone());
        state = next;
    }
    Ok(completed.into_iter().collect())
}

fn order_variants(
    state: &Prediction,
    action: CitizenAction,
    amount: f64,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let list = match action {
        CitizenAction::Buy(list) => Some(list),
        CitizenAction::BuyFood(good) => Some(crate::marketplace::ShoppingList::single(
            good,
            ((amount.max(MEAL_NOURISHMENT) - state.citizen.available_food_nutrition()).max(0.0)
                / good.nutrition_per_unit().unwrap())
            .ceil() as crate::Quantity,
        )),
        _ => None,
    };
    if let Some(list) = list {
        if (amount > 0.0 || matches!(action, CitizenAction::BuyFood(_)))
            && list.items().count() == 1
            && let Some((good, _)) = list.items().next()
            && good.nutrition_per_unit().is_some()
        {
            let demand = food_purchase_demand(state, amount.max(MEAL_NOURISHMENT), good);
            return shopping_trip_with_demand(state, list, demand, reserved_ms, prior_actions);
        }
        return shopping_trip(state, list, reserved_ms, prior_actions);
    }
    if let Some(recipe) = action.production_recipe()
        && recipe != crate::production::Recipe::Forage
        && amount > state.citizen.available_food_nutrition()
    {
        return food_production_variants(state, recipe, amount, reserved_ms, prior_actions);
    }
    let gathering = action.production_recipe() == Some(crate::production::Recipe::Forage);
    if gathering
        && state
            .last_gathering_order
            .is_some_and(|previous| previous.production_recipe() == action.production_recipe())
    {
        return Ok(Vec::new());
    }
    let mut variants = Vec::new();
    for ready in prepare(state, action, amount, reserved_ms, prior_actions)? {
        // Nested resource acquisition can change the preceding order.
        if gathering
            && ready
                .last_gathering_order
                .is_some_and(|previous| previous.production_recipe() == action.production_recipe())
        {
            continue;
        }
        let limit = if gathering {
            gathering_segments(GOAL_HORIZON_MS, action.predicted_duration())
        } else {
            1
        };
        let mut next = ready;
        for _ in 0..limit {
            let Some(mut performed) = next.perform(action, prior_actions)? else {
                break;
            };
            if gathering {
                performed.last_gathering_order = Some(action);
            }
            variants.push(performed.clone());
            next = performed;
        }
    }
    Ok(variants)
}

fn withdrawable_units(citizen: &Citizen, good: Good) -> crate::Quantity {
    citizen.withdrawable_units(good)
}

fn food_purchase_demand(
    state: &Prediction,
    target: f64,
    good: Good,
) -> crate::marketplace::ShoppingList {
    let owned: f64 = Good::FOOD
        .into_iter()
        .map(|food| {
            withdrawable_units(&state.citizen, food) as f64 * food.nutrition_per_unit().unwrap()
        })
        .sum();
    let missing = (target - state.citizen.available_food_nutrition() - owned).max(0.0);
    crate::marketplace::ShoppingList::single(
        good,
        (missing / good.nutrition_per_unit().unwrap()).ceil() as crate::Quantity,
    )
}

fn record_blocked_purchase(
    state: &Prediction,
    list: crate::marketplace::ShoppingList,
) -> Result<(), SimulationError> {
    if state.cooldowns.buy > 0 {
        return Ok(());
    }
    let citizen = &state.citizen;
    if citizen.coins() <= 0 {
        return Ok(());
    }
    let filled = citizen.market().affordable_purchase_list_at(
        citizen.market_party(),
        None,
        list,
        citizen.coins(),
    )?;
    let mut blocked = state.blocked_requests.borrow_mut();
    *blocked = crate::marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
        let requested = if filled.units(good) < list.units(good) {
            list.units(good)
        } else {
            0
        };
        (good, blocked.units(good).max(requested))
    }))?;
    Ok(())
}

fn shopping_trip(
    initial: &Prediction,
    list: crate::marketplace::ShoppingList,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if initial.cooldowns.buy > 0 {
        return Ok(Vec::new());
    }
    let demand = crate::marketplace::ShoppingList::new(list.items().map(|(good, units)| {
        (
            good,
            units.saturating_sub(withdrawable_units(&initial.citizen, good)),
        )
    }))?;
    shopping_trip_with_demand(initial, list, demand, reserved_ms, prior_actions)
}

fn shopping_trip_with_demand(
    initial: &Prediction,
    list: crate::marketplace::ShoppingList,
    demand: crate::marketplace::ShoppingList,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if initial.cooldowns.buy > 0 || initial.elapsed_ms + reserved_ms >= GOAL_HORIZON_MS {
        return Ok(Vec::new());
    }
    record_blocked_purchase(initial, demand)?;
    shopping_trip_from_mode(initial, list, reserved_ms, prior_actions, false)
}

fn shopping_trip_from_mode(
    initial: &Prediction,
    list: crate::marketplace::ShoppingList,
    reserved_ms: u64,
    prior_actions: usize,
    partial: bool,
) -> Result<Vec<Prediction>, SimulationError> {
    if initial.cooldowns.buy > 0 || initial.elapsed_ms + reserved_ms >= GOAL_HORIZON_MS {
        return Ok(Vec::new());
    }
    let list = initial.production_quote_ceilings.map_or(list, |ceilings| {
        list.with_quote_ceilings(ceilings, initial.citizen.coins())
    });
    let mut state = initial.clone();
    let mut remaining = list;
    for (good, wanted) in list.items() {
        let before = state.citizen.carried_available_units(good);
        let Some(retrieved) = collect_listed_inputs(
            &state,
            good,
            before.saturating_add(wanted),
            reserved_ms,
            prior_actions,
        )?
        else {
            return Ok(Vec::new());
        };
        let collected = retrieved
            .citizen
            .carried_available_units(good)
            .saturating_sub(before);
        state = retrieved;
        remaining = remaining.with_units_from(crate::marketplace::ShoppingList::new(
            remaining.items().map(|(item, units)| {
                (
                    item,
                    if item == good {
                        units.saturating_sub(collected)
                    } else {
                        units
                    },
                )
            }),
        )?);
    }
    if remaining.items().next().is_none() {
        return Ok(vec![state]);
    }
    while remaining.items().next().is_some() {
        let citizen = &state.citizen;
        let Some((place, basket)) = citizen.market().next_purchase_stop(
            citizen.market_party(),
            &citizen.map(),
            citizen.position(),
            remaining,
            Some(citizen.coins()),
        ) else {
            return Ok(if partial && state.actions.len() > initial.actions.len() {
                vec![state]
            } else {
                Vec::new()
            });
        };
        if citizen.position() != citizen.map().position(place) {
            let Some(travelled) = state.perform(CitizenAction::Travel(place), prior_actions)?
            else {
                return Ok(if partial && state.actions.len() > initial.actions.len() {
                    vec![state]
                } else {
                    Vec::new()
                });
            };
            state = travelled;
        }
        if state.elapsed_ms + reserved_ms > GOAL_HORIZON_MS {
            return Ok(if partial && state.actions.len() > initial.actions.len() {
                vec![state]
            } else {
                Vec::new()
            });
        }
        let inventory = state.citizen.clone();
        // Only this trip's next stop may bypass the cooldown from its previous purchase.
        state.cooldowns.buy = 0;
        let Some(bought) = state.perform(
            CitizenAction::BuyAt {
                place,
                list: basket,
            },
            prior_actions,
        )?
        else {
            return Ok(if partial && state.actions.len() > initial.actions.len() {
                vec![state]
            } else {
                Vec::new()
            });
        };
        remaining = remaining.with_units_from(crate::marketplace::ShoppingList::new(
            remaining.items().map(|(good, units)| {
                (
                    good,
                    units.saturating_sub(
                        bought
                            .citizen
                            .available_units(good)
                            .saturating_sub(inventory.available_units(good)),
                    ),
                )
            }),
        )?);
        if !basket
            .items()
            .any(|(good, _)| bought.citizen.available_units(good) > inventory.available_units(good))
        {
            return Ok(if partial && state.actions.len() > initial.actions.len() {
                vec![state]
            } else {
                Vec::new()
            });
        }
        state = bought;
    }
    Ok(vec![state])
}

fn prepare(
    state: &Prediction,
    action: CitizenAction,
    amount: f64,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let prepared = if let Some(recipe) = action.production_recipe() {
        prepare_inputs(
            state,
            &recipe_requirements(&state.citizen, &[recipe])?,
            reserved_ms,
            prior_actions,
        )?
    } else {
        match action.input_for(&state.citizen, amount) {
            Some(requirement) => satisfy(state, requirement, reserved_ms, prior_actions)?,
            None => vec![state.clone()],
        }
    };
    let mut variants = Vec::new();
    for mut ready in prepared {
        if let Some(location) = action.required_place(&ready.citizen)
            && ready.citizen.position() != ready.citizen.map().position(location)
        {
            let Some(travelled) = ready.perform(CitizenAction::Travel(location), prior_actions)?
            else {
                continue;
            };
            ready = travelled;
        }
        // Preparation includes travel; the final goal activity is not reserved against this budget.
        if ready.elapsed_ms + reserved_ms <= GOAL_HORIZON_MS {
            variants.push(ready);
        }
    }
    Ok(variants)
}

fn recipe_requirements(
    citizen: &Citizen,
    recipes: &[crate::production::Recipe],
) -> Result<crate::marketplace::ShoppingList, SimulationError> {
    crate::production::recipe_requirements(citizen, recipes)
}

fn production_order(
    initial: &Prediction,
    recipes: &[crate::production::Recipe],
    food_target: Option<f64>,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    production_order_with_evidence(
        initial,
        recipes,
        food_target,
        reserved_ms,
        prior_actions,
        &mut false,
    )
}

fn production_order_with_evidence(
    initial: &Prediction,
    recipes: &[crate::production::Recipe],
    food_target: Option<f64>,
    reserved_ms: u64,
    prior_actions: usize,
    preparation_limit: &mut bool,
) -> Result<Vec<Prediction>, SimulationError> {
    production_order_bounded(
        initial,
        recipes,
        food_target,
        reserved_ms,
        prior_actions,
        preparation_limit,
        None,
    )
}

fn production_order_bounded(
    initial: &Prediction,
    recipes: &[crate::production::Recipe],
    food_target: Option<f64>,
    reserved_ms: u64,
    prior_actions: usize,
    preparation_limit: &mut bool,
    site: Option<crate::locations::PlaceId>,
) -> Result<Vec<Prediction>, SimulationError> {
    let requirements = crate::production::recipe_requirements_at(&initial.citizen, site, recipes)?;
    let mut variants = Vec::new();
    let destination = site.or_else(|| {
        recipes
            .first()
            .and_then(|recipe| initial.citizen.production_place(*recipe).ok())
    });
    let needs_transport = destination
        .is_some_and(|place| initial.citizen.position() != initial.citizen.map().position(place));
    let prepared = if !needs_transport
        && requirements.items().all(|(good, units)| {
            initial
                .citizen
                .carried_available_units(good)
                .saturating_add(destination.map_or(0, |place| {
                    initial.citizen.stored_available_units(place, good)
                }))
                >= units
        }) {
        vec![initial.clone()]
    } else {
        prepare_inputs(initial, &requirements, reserved_ms, prior_actions)?
    };
    for mut state in prepared {
        let mut complete = requirements
            .items()
            .all(|(good, units)| state.citizen.available_units(good) >= units);
        for (index, &recipe) in recipes.iter().enumerate() {
            if !complete
                || index > 0
                    && site.is_some()
                    && !state.qualified_production_route
                    && state.elapsed_ms >= PRODUCTION_DURATION_MS
                || index > 0
                    && food_target
                        .is_some_and(|target| state.citizen.available_food_nutrition() >= target)
            {
                complete = false;
                break;
            }
            if food_target.is_none() && !state.meets_production_margin(recipe, site) {
                complete = false;
                break;
            }
            let action = site.map_or(CitizenAction::Produce(recipe), |place| {
                CitizenAction::ProduceAt { place, recipe }
            });
            if recipe == crate::production::Recipe::Forage
                && !state.qualified_production_route
                && (index == 0 || recipes[index - 1] != recipe)
                && state.last_gathering_order.is_some_and(|previous| {
                    previous.production_recipe() == action.production_recipe()
                })
            {
                complete = false;
                break;
            }
            if let Some(location) = action.required_place(&state.citizen)
                && state.citizen.position() != state.citizen.map().position(location)
            {
                let Some(travelled) =
                    state.perform(CitizenAction::Travel(location), prior_actions)?
                else {
                    complete = false;
                    break;
                };
                state = travelled;
            }
            if !state.qualified_production_route
                && (state.elapsed_ms >= GOAL_HORIZON_MS
                    || state.elapsed_ms + reserved_ms > GOAL_HORIZON_MS)
            {
                *preparation_limit = true;
                complete = false;
                break;
            }
            if recipe == crate::production::Recipe::BrewBeer
                && state.citizen.action_duration_ms(action).is_err()
            {
                complete = false;
                break;
            }
            let Some(next) = (if food_target.is_none() {
                state.perform_commercial(action, prior_actions)?
            } else {
                state.perform(action, prior_actions)?
            }) else {
                complete = false;
                break;
            };
            state = next;
            if recipe == crate::production::Recipe::Forage {
                state.last_gathering_order = Some(action);
            }
        }
        if complete {
            variants.push(state);
        }
    }
    Ok(variants)
}

fn food_production_variants(
    state: &Prediction,
    recipe: crate::production::Recipe,
    target: f64,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let nutrition = |items: &[(Good, crate::Quantity)]| -> f64 {
        items
            .iter()
            .map(|(good, units)| good.nutrition_per_unit().unwrap_or(0.0) * *units as f64)
            .sum()
    };
    let gain = nutrition(recipe.outputs()) - nutrition(recipe.inputs());
    if gain <= 0.0 {
        return Ok(Vec::new());
    }
    let count = ((target - state.citizen.available_food_nutrition()) / gain).ceil() as usize;
    let mut recipes = Vec::new();
    let mut variants = Vec::new();
    for _ in 0..count {
        recipes.push(recipe);
        let next = production_order(state, &recipes, Some(target), reserved_ms, prior_actions)?;
        if next.is_empty() {
            break;
        }
        let can_extend = next.iter().any(|candidate| {
            candidate.elapsed_ms < GOAL_HORIZON_MS
                && candidate.citizen.available_food_nutrition() < target
        });
        variants.extend(next);
        if !can_extend {
            break;
        }
    }
    Ok(variants)
}

fn collect_stored_inputs(
    initial: &Prediction,
    good: Good,
    units: crate::Quantity,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    let mut state = initial.clone();
    for (place, available) in state.citizen.stored_sources(good) {
        let collected = units
            .saturating_sub(state.citizen.carried_available_units(good))
            .min(available);
        if collected == 0 {
            break;
        }
        let action = CitizenAction::CollectStored {
            place,
            list: crate::marketplace::ShoppingList::single(good, collected),
        };
        let Some(next) = order_variants(&state, action, 0.0, reserved_ms, prior_actions)?.pop()
        else {
            return Ok(None);
        };
        state = next;
    }
    Ok(Some(state))
}

fn collect_listed_inputs(
    initial: &Prediction,
    good: Good,
    units: crate::Quantity,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    let mut state = initial.clone();
    for (place, available) in state.citizen.listed_sources(good) {
        let collected = units
            .saturating_sub(state.citizen.carried_available_units(good))
            .min(available);
        if collected == 0 {
            break;
        }
        let action = state.citizen.withdraw_action(place, good, collected);
        let Some(next) = order_variants(&state, action, 0.0, reserved_ms, prior_actions)?.pop()
        else {
            return Ok(None);
        };
        state = next;
    }
    Ok(Some(state))
}

fn prepare_inputs(
    state: &Prediction,
    requirements: &crate::marketplace::ShoppingList,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let mut states = vec![state.clone()];
    for (good, units) in requirements.items() {
        let mut next_states = Vec::new();
        for current in states {
            if current.elapsed_ms + reserved_ms > GOAL_HORIZON_MS {
                continue;
            }
            let Some(mut current) =
                collect_stored_inputs(&current, good, units, reserved_ms, prior_actions)?
            else {
                continue;
            };
            let missing = units.saturating_sub(current.citizen.carried_available_units(good));
            if missing == 0 {
                next_states.push(current);
                continue;
            }
            let Some(retrieved) =
                collect_listed_inputs(&current, good, units, reserved_ms, prior_actions)?
            else {
                continue;
            };
            current = retrieved;
            next_states.push(current.clone());
            if current.citizen.carried_available_units(good) < units
                && !current.qualified_production_route
            {
                let suppliers: Vec<_> = current
                    .citizen
                    .available_recipes()
                    .filter(|r| r.outputs().iter().any(|(output, _)| *output == good))
                    .map(CitizenAction::Produce)
                    .collect();
                for supplier in suppliers {
                    next_states.extend(gather_input(
                        &current,
                        supplier,
                        good,
                        units,
                        reserved_ms,
                        prior_actions,
                    )?);
                }
            }
        }
        states = next_states;
    }
    let mut ready = Vec::new();
    for state in states {
        let list =
            crate::marketplace::ShoppingList::new(requirements.items().map(|(good, units)| {
                (
                    good,
                    units.saturating_sub(state.citizen.carried_available_units(good)),
                )
            }))?;
        if list.items().next().is_none() {
            ready.push(state);
            continue;
        }
        for bought in order_variants(
            &state,
            CitizenAction::Buy(list),
            0.0,
            reserved_ms,
            prior_actions,
        )? {
            if requirements
                .items()
                .all(|(good, units)| bought.citizen.carried_available_units(good) >= units)
            {
                ready.push(bought);
            }
        }
    }
    Ok(ready)
}

fn gather_input(
    state: &Prediction,
    action: CitizenAction,
    good: Good,
    units: crate::Quantity,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if state.elapsed_ms + reserved_ms >= GOAL_HORIZON_MS {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for produced in order_variants(state, action, 0.0, reserved_ms, prior_actions)? {
        let Some(next) = collect_stored_inputs(&produced, good, units, reserved_ms, prior_actions)?
        else {
            continue;
        };
        if next.elapsed_ms + reserved_ms > GOAL_HORIZON_MS
            || next.citizen.carried_available_units(good)
                <= state.citizen.carried_available_units(good)
        {
            continue;
        }
        if next.citizen.carried_available_units(good) >= units {
            result.push(next);
        } else {
            result.extend(gather_input(
                &next,
                action,
                good,
                units,
                reserved_ms,
                prior_actions,
            )?);
        }
    }
    Ok(result)
}

#[cfg(test)]
pub(super) fn best_variant(
    citizen: &Citizen,
    goal: Effect,
    cooldowns: Cooldowns,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    Ok(local_best(variants_from(
        Prediction::new(citizen, cooldowns),
        goal,
        prior_actions,
    )?))
}

#[cfg(test)]
pub(super) fn best_variant_after(
    prefix: &Prediction,
    goal: Effect,
) -> Result<Option<Prediction>, SimulationError> {
    Ok(local_best(variants_after(prefix, goal)?))
}

#[cfg(test)]
fn local_best(variants: Vec<Prediction>) -> Option<Prediction> {
    let mut best: Option<Prediction> = None;
    for candidate in variants {
        if best
            .as_ref()
            .is_none_or(|old| candidate.goal_score.mean > old.goal_score.mean)
        {
            best = Some(candidate);
        }
    }
    best
}

pub(super) fn variants_after(
    prefix: &Prediction,
    goal: Effect,
) -> Result<Vec<Prediction>, SimulationError> {
    let mut initial = Prediction::new(&prefix.citizen, prefix.cooldowns);
    initial.blocked_requests = prefix.blocked_requests.clone();
    initial.waiting = prefix.waiting.clone();
    initial.score = prefix.score;
    initial.last_gathering_order = prefix.last_gathering_order;
    variants_from(initial, goal, prefix.actions.len())
}

pub(super) const PRODUCTION_DURATION_MS: u64 = crate::production::PRODUCTION_BLOCK_MS;

#[cfg(test)]
fn production_restock(
    initial: &Prediction,
    prior_actions: usize,
) -> Result<Prediction, SimulationError> {
    production_restock_at(initial, prior_actions, None)
}

fn deliver_workplace_inputs(
    initial: Prediction,
    site: Option<crate::locations::PlaceId>,
    requirements: crate::marketplace::ShoppingList,
    prior_actions: usize,
) -> Result<Prediction, SimulationError> {
    let Some(place) = site else {
        return Ok(initial);
    };
    let list = crate::marketplace::ShoppingList::new(requirements.items().map(|(good, units)| {
        (
            good,
            units
                .saturating_sub(initial.citizen.stored_available_units(place, good))
                .min(initial.citizen.carried_available_units(good)),
        )
    }))?;
    if list.items().next().is_none() {
        return Ok(initial);
    }
    let mut ready = initial.clone();
    if ready.citizen.position() != ready.citizen.map().position(place) {
        let Some(travelled) = ready.perform(CitizenAction::Travel(place), prior_actions)? else {
            return Ok(initial);
        };
        ready = travelled;
    }
    Ok(ready
        .perform(CitizenAction::DepositStored { place, list }, prior_actions)?
        .unwrap_or(initial))
}

fn production_restock_at(
    initial: &Prediction,
    prior_actions: usize,
    site: Option<crate::locations::PlaceId>,
) -> Result<Prediction, SimulationError> {
    if initial.cooldowns.buy > 0 {
        return Ok(initial.clone());
    }
    let target = if let Some(place) = site {
        let calculate = |horizon| -> Result<crate::marketplace::ShoppingList, SimulationError> {
            let recipes =
                crate::production::projected_recipes_at(&initial.citizen, Some(place), horizon)?;
            crate::production::recipe_requirements_at(&initial.citizen, Some(place), &recipes)
        };
        let fresh_threshold = calculate(PRODUCTION_DURATION_MS)?;
        let fresh_target = if let Some(targets) = initial
            .citizen
            .production_targets
            .as_ref()
            .filter(|targets| targets.site == site)
        {
            crate::production::recipe_requirements_at(
                &initial.citizen,
                site,
                &targets.projected_order,
            )?
        } else {
            calculate(crate::production::DAILY_CAPACITY_MS)?
        };
        let cached = initial.citizen.stocking_requirements_at(site);
        let own = initial.citizen.own_workplace_supplies.get(&place).copied();
        let replace_own = |cached: crate::marketplace::ShoppingList,
                           own: crate::marketplace::ShoppingList,
                           fresh: crate::marketplace::ShoppingList| {
            crate::marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
                (
                    good,
                    cached
                        .units(good)
                        .saturating_sub(own.units(good))
                        .saturating_add(fresh.units(good)),
                )
            }))
        };
        let (threshold, target) = if let Some((cached_threshold, cached_target)) = cached {
            let (own_threshold, own_target) = own.unwrap_or_default();
            (
                replace_own(cached_threshold, own_threshold, fresh_threshold)?,
                replace_own(cached_target, own_target, fresh_target)?,
            )
        } else {
            (fresh_threshold, fresh_target)
        };
        if threshold.items().all(|(good, units)| {
            let available = if cached.is_some() {
                initial.citizen.stored_available_units(place, good)
            } else {
                initial.citizen.usable_owned_units(good)
            };
            available.saturating_add(initial.citizen.incoming_workplace_units(place, good)) >= units
        }) {
            return Ok(initial.clone());
        }
        target
    } else {
        let Some(recipes) = initial.citizen.production_restock_recipes_at(site)? else {
            return Ok(initial.clone());
        };
        let target = crate::production::recipe_requirements_at(&initial.citizen, site, &recipes)?;
        target
    };
    let target = crate::marketplace::ShoppingList::new(target.items().map(|(good, units)| {
        (
            good,
            units.saturating_sub(site.map_or(0, |place| {
                initial.citizen.incoming_workplace_units(place, good)
            })),
        )
    }))?;
    let demand_for = |requirements: crate::marketplace::ShoppingList| {
        crate::marketplace::ShoppingList::new(requirements.items().map(|(good, units)| {
            (
                good,
                units.saturating_sub(initial.citizen.usable_owned_units(good)),
            )
        }))
    };
    let demand = demand_for(target)?;
    record_blocked_purchase(initial, demand)?;
    let requirements = target;
    let market = initial.citizen.market();
    let tolerance =
        f64::from(market.production_adjustment_rate(initial.citizen.economic_id())) / 100.0;
    let quote_ceilings = std::array::from_fn(|index| {
        market
            .lowest_purchase_quote(
                initial.citizen.market_party(),
                Good::ALL[index],
                &initial.citizen.map(),
            )
            .map_or(0.0, |price| price * (1.0 + tolerance))
    });
    let mut state = initial.clone();
    for (good, units) in requirements.items() {
        let Some(collected) = collect_stored_inputs(&state, good, units, 0, prior_actions)? else {
            return Ok(initial.clone());
        };
        state = collected;
        let Some(withdrawn) = collect_listed_inputs(&state, good, units, 0, prior_actions)? else {
            return Ok(initial.clone());
        };
        state = withdrawn;
    }
    let list = crate::marketplace::ShoppingList::new(requirements.items().map(|(good, units)| {
        (
            good,
            units.saturating_sub(state.citizen.available_units(good)),
        )
    }))?
    .with_quote_ceilings(quote_ceilings, initial.citizen.coins());
    if list.items().next().is_none() {
        return deliver_workplace_inputs(state, site, target, prior_actions);
    }
    let supplied = shopping_trip_from_mode(&state, list, 0, prior_actions, true)?
        .pop()
        .unwrap_or(state);
    deliver_workplace_inputs(supplied, site, target, prior_actions)
}

pub(super) fn production_prefixes(
    initial: Prediction,
    prior_actions: usize,
    diagnostics: &mut [super::ProductionDecision],
) -> Result<Vec<Prediction>, SimulationError> {
    production_prefixes_with_locations(initial, prior_actions, diagnostics, &mut [])
}

pub(super) fn production_prefixes_with_locations(
    initial: Prediction,
    prior_actions: usize,
    diagnostics: &mut [super::ProductionDecision],
    locations: &mut [super::ProductionLocationForecast],
) -> Result<Vec<Prediction>, SimulationError> {
    let mut variants = Vec::new();
    for site in initial.citizen.production_sites() {
        variants.extend(production_at_site(
            initial.clone(),
            site,
            prior_actions,
            diagnostics,
            locations
                .iter_mut()
                .find(|location| location.place == site)
                .map(|location| &mut location.projection),
        )?);
    }
    Ok(variants)
}

fn production_at_site(
    initial: Prediction,
    site: crate::locations::PlaceId,
    prior_actions: usize,
    diagnostics: &mut [super::ProductionDecision],
    projection: Option<&mut Option<super::ProductionProjection>>,
) -> Result<Vec<Prediction>, SimulationError> {
    let mut initial = initial;
    let (targets, blocks) = crate::production::ProductionTargets::calculate_at_with_blocks(
        &initial.citizen,
        initial.citizen.current_time_ms(),
        Some(site),
    )?;
    if let Some(projection) = projection {
        *projection = Some(super::ProductionProjection::from_targets(
            &initial.citizen,
            &targets,
        )?);
    }
    initial.citizen.production_targets = Some(targets);
    initial.production_prices = Some(crate::production::ProductionPrices::snapshot(
        &initial.citizen,
    ));
    let market = initial.citizen.market();
    let tolerance =
        f64::from(market.production_adjustment_rate(initial.citizen.economic_id())) / 100.0;
    initial.production_quote_ceilings = Some(std::array::from_fn(|index| {
        market
            .lowest_purchase_quote(
                initial.citizen.market_party(),
                Good::ALL[index],
                &initial.citizen.map(),
            )
            .map_or(0.0, |quote| quote * (1.0 + tolerance))
    }));
    let mut state = if initial.citizen.brewing_actions_at(site).is_empty() {
        production_restock_at(&initial, prior_actions, Some(site))?
    } else {
        initial.clone()
    };
    let restocked = state.actions.iter().any(|action| {
        matches!(
            action,
            CitizenAction::BuyAt { .. }
                | CitizenAction::CollectStored { .. }
                | CitizenAction::Withdraw(..)
                | CitizenAction::WithdrawAt { .. }
                | CitizenAction::DepositStored { .. }
        )
    });
    let mut blocks = blocks.into_iter();
    let mut productive = false;
    while state.elapsed_ms < PRODUCTION_DURATION_MS || !productive {
        let farm_actions = state.citizen.farming_actions_at(site);
        if let Some(action) = state.citizen.brewing_actions_at(site).first().copied()
            && let Some(ready) = prepare(&state, action, 0.0, 0, prior_actions)?
                .into_iter()
                .next()
            && let Some(next) = ready.perform(action, prior_actions)?
        {
            state = next;
            productive = true;
            continue;
        }
        if let Some(action) = farm_actions
            .iter()
            .copied()
            .find(|action| !matches!(action, CitizenAction::Plant { .. }))
            && let Some(ready) = prepare(&state, action, 0.0, 0, prior_actions)?
                .into_iter()
                .next()
            && let Some(next) = ready.perform(action, prior_actions)?
        {
            state = next;
            productive = true;
            continue;
        }
        let Some(block) = blocks.next() else {
            break;
        };
        let mut qualified = state.clone();
        qualified.qualified_production_route = true;
        if let [recipe] = block.as_slice()
            && let Some(kind) = crate::production::crop_kind(*recipe)
        {
            let Some(action) = farm_actions.iter().copied().find(
                |action| matches!(action, CitizenAction::Plant { crop, .. } if *crop == kind),
            ) else {
                continue;
            };
            if let Some(diagnostic) = diagnostics.iter_mut().find(|d| d.recipe == *recipe) {
                diagnostic.prefix_attempted = true;
            }
            if let Some(ready) = prepare(&qualified, action, 0.0, 0, prior_actions)?
                .into_iter()
                .next()
                && let Some(mut next) = ready.perform_commercial(action, prior_actions)?
            {
                next.qualified_production_route = false;
                state = next;
                productive = true;
            }
            continue;
        }
        let mut preparation_limit = false;
        let next = production_order_bounded(
            &qualified,
            &block,
            None,
            0,
            prior_actions,
            &mut preparation_limit,
            Some(site),
        )?;
        for recipe in &block {
            if let Some(diagnostic) = diagnostics.iter_mut().find(|d| d.recipe == *recipe) {
                diagnostic.prefix_attempted = true;
                diagnostic.preparation_limit_observed |= preparation_limit;
            }
        }
        if let Some(mut candidate) = next
            .into_iter()
            .max_by(|a, b| a.score.mean.total_cmp(&b.score.mean))
        {
            candidate.qualified_production_route = false;
            state = candidate;
            productive = true;
        }
    }
    Ok(if productive || restocked {
        vec![state]
    } else {
        Vec::new()
    })
}

pub(super) fn need_eligible(citizen: &Citizen, goal: Effect) -> bool {
    match goal {
        Effect::ReduceHunger => citizen.hunger() > 0.0,
        Effect::ReduceThirst => citizen.thirst() > 0.0,
        Effect::ReduceTiredness => citizen.tiredness() > 0.0,
        Effect::ReduceClothingNeed => citizen.garment_condition().unwrap_or(0.0) <= 0.1,
        Effect::ReplenishReserves => provisioning_needed(citizen),
        _ => true,
    }
}

fn variants_from(
    initial: Prediction,
    goal: Effect,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let citizen = &initial.citizen;
    if matches!(goal, Effect::ReplenishReserves | Effect::ListExcess)
        && (prior_actions > 0 || !citizen.maintenance_available(goal))
    {
        return Ok(Vec::new());
    }
    if !need_eligible(citizen, goal) {
        return Ok(Vec::new());
    }
    if goal == Effect::Wait {
        let mut waited = initial;
        while waited.elapsed_ms < WAIT_GOAL_DURATION_MS {
            let Some(next) = waited.perform(CitizenAction::Wait, prior_actions)? else {
                return Ok(Vec::new());
            };
            waited = next;
        }
        return Ok(vec![waited]);
    }
    let mut variants: Vec<Prediction> = Vec::new();
    if matches!(goal, Effect::ReduceHunger | Effect::ReduceThirst) {
        let mut ready = initial.clone();
        if citizen.position() != citizen.map().position(citizen.home()) {
            let Some(home) =
                initial.perform(CitizenAction::Travel(citizen.home()), prior_actions)?
            else {
                return Ok(variants);
            };
            ready = home;
        }
        let action = if goal == Effect::ReduceHunger {
            CitizenAction::Eat
        } else {
            CitizenAction::Drink
        };
        if let Some(consumed) = ready.perform(action, prior_actions)? {
            variants.push(consumed);
        }
        return Ok(variants);
    }
    let mut consider = |candidate: Prediction| {
        if !variants.iter().any(|old| old.actions == candidate.actions) {
            variants.push(candidate);
        }
    };
    if goal == Effect::ReduceClothingNeed {
        if citizen.clothing_need() <= 0.0 {
            return Ok(variants);
        }
        let garments = crate::marketplace::ShoppingList::single(Good::FlaxGarment, 1);
        for ready in prepare_inputs(&initial, &garments, 0, prior_actions)? {
            for candidate in
                order_variants(&ready, CitizenAction::EquipClothing, 0.0, 0, prior_actions)?
            {
                consider(candidate);
            }
        }
        return Ok(variants);
    }
    if goal == Effect::Production {
        return production_prefixes(initial, prior_actions, &mut []);
    }
    if goal == Effect::ReplenishReserves {
        return provision_household(&initial, prior_actions);
    }
    let goal_actions: Vec<_> = ACTIONS
        .into_iter()
        .filter(|action| action.effects().contains(&goal))
        .collect();
    if goal == Effect::ListExcess && citizen.excess_value()? >= crate::production::MIN_LISTING_VALUE
    {
        return listing_trip(&initial, prior_actions);
    }
    for action in goal_actions {
        let candidates = order_variants(&initial, action, MEAL_NOURISHMENT, 0, prior_actions)?;
        for candidate in candidates {
            consider(candidate);
        }
        // Existing small meals remain useful when acquiring a full meal would delay relief.
        if action == CitizenAction::Eat
            && citizen.available_food_nutrition() < MEAL_NOURISHMENT
            && (prior_actions == 0 || citizen.available_food_nutrition() >= REPLAN_MIN_NUTRITION)
            && let Some(candidate) = initial.perform(action, prior_actions)?
        {
            consider(candidate);
        }
    }
    Ok(variants)
}

#[cfg(test)]
mod tests {
    #[test]
    fn clothing_thresholds_and_spare_only_replenishment() {
        let source = reserve_shopper(10000, &[(Good::FlaxGarment, 10, 1.0)])
            .with_home_good(Good::BreadRoll, 10)
            .unwrap()
            .with_home_good(Good::Water, 1200)
            .unwrap()
            .with_garment_condition(Some(0.2))
            .unwrap();
        assert!(!need_eligible(&source, Effect::ReduceClothingNeed));
        assert!(need_eligible(&source, Effect::ReplenishReserves));
        let supplied =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        assert_eq!(supplied.len(), 1);
        assert_eq!(supplied[0].citizen.home_units(Good::FlaxGarment), 1);
        assert!(!provisioning_needed(&supplied[0].citizen));
        assert!(need_eligible(
            &source.with_garment_condition(Some(0.1)).unwrap(),
            Effect::ReduceClothingNeed
        ));
        let surplus = source.with_home_good(Good::FlaxGarment, 2).unwrap();
        assert_eq!(surplus.excess_goods().unwrap().units(Good::FlaxGarment), 1);
    }

    #[test]
    fn listing_trip_splits_competing_dry_goods_and_unrelated_goods_with_real_travel() {
        use crate::locations::{Location, Map, Position};
        for textiles in [false, true] {
            let mut citizen = Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Wheat, 100)
                .unwrap()
                .with_good(Good::Hops, 100)
                .unwrap()
                .with_map(
                    Map::new(
                        Position::default(),
                        Position::default(),
                        Position { x: 100.0, y: 0.0 },
                    )
                    .unwrap(),
                )
                .unwrap();
            if textiles {
                citizen = citizen.with_good(Good::Thread, 100).unwrap();
            }
            let (map, bakery) = citizen
                .map
                .with_place(
                    Location::Bakery,
                    Position::default(),
                    Some(citizen.id),
                    "Bakery",
                )
                .unwrap();
            citizen.map = map;
            citizen.designated_workplace = Some(bakery);
            let market = citizen.map.public_place(Location::Market);
            let initial = Prediction::new(&citizen, Cooldowns::default());
            let trip = listing_trip(&initial, 0).unwrap().pop().unwrap();
            assert!(trip.actions.contains(&CitizenAction::Travel(market)));
            let orders: Vec<_> = trip.citizen.market.orders().collect();
            assert_eq!(
                orders
                    .iter()
                    .find(|order| order.good == Good::Wheat)
                    .unwrap()
                    .place,
                bakery
            );
            assert_eq!(
                orders
                    .iter()
                    .find(|order| order.good == Good::Hops)
                    .unwrap()
                    .place,
                market
            );
            if textiles {
                assert_eq!(
                    orders
                        .iter()
                        .find(|order| order.good == Good::Thread)
                        .unwrap()
                        .place,
                    market
                );
            }
            trip.citizen
                .storage
                .validate_objects(&trip.citizen.map, &trip.citizen.market)
                .unwrap();
            assert_eq!(citizen.market.orders().count(), 0);
        }
    }

    #[test]
    fn listing_transports_the_selected_surplus_without_reinterpreting_its_reserves() {
        use crate::locations::{Location, Map, Position};
        let source = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(crate::StartingRole::Brewer)
            .with_map(
                Map::new(
                    Position::default(),
                    Position::default(),
                    Position { x: 200.0, y: 0.0 },
                )
                .unwrap(),
            )
            .unwrap();
        let (map, brewery) = source
            .map
            .with_place(
                Location::Brewery,
                Position { x: 100.0, y: 0.0 },
                Some(source.id),
                "Brewery",
            )
            .unwrap();
        let mut source = source.with_map(map).unwrap();
        source.designated_workplace = Some(brewery);
        source = source
            .with_home_good(Good::Berries, 20000)
            .unwrap()
            .with_home_good(Good::BreadRoll, 200)
            .unwrap()
            .with_home_good(Good::Fish, 200)
            .unwrap()
            .with_home_good(Good::Water, 50000)
            .unwrap();
        source.production_targets = Some(
            crate::production::ProductionTargets::calculate_at(&source, 0, Some(brewery)).unwrap(),
        );
        let selected = source.excess_goods().unwrap();
        assert!(selected.units(Good::Water) > 0);
        assert!(selected.units(Good::Berries) > 0);
        let trip = listing_trip(&Prediction::new(&source, Cooldowns::default()), 0)
            .unwrap()
            .pop()
            .unwrap();
        let market = source.map.public_place(Location::Market);
        assert!(trip.actions.contains(&CitizenAction::Travel(brewery)));
        assert!(trip.actions.contains(&CitizenAction::Travel(market)));
        let mut replayed = source.clone();
        let mut acquired_water = false;
        for &action in &trip.actions {
            let started = replayed.start_action(action).unwrap();
            let duration = started.active_action().unwrap().duration_ms();
            replayed = started.advance(duration).unwrap();
            acquired_water |= replayed.production_acquisitions[Good::Water as usize] > 0;
        }
        assert!(acquired_water);
        for (good, units) in selected.items() {
            assert_eq!(replayed.market.listed_units(replayed.id, good), units);
            assert_eq!(trip.citizen.market.listed_units(source.id, good), units);
        }
        assert_eq!(replayed.production_acquisitions[Good::Water as usize], 0);
        assert_eq!(source.market.orders().count(), 0);
    }

    #[test]
    fn listing_keeps_only_completed_stops_when_later_travel_exceeds_the_horizon() {
        use crate::locations::{Location, Map, Position};
        let source = Citizen::new(0.0)
            .unwrap()
            .with_good(Good::Wheat, 100)
            .unwrap()
            .with_good(Good::Thread, 100)
            .unwrap()
            .with_map(
                Map::new(
                    Position::default(),
                    Position::default(),
                    Position { x: 100.0, y: 0.0 },
                )
                .unwrap(),
            )
            .unwrap();
        let (map, bakery) = source
            .map
            .with_place(
                Location::Bakery,
                Position::default(),
                Some(source.id),
                "Bakery",
            )
            .unwrap();
        let mut source = source.with_map(map).unwrap();
        source.designated_workplace = Some(bakery);
        let mut initial = Prediction::new(&source, Cooldowns::default());
        initial.elapsed_ms = GOAL_HORIZON_MS - TRADE_DURATION_MS;
        let trip = listing_trip(&initial, 0).unwrap().pop().unwrap();
        assert_eq!(trip.actions.len(), 1);
        assert!(matches!(trip.actions[0], CitizenAction::ListAt { place, .. } if place == bakery));
        assert_eq!(trip.action_durations_ms, vec![TRADE_DURATION_MS]);
        assert_eq!(
            trip.citizen.market.listed_units(source.id, Good::Wheat),
            100
        );
        assert_eq!(trip.citizen.market.listed_units(source.id, Good::Thread), 0);
        assert_eq!(trip.citizen.units(Good::Thread), 100);
        assert_eq!(trip.citizen.position(), source.position());
    }

    #[test]
    fn missing_diversity_alone_does_not_trigger_replenishment() {
        let source = reserve_shopper(1000, &[])
            .with_home_good(Good::BreadRoll, 100)
            .unwrap()
            .with_home_good(Good::Water, 1200)
            .unwrap()
            .with_garment_condition(Some(1.0))
            .unwrap();
        let initial = Prediction::new(&source, Cooldowns::default());
        assert!(provision_household(&initial, 0).unwrap().is_empty());
        assert_eq!(
            *initial.blocked_requests.borrow(),
            crate::marketplace::ShoppingList::default()
        );
    }

    #[test]
    fn actual_provisioning_coalesces_fragmented_orders_and_uses_rounded_cost() {
        let source = reserve_shopper(1, &[(Good::BreadRoll, 5, 0.08); 55])
            .with_home_good(Good::Water, 1200)
            .unwrap();
        let initial = Prediction::new(&source, Cooldowns::default());
        let acquired = acquire_provisioning_list(
            &initial,
            crate::marketplace::ShoppingList::single(Good::BreadRoll, 10),
            0,
        )
        .unwrap();
        assert_eq!(acquired.citizen.coins(), 0);
        assert_eq!(
            acquired.citizen.carried_available_units(Good::BreadRoll),
            10
        );
        assert_eq!(acquired.actions.len(), 1);
        let delivered = deliver_reserves(&acquired, 0).unwrap().unwrap();
        assert_eq!(delivered.citizen.home_food_nutrition(), 100.0);
    }

    #[test]
    fn empty_unfunded_pantry_delivers_partial_food_and_drink() {
        let mut source = reserve_shopper(0, &[]);
        source.hunger = 50.0;
        let candidates =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        let delivered = &candidates[0];
        assert!(delivered.citizen.home_food_nutrition() > 0.0);
        assert!(delivered.citizen.home_drink_units() >= 400);
        assert!(delivered.citizen.home_food_nutrition() < 100.0);
        assert!(matches!(
            delivered.actions.last(),
            Some(CitizenAction::DepositStored { .. })
        ));
        assert!(delivered.elapsed_ms <= GOAL_HORIZON_MS + crate::TRADE_DURATION_MS);
        let meals = variants_after(
            &Prediction::new(&delivered.citizen, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(!meals.is_empty());
        assert!(meals[0].citizen.hunger() < delivered.citizen.hunger());
        assert!(meals[0].citizen.thirst() < delivered.citizen.thirst());
    }

    #[test]
    fn actual_partial_provisioning_keeps_selected_unmet_demand() {
        let source = reserve_shopper(3, &[(Good::Fish, 1, 2.0)])
            .with_home_good(Good::Water, 1200)
            .unwrap();
        let initial = Prediction::new(&source, Cooldowns::default());
        let candidates = provision_household(&initial, 0).unwrap();
        assert!(candidates[0].citizen.home_units(Good::Fish) > 0);
        assert!(candidates[0].citizen.home_food_nutrition() > 0.0);
        assert!(initial.blocked_requests.borrow().items().next().is_some());
    }

    #[test]
    fn attainable_three_categories_beat_nominally_cheap_unavailable_food() {
        let mut source = reserve_shopper(
            1000,
            &[(Good::Cabbage, 20, 2.0), (Good::Berries, 10000, 2.0)],
        )
        .with_home_good(Good::BreadRoll, 100)
        .unwrap()
        .with_home_good(Good::Water, 1200)
        .unwrap();
        source = source.with_prices(source.prices.with_price(Good::Fish, 0.01).unwrap());
        let initial = Prediction::new(&source, Cooldowns::default());
        let candidates = provision_household(&initial, 0).unwrap();
        let supplied = &candidates[0].citizen;
        assert!(supplied.home_units(Good::Cabbage) >= 3);
        assert!(
            supplied.home_units(Good::Berries) as f64 * crate::BERRY_NUTRITION_PER_GRAM >= 75.0
        );
        assert_eq!(initial.blocked_requests.borrow().units(Good::Fish), 0);
    }

    #[test]
    fn owned_workplace_food_is_costed_before_market_category_choice() {
        use crate::{
            locations::{Location, Position},
            storage::StockKey,
        };
        let mut source = reserve_shopper(
            1000,
            &[
                (Good::Cabbage, 20, 1000.0),
                (Good::Fish, 20, 2.0),
                (Good::Berries, 10000, 2.0),
            ],
        )
        .with_home_good(Good::BreadRoll, 100)
        .unwrap()
        .with_home_good(Good::Water, 1200)
        .unwrap();
        let (map, workplace) = source
            .map()
            .with_place(
                Location::Field,
                Position::default(),
                Some(source.id),
                "Workplace",
            )
            .unwrap();
        source.map = map;
        source
            .storage
            .set_units(
                StockKey {
                    place: workplace,
                    owner: source.goods_owner(),
                    good: Good::Cabbage,
                },
                3,
                &source.map,
                &source.market,
            )
            .unwrap();
        let candidates =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        let supplied = &candidates[0].citizen;
        assert_eq!(supplied.home_units(Good::Cabbage), 3);
        assert!(candidates[0].actions.iter().any(|action| matches!(action, CitizenAction::CollectStored { place, list } if *place == workplace && list.units(Good::Cabbage) == 3)));
        assert!(candidates[0].actions.iter().all(|action| !matches!(action, CitizenAction::BuyAt { list, .. } if list.units(Good::Cabbage) > 0)));
    }

    #[test]
    fn replenishment_acquires_a_remote_spare_and_delivers_it() {
        use crate::locations::{Location, Position};
        let source = reserve_shopper(1000, &[])
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap()
            .with_good(Good::Fish, 3)
            .unwrap()
            .with_good(Good::Cabbage, 3)
            .unwrap();
        let (map, remote) = source
            .map()
            .with_place(
                Location::Tailory,
                Position { x: 1000.0, y: 0.0 },
                Some(crate::AgentId(uuid::Uuid::new_v4())),
                "Remote tailor",
            )
            .unwrap();
        let mut source = source.with_map(map).unwrap();
        source
            .market
            .list(
                crate::AgentId(uuid::Uuid::new_v4()),
                remote,
                Good::FlaxGarment,
                1,
            )
            .unwrap();
        let supplied =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        assert!(supplied[0].citizen.home_food_nutrition() >= 300.0);
        assert_eq!(supplied[0].citizen.home_units(Good::FlaxGarment), 1);
        assert!(supplied[0].actions.contains(&CitizenAction::Travel(remote)));
    }

    #[test]
    fn listing_after_provisioning_retains_the_three_category_pantry() {
        let mut source = reserve_shopper(
            1000,
            &[
                (Good::Cabbage, 20, 2.0),
                (Good::Berries, 10000, 2.0),
                (Good::FlaxGarment, 1, 1.0),
            ],
        )
        .with_home_good(Good::BreadRoll, 100)
        .unwrap()
        .with_home_good(Good::Water, 1200)
        .unwrap();
        source.diet = [100.0; 4];
        source.garment_condition = Some(0.2);
        let supplied =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        let listed = supplied[0]
            .citizen
            .start_action(CitizenAction::ListExcess)
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert!(!provisioning_needed(&listed));
        assert!(listed.home_units(Good::Cabbage) >= 3);
        assert!(listed.home_units(Good::Berries) as f64 * crate::BERRY_NUTRITION_PER_GRAM >= 75.0);
    }

    #[test]
    fn pantry_shortages_ignore_carried_and_other_members_uncommitted_stock() {
        let mut source = reserve_shopper(0, &[])
            .with_good(Good::BreadRoll, 100)
            .unwrap()
            .with_good(Good::Water, 5000)
            .unwrap();
        source.household = Some(crate::households::HouseholdContext {
            id: crate::AgentId(uuid::Uuid::new_v4()),
            members: vec![source.id, crate::AgentId(uuid::Uuid::new_v4())],
            allegiance: 0.0,
            other_carried: [10000; Good::COUNT],
            other_consumption_value: 0.0,
            other_clothing_replacements: 0,
        });
        assert!(provisioning_needed(&source));
        assert!(source.urgent_provisioning());
        assert_eq!(reserve_units(&source, Good::BreadRoll), 100);
        let variants =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        assert!(variants.iter().any(|candidate| {
            candidate
                .actions
                .last()
                .is_some_and(|action| matches!(action, CitizenAction::DepositStored { .. }))
        }));
    }

    #[test]
    fn household_baskets_scale_and_count_mixed_food_and_beer() {
        let mut source = reserve_shopper(
            10000,
            &[
                (Good::Fish, 100, 2.0),
                (Good::Cabbage, 100, 2.0),
                (Good::BerryPie, 100, 2.0),
                (Good::Beer, 10000, 2.0),
            ],
        );
        source.household = Some(crate::households::HouseholdContext {
            id: crate::AgentId(uuid::Uuid::new_v4()),
            members: vec![source.id, crate::AgentId(uuid::Uuid::new_v4())],
            allegiance: 0.0,
            other_carried: [0; Good::COUNT],
            other_consumption_value: 0.0,
            other_clothing_replacements: 0,
        });
        let list = provisioning_basket(&source, 3, false).unwrap();
        let nutrition: f64 = list
            .items()
            .map(|(good, units)| {
                units as f64
                    * good.nutrition_per_unit().unwrap_or(if good == Good::Beer {
                        0.05
                    } else {
                        0.0
                    })
            })
            .sum();
        assert!(nutrition >= 600.0);
        assert!(list.units(Good::Water) + list.units(Good::Beer) >= 2400);
        let mut points = [0.0; 4];
        for (good, units) in list.items() {
            for (total, per_unit) in points.iter_mut().zip(good.diet_per_unit()) {
                *total += units as f64 * per_unit;
            }
        }
        assert!(points[..3].iter().all(|points| *points >= 150.0));
    }

    #[test]
    fn full_monoculture_pantry_still_buys_three_categories_and_excludes_expensive_fourth() {
        let source = reserve_shopper(
            1000,
            &[
                (Good::Fish, 100, 1000.0),
                (Good::Cabbage, 100, 2.0),
                (Good::Berries, 10000, 2.0),
            ],
        )
        .with_home_good(Good::BreadRoll, 100)
        .unwrap()
        .with_home_good(Good::Water, 1200)
        .unwrap();
        assert!(provisioning_needed(&source));
        let variants =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        let supplied = &variants[0].citizen;
        assert_eq!(supplied.home_units(Good::Fish), 0);
        assert!(supplied.home_units(Good::Cabbage) >= 3);
        assert!(
            supplied.home_units(Good::Berries) as f64 * crate::BERRY_NUTRITION_PER_GRAM >= 75.0
        );
        assert_eq!(supplied.home_units(Good::BreadRoll), 100);
    }

    #[test]
    fn stocked_pantry_does_not_shop_for_individual_diet_or_spare() {
        let mut source = reserve_shopper(
            10000,
            &[(Good::Fish, 100, 1.0), (Good::FlaxGarment, 10, 1.0)],
        )
        .with_home_good(Good::BreadRoll, 10)
        .unwrap()
        .with_home_good(Good::Cabbage, 1)
        .unwrap()
        .with_home_good(Good::Fish, 1)
        .unwrap()
        .with_home_good(Good::Water, 400)
        .unwrap();
        source.diet = [0.0; 4];
        source.garment_condition = Some(1.0);
        assert!(!provisioning_needed(&source));
        assert!(
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn provisioning_delivers_useful_partial_market_supply() {
        let source = reserve_shopper(3, &[(Good::Fish, 1, 2.0)])
            .with_home_good(Good::Water, 1200)
            .unwrap();
        let state = Prediction::new(&source, Cooldowns::default());
        let acquired = acquire_provisioning_list(
            &state,
            crate::marketplace::ShoppingList::new([(Good::Fish, 4), (Good::Cabbage, 3)]).unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(acquired.citizen.carried_available_units(Good::Fish), 1);
        let delivered = deliver_reserves(&acquired, 0).unwrap().unwrap();
        assert_eq!(delivered.citizen.home_units(Good::Fish), 1);
        assert_eq!(delivered.citizen.coins(), 1);
    }

    #[test]
    fn urgent_home_shortages_bypass_cooldown_but_never_shared_claims() {
        let mut source = reserve_shopper(0, &[])
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap();
        source.maintenance.replenishment.completed_at_ms = Some(source.current_time_ms());
        assert!(source.maintenance_available(Effect::ReplenishReserves));
        let plan = variants_after(
            &Prediction::new(&source, Cooldowns::default()),
            Effect::ReplenishReserves,
        )
        .unwrap();
        assert!(!plan.is_empty());
        source.maintenance.replenishment.claimant = Some(crate::AgentId(uuid::Uuid::new_v4()));
        assert!(!source.maintenance_available(Effect::ReplenishReserves));
        assert!(
            variants_after(
                &Prediction::new(&source, Cooldowns::default()),
                Effect::ReplenishReserves
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn provisioning_retrieves_remote_owned_spare_before_buying_local_replacement() {
        use crate::locations::{Location, Position};
        let source = reserve_shopper(1000, &[(Good::FlaxGarment, 10, 1.0)])
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap()
            .with_good(Good::Fish, 3)
            .unwrap()
            .with_good(Good::Cabbage, 3)
            .unwrap();
        let (map, remote) = source
            .map()
            .with_place(
                Location::Tailory,
                Position { x: 10.0, y: 0.0 },
                Some(source.id()),
                "Own tailor",
            )
            .unwrap();
        let mut source = source.with_map(map).unwrap();
        source
            .market
            .list(source.id(), remote, Good::FlaxGarment, 1)
            .unwrap();
        let variants =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        assert_eq!(variants[0].citizen.home_units(Good::FlaxGarment), 1);
        assert!(variants[0].actions.contains(&CitizenAction::Travel(remote)));
        assert!(!variants[0].actions.iter().any(|action| matches!(action, CitizenAction::BuyAt { list, .. } if list.units(Good::FlaxGarment) > 0)));
    }

    #[test]
    fn useful_provisioning_adds_one_shared_spare_after_basic_coverage() {
        let source = reserve_shopper(1000, &[(Good::FlaxGarment, 10, 1.0)])
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap()
            .with_good(Good::Fish, 3)
            .unwrap()
            .with_good(Good::Cabbage, 3)
            .unwrap();
        let variants =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        assert_eq!(variants[0].citizen.home_units(Good::FlaxGarment), 1);
        assert_eq!(
            variants[0]
                .citizen
                .carried_available_units(Good::FlaxGarment),
            0
        );
    }

    #[test]
    fn provisioning_evaluates_food_after_delivery_and_consumption_keeps_travel_cost() {
        use crate::locations::{Location, Position};
        let mut source = reserve_shopper(0, &[])
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap()
            .with_good(Good::Fish, 3)
            .unwrap()
            .with_good(Good::Cabbage, 3)
            .unwrap();
        source.hunger = 50.0;
        let (map, remote) = source
            .map()
            .with_place(
                Location::Market,
                Position { x: 100.0, y: 0.0 },
                None,
                "Remote",
            )
            .unwrap();
        let source = source
            .with_map(map)
            .unwrap()
            .with_position(Position { x: 100.0, y: 0.0 })
            .unwrap();
        assert!(
            variants_after(
                &Prediction::new(&source, Cooldowns::default()),
                Effect::ReduceHunger
            )
            .unwrap()
            .is_empty()
        );
        let delivered =
            provision_household(&Prediction::new(&source, Cooldowns::default()), 0).unwrap();
        assert_eq!(
            delivered[0].actions.first(),
            Some(&CitizenAction::Travel(source.home()))
        );
        assert!(delivered[0].elapsed_ms > crate::TRADE_DURATION_MS);
        let away = delivered[0]
            .citizen
            .with_position(source.map().position(remote))
            .unwrap();
        let meal = variants_after(
            &Prediction::new(&away, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert_eq!(
            meal[0].actions,
            [CitizenAction::Travel(source.home()), CitizenAction::Eat]
        );
    }

    #[test]
    #[ignore]
    fn household_provisioning_town_smoke() {
        let path = std::env::var("LL_SMOKE_FIXTURE").unwrap();
        let source =
            crate::Universe::from_save_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let end = source.advance(crate::marketplace::DAY_MS).unwrap();
        let mut worst_hunger = f64::NEG_INFINITY;
        let mut worst_thirst = f64::NEG_INFINITY;
        let mut at_home_meals = 0;
        for agent in end.agents().values() {
            let crate::AgentKind::Citizen(citizen) = &agent.kind;
            worst_hunger = worst_hunger.max(citizen.hunger());
            worst_thirst = worst_thirst.max(citizen.thirst());
            if citizen.active_action().is_some_and(|active| {
                matches!(active.action(), CitizenAction::Eat | CitizenAction::Drink)
            }) {
                assert_eq!(citizen.position(), citizen.map().position(citizen.home()));
                at_home_meals += 1;
            }
        }
        eprintln!(
            "one-day pantry smoke worst hunger={worst_hunger}, thirst={worst_thirst}, active home consumption={at_home_meals}"
        );
        assert!(worst_hunger < 150.0 && worst_thirst < 150.0);
    }

    #[test]
    fn capped_commercial_gathering_is_excluded_while_ingredient_gathering_remains_available() {
        let mut citizen = crate::Citizen::new(0.0)
            .unwrap()
            .with_good(crate::marketplace::Good::Berries, 10_000)
            .unwrap();
        let mut policy = citizen.market.caravan_policy();
        policy.exports[crate::marketplace::Good::Berries as usize].allowed = false;
        citizen.market.set_caravan_policy(policy).unwrap();
        let prediction = super::Prediction::new(&citizen, super::Cooldowns::default());
        let action = crate::CitizenAction::Produce(crate::production::Recipe::Forage);
        assert!(
            crate::production::recipe_profit_for_site(
                crate::production::Recipe::Forage,
                &citizen,
                None
            )
            .is_none()
        );
        assert!(prediction.perform_commercial(action, 0).unwrap().is_none());
        assert!(prediction.perform(action, 0).unwrap().is_some());
    }

    use super::*;

    #[test]
    fn full_brewery_production_keeps_thirst_alternatives() {
        use crate::{brewing::Batch, locations::Location, production::Recipe};
        let mut worker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(crate::StartingRole::Brewer)
            .with_thirst(80.0)
            .unwrap()
            .with_home_good(Good::Water, 400)
            .unwrap();
        for &(good, units) in Recipe::BrewBeer.inputs() {
            worker = worker.with_good(good, units * 8).unwrap();
        }
        let (mut map, brewery) = worker
            .map()
            .with_place(
                Location::Brewery,
                crate::locations::Position { x: 10.0, y: 0.0 },
                Some(worker.id()),
                "Brewery",
            )
            .unwrap();
        for slot in 0..2 {
            map.set_batch(
                brewery,
                slot,
                Some(Batch::Fermenting {
                    ready_at_ms: crate::brewing::FERMENTATION_MS,
                }),
            )
            .unwrap();
        }
        let worker = worker.with_map(map).unwrap();
        let production = site_candidate(&worker, brewery);
        assert!(
            production
                .citizen
                .map()
                .place(brewery)
                .unwrap()
                .batches()
                .iter()
                .all(Option::is_some)
        );
        assert!(matches!(
            production
                .citizen
                .start_action(CitizenAction::Produce(Recipe::BrewBeer)),
            Err(SimulationError::MissingInputs)
        ));
        assert!(
            production
                .perform(CitizenAction::Produce(Recipe::BrewBeer), 0)
                .unwrap()
                .is_none()
        );
        let variants = variants_after(&production, Effect::ReduceThirst).unwrap();
        assert!(variants.iter().any(|variant| {
            variant.actions.last() == Some(&CitizenAction::Drink)
                && variant.citizen.thirst() < production.citizen.thirst()
        }));
    }

    #[test]
    fn prediction_preserves_errors_other_than_missing_inputs() {
        let mut worker = Citizen::new(0.0).unwrap();
        let busy = worker.start_action(CitizenAction::Wait).unwrap();
        assert!(matches!(
            Prediction::new(&busy, Cooldowns::default()).perform(CitizenAction::Wait, 0),
            Err(SimulationError::CitizenBusy)
        ));
        worker.market_time_ms = u64::MAX;
        assert!(matches!(
            Prediction::new(&worker, Cooldowns::default()).perform(CitizenAction::Wait, 0),
            Err(SimulationError::TimeOverflow)
        ));
    }

    fn site_candidate(initial: &Citizen, site: crate::locations::PlaceId) -> Prediction {
        production_at_site(
            Prediction::new(initial, Cooldowns::default()),
            site,
            0,
            &mut [],
            None,
        )
        .unwrap()
        .pop()
        .unwrap()
    }

    #[test]
    fn maintenance_is_first_only_and_replenishment_delivers_home() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_good(Good::BreadRoll, 10)
            .unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let delivered = variants_after(&initial, Effect::ReplenishReserves).unwrap();
        assert!(!delivered.is_empty());
        assert_eq!(
            citizen.maintenance,
            crate::maintenance::MaintenanceState::default()
        );
        for variant in &delivered {
            assert!(
                matches!(variant.actions.last(), Some(CitizenAction::DepositStored { place, .. }) if *place == citizen.home())
            );
            assert_eq!(variant.citizen.carried_available_units(Good::BreadRoll), 0);
            assert!(
                variant
                    .citizen
                    .stored_available_units(citizen.home(), Good::BreadRoll)
                    >= 10
            );
        }
        assert!(
            variants_after(&delivered[0], Effect::ReplenishReserves)
                .unwrap()
                .is_empty()
        );
        assert!(
            variants_after(&delivered[0], Effect::ListExcess)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn mixed_food_and_drink_purchases_keep_food_effects() {
        for drink in [Good::Water, Good::Beer] {
            let drinks = crate::marketplace::ShoppingList::single(drink, 200);
            assert_eq!(
                CitizenAction::Buy(drinks).effects(),
                &[Effect::ReplenishReserves]
            );
            let mixed = crate::marketplace::ShoppingList::new([(drink, 200), (Good::BreadRoll, 5)])
                .unwrap();
            assert!(CitizenAction::Buy(mixed).effects().contains(&Effect::Food));
        }
    }

    #[test]
    fn drink_reserves_pool_owned_listings_before_mixed_market_purchases() {
        let source = reserve_shopper(100, &[(Good::Beer, 100, 0.01)])
            .with_good(Good::Water, 100)
            .unwrap()
            .with_good(Good::Beer, 200)
            .unwrap();
        let listed = source
            .start_action(CitizenAction::List(Good::Beer, 100))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let prepared = [acquire_provisioning_list(
            &Prediction::new(&listed, Cooldowns::default()),
            crate::marketplace::ShoppingList::single(Good::Beer, 200),
            0,
        )
        .unwrap()];
        assert!(
            prepared
                .iter()
                .any(|p| p.citizen.available_drink_units() >= 400
                    && p.actions.first() == Some(&CitizenAction::Withdraw(Good::Beer, 100)))
        );
        let mixed = reserve_shopper(100, &[(Good::Water, 100, 0.01), (Good::Beer, 200, 0.01)])
            .with_good(Good::Water, 100)
            .unwrap();
        let prepared = [acquire_provisioning_list(
            &Prediction::new(&mixed, Cooldowns::default()),
            crate::marketplace::ShoppingList::new([(Good::Water, 100), (Good::Beer, 200)]).unwrap(),
            0,
        )
        .unwrap()];
        assert!(
            prepared
                .iter()
                .any(|p| reserve_units(&p.citizen, Good::Water) == 200
                    && p.citizen.available_units(Good::Beer) == 200)
        );
        let stocked = mixed
            .with_good(Good::Water, 200)
            .unwrap()
            .with_good(Good::Beer, 200)
            .unwrap()
            .with_good(Good::BreadRoll, 30)
            .unwrap();
        assert!(
            variants_after(
                &Prediction::new(&stocked, Cooldowns::default()),
                Effect::ReplenishReserves
            )
            .unwrap()
            .iter()
            .all(|p| !p.actions.iter().any(|a| matches!(
                a,
                CitizenAction::Withdraw(Good::Beer | Good::Water, _)
                    | CitizenAction::Produce(crate::production::Recipe::FetchWater)
            )))
        );
    }

    #[test]
    fn production_site_candidates_keep_each_workplace_separate() {
        use crate::{
            StartingRole,
            locations::{Location, Position},
            production::{Recipe, Skill},
        };
        let worker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_skill_practice_ms(Skill::Milling, Some(0))
            .unwrap()
            .with_good(Good::Wheat, 600)
            .unwrap()
            .with_good(Good::Flour, 500)
            .unwrap()
            .with_good(Good::Wood, 500)
            .unwrap()
            .with_good(Good::Water, 1000)
            .unwrap()
            .with_good(Good::Berries, 100)
            .unwrap();
        let (map, mill) = worker
            .map()
            .with_place(
                Location::Mill,
                Position { x: 10.0, y: 0.0 },
                Some(worker.id()),
                "Mill A",
            )
            .unwrap();
        let (map, second) = map
            .with_place(
                Location::Mill,
                Position { x: 20.0, y: 0.0 },
                Some(worker.id()),
                "Mill B",
            )
            .unwrap();
        let (map, bakery) = map
            .with_place(
                Location::Bakery,
                Position::default(),
                Some(worker.id()),
                "Bakery",
            )
            .unwrap();
        let (map, private) = map
            .with_place(
                Location::Mill,
                Position::default(),
                Some(crate::AgentId(uuid::Uuid::new_v4())),
                "Private mill",
            )
            .unwrap();
        let worker = worker.with_map(map).unwrap();
        let variants =
            production_prefixes(Prediction::new(&worker, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert_eq!(variants.len(), 5);
        for site in [
            mill,
            second,
            bakery,
            worker.map().public_place(Location::Forest),
            worker.map().public_place(Location::River),
        ] {
            let variant = variants.iter().find(|variant| variant.actions.iter().any(|action| matches!(action, CitizenAction::ProduceAt { place, .. } if *place == site))).unwrap();
            assert!(
                variant
                    .actions
                    .iter()
                    .filter_map(|action| match action {
                        CitizenAction::ProduceAt { place, .. } => Some(*place),
                        _ => None,
                    })
                    .all(|place| place == site)
            );
            assert_eq!(
                variant.citizen.production_targets().unwrap().site,
                Some(site)
            );
        }
        assert!(!worker.production_sites().contains(&private));
        let mill_plan = variants
            .iter()
            .find(|variant| {
                variant.actions.contains(&CitizenAction::ProduceAt {
                    place: mill,
                    recipe: Recipe::MillFlour,
                })
            })
            .unwrap();
        assert!(mill_plan.actions.contains(&CitizenAction::Travel(mill)));
        assert!(mill_plan.elapsed_ms > PRODUCTION_DURATION_MS);
    }

    #[test]
    fn production_site_bakery_mixes_recipes_without_other_site_work() {
        use crate::{locations::Location, production::Recipe};
        let worker = baker();
        let site = worker.production_place(Recipe::BakeRolls).unwrap();
        let variant = site_candidate(&worker, site);
        let recipes: Vec<_> = variant
            .actions
            .iter()
            .filter_map(|action| match *action {
                CitizenAction::ProduceAt { place, recipe } if place == site => Some(recipe),
                _ => None,
            })
            .collect();
        assert!(recipes.contains(&Recipe::BakeRolls));
        assert!(recipes.contains(&Recipe::BakeBerryPie));
        assert!(variant.actions.iter().all(|action| {
            action
                .production_recipe()
                .is_none_or(|recipe| recipe.location() == Location::Bakery)
        }));
        assert!(variant.elapsed_ms >= PRODUCTION_DURATION_MS);
    }

    #[test]
    fn production_site_restock_uses_only_its_mix_and_skips_a_covered_block() {
        use crate::{
            locations::{Location, Position},
            production::{Recipe, Skill},
        };
        let worker = bread_only(buying_baker(2000))
            .with_skill_practice_ms(Skill::Milling, Some(0))
            .unwrap();
        let (map, _) = worker
            .map()
            .with_place(
                Location::Mill,
                Position::default(),
                Some(worker.id()),
                "Mill",
            )
            .unwrap();
        let mut worker = worker.with_map(map).unwrap();
        let site = worker.production_place(Recipe::BakeRolls).unwrap();
        worker.production_targets = Some(
            crate::production::ProductionTargets::calculate_at(&worker, 0, Some(site)).unwrap(),
        );
        let variant = production_restock_at(
            &Prediction::new(&worker, Cooldowns::default()),
            0,
            Some(site),
        )
        .unwrap();
        assert!(
            variant
                .actions
                .iter()
                .any(|action| matches!(action, CitizenAction::BuyAt { .. }))
        );
        for action in &variant.actions {
            if let CitizenAction::BuyAt { list, .. } = action {
                assert_eq!(list.units(Good::Wheat), 0);
                assert_eq!(list.units(Good::Flour), 1200);
            }
        }
        let preferred =
            crate::production::projected_recipes_at(&worker, Some(site), PRODUCTION_DURATION_MS)
                .unwrap();
        for (good, units) in
            crate::production::recipe_requirements_at(&worker, Some(site), &preferred)
                .unwrap()
                .items()
        {
            worker = worker.with_good(good, units).unwrap();
        }
        let covered = production_restock_at(
            &Prediction::new(&worker, Cooldowns::default()),
            0,
            Some(site),
        )
        .unwrap();
        assert!(covered.actions.is_empty());
        assert!(
            worker
                .purchase_shortages()
                .unwrap()
                .items()
                .next()
                .is_none()
        );
    }

    #[test]
    fn production_site_missing_inputs_can_buy_elsewhere_and_long_preparation_finishes_a_job() {
        use crate::{
            locations::{Location, Position},
            production::Recipe,
        };
        let mut worker = bread_only(buying_baker(2000));
        let site = worker.production_place(Recipe::BakeRolls).unwrap();
        let mut map = worker.map();
        let mut suppliers = Vec::new();
        for index in 0..24 {
            let (next, place) = map
                .with_place(
                    Location::Warehouse,
                    Position { x: 1000.0, y: 0.0 },
                    None,
                    format!("Supplier {index}"),
                )
                .unwrap();
            map = next;
            suppliers.push(place);
        }
        worker = worker.with_map(map).unwrap();
        worker.market = crate::marketplace::Market::default();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        for &supplier in &suppliers {
            worker
                .market
                .list(seller, supplier, Good::Flour, 50)
                .unwrap();
        }
        worker
            .market
            .list(seller, suppliers[0], Good::Wood, 300)
            .unwrap();
        worker
            .market
            .list(seller, suppliers[0], Good::Water, 1200)
            .unwrap();
        let variant = site_candidate(&worker, site);
        assert!(variant.actions.iter().any(
            |action| matches!(action, CitizenAction::Travel(place) if suppliers.contains(place))
        ));
        assert!(variant.actions.contains(&CitizenAction::Travel(site)));
        assert!(variant.actions.contains(&CitizenAction::ProduceAt {
            place: site,
            recipe: Recipe::BakeRolls
        }));
        assert!(variant.elapsed_ms > PRODUCTION_DURATION_MS);
        assert_eq!(
            variant
                .actions
                .iter()
                .filter(|action| matches!(action, CitizenAction::ProduceAt { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn production_site_fields_care_for_their_own_crops_even_when_exports_are_disabled() {
        use crate::{
            StartingRole,
            farming::{Crop, CropKind, HOUR_MS},
            locations::{Location, Position},
        };
        let worker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Farmer);
        let (map, first) = worker
            .map()
            .with_place(
                Location::Field,
                Position::default(),
                Some(worker.id()),
                "First field",
            )
            .unwrap();
        let (mut map, second) = map
            .with_place(
                Location::Field,
                Position::default(),
                Some(worker.id()),
                "Second field",
            )
            .unwrap();
        for field in [first, second] {
            map.set_crop(
                field,
                0,
                Some(Crop {
                    kind: CropKind::Wheat,
                    planted_at_ms: 0,
                    tended: [false; 2],
                }),
            )
            .unwrap();
        }
        let mut worker = worker
            .with_map(map)
            .unwrap()
            .with_good(Good::Wheat, 100000)
            .unwrap();
        worker.market_time_ms = 24 * HOUR_MS;
        let mut policy = worker.market.caravan_policy();
        policy.exports[Good::Wheat as usize].allowed = false;
        worker.market.set_caravan_policy(policy).unwrap();
        for site in [first, second] {
            let variant = site_candidate(&worker, site);
            let fields: Vec<_> = variant
                .actions
                .iter()
                .filter_map(|action| action.crop_plot().map(|(field, _)| field))
                .collect();
            assert!(!fields.is_empty());
            assert!(fields.into_iter().all(|field| field == site));
            assert!(variant.actions.iter().any(|action| matches!(action, CitizenAction::Tend { field, stage: 0, .. } if *field == site)));
        }
    }

    #[test]
    fn site_specific_forage_preserves_the_existing_gathering_order() {
        let citizen = Citizen::new(0.0).unwrap();
        let mut initial = Prediction::new(&citizen, Cooldowns::default());
        initial.last_gathering_order =
            Some(CitizenAction::Produce(crate::production::Recipe::Forage));
        let action = CitizenAction::ProduceAt {
            place: citizen
                .map()
                .public_place(crate::locations::Location::Forest),
            recipe: crate::production::Recipe::Forage,
        };
        let next = initial.perform(action, 0).unwrap().unwrap();
        assert_eq!(next.last_gathering_order, initial.last_gathering_order);
        assert!(
            order_variants(&initial, action, 0.0, 0, 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn crop_planting_and_gathering_have_independent_site_candidates() {
        use crate::{StartingRole, locations::Location, production::Recipe};
        let citizen = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Farmer)
            .with_good(Good::BreadRoll, 70)
            .unwrap();
        let (map, field) = citizen
            .map()
            .with_place(
                Location::Field,
                citizen.position(),
                Some(citizen.id()),
                "Field",
            )
            .unwrap();
        let citizen = citizen.with_map(map).unwrap().with_prices(
            crate::marketplace::Prices::default()
                .with_price(Good::Wheat, 300.0)
                .unwrap(),
        );
        let variants =
            production_prefixes(Prediction::new(&citizen, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert_eq!(variants.len(), 3);
        let field_plan = variants
            .iter()
            .find(|variant| {
                variant
                    .actions
                    .iter()
                    .any(|action| action.crop_plot().is_some_and(|(site, _)| site == field))
            })
            .unwrap();
        assert!(
            matches!(field_plan.actions.first(), Some(CitizenAction::Plant { field: site, crop: crate::farming::CropKind::Wheat, .. }) if *site == field)
        );
        let citizen =
            citizen.with_prices(citizen.prices().with_price(Good::Berries, 5000.0).unwrap());
        let variants =
            production_prefixes(Prediction::new(&citizen, Cooldowns::default()), 0, &mut [])
                .unwrap();
        let forest = citizen.map().public_place(Location::Forest);
        let forest_plan = variants.iter().find(|variant| variant.actions.iter().any(|action| matches!(action, CitizenAction::ProduceAt { place, .. } if *place == forest))).unwrap();
        let first_work = forest_plan
            .actions
            .iter()
            .find(|action| !matches!(action, CitizenAction::Travel(_)))
            .unwrap();
        assert_eq!(
            *first_work,
            CitizenAction::ProduceAt {
                place: forest,
                recipe: Recipe::Forage
            }
        );
    }

    #[test]
    fn thirst_requires_delivery_before_home_consumption() {
        let source = Citizen::new(0.0)
            .unwrap()
            .with_thirst(80.0)
            .unwrap()
            .with_good(Good::Water, 100)
            .unwrap();
        assert!(
            variants_after(
                &Prediction::new(&source, Cooldowns::default()),
                Effect::ReduceThirst
            )
            .unwrap()
            .is_empty()
        );
        let delivered = deliver_reserves(&Prediction::new(&source, Cooldowns::default()), 0)
            .unwrap()
            .unwrap();
        let drinks = variants_after(
            &Prediction::new(&delivered.citizen, Cooldowns::default()),
            Effect::ReduceThirst,
        )
        .unwrap();
        assert_eq!(drinks[0].actions, [CitizenAction::Drink]);
        assert_eq!(drinks[0].citizen.home_drink_units(), 0);
        assert_eq!(drinks[0].citizen.carried_available_units(Good::Water), 0);
        assert!(drinks[0].citizen.thirst() < delivered.citizen.thirst());
    }

    #[test]
    fn personal_need_goals_require_positive_need_at_their_start() {
        for need in [-1.0, 0.0, 0.001] {
            let mut citizen = Citizen::new(need)
                .unwrap()
                .with_thirst(need)
                .unwrap()
                .with_home_good(Good::BreadRoll, 30)
                .unwrap()
                .with_home_good(Good::Water, 1200)
                .unwrap();
            citizen.tiredness = need;
            let initial = Prediction::new(&citizen, Cooldowns::default());
            for goal in [
                Effect::ReduceHunger,
                Effect::ReduceThirst,
                Effect::ReduceTiredness,
            ] {
                assert_eq!(
                    !variants_after(&initial, goal).unwrap().is_empty(),
                    need > 0.0,
                    "{goal:?}, need {need}"
                );
            }
        }
    }

    #[test]
    fn waiting_scores_exactly_zero_even_when_needs_cross_thresholds() {
        for need in [-110.0, -1.0, 0.0, 100.0] {
            let source = Citizen::with_needs(need, need)
                .unwrap()
                .with_thirst(need)
                .unwrap()
                .with_home_good(Good::Fish, 3)
                .unwrap();
            let waited = best_variant(&source, Effect::Wait, Cooldowns::default(), 0)
                .unwrap()
                .unwrap();
            assert_eq!(waited.average().unwrap(), 0.0);
            assert_eq!(
                waited.waiting.planning_wellbeing(),
                waited.citizen.planning_wellbeing()
            );
            assert_eq!(waited.waiting.market_time_ms, waited.citizen.market_time_ms);
        }
    }

    #[test]
    fn production_scores_its_gain_after_shared_need_deterioration_cancels() {
        let source = Citizen::with_needs(100.0, 100.0)
            .unwrap()
            .with_thirst(100.0)
            .unwrap();
        let action = CitizenAction::ProduceAt {
            place: source
                .map()
                .public_place(crate::locations::Location::Forest),
            recipe: crate::production::Recipe::Forage,
        };
        let produced = Prediction::new(&source, Cooldowns::default())
            .perform(action, 0)
            .unwrap()
            .unwrap();
        assert!(
            produced.citizen.planning_wellbeing().unwrap() < source.planning_wellbeing().unwrap()
        );
        let gain = produced.citizen.planning_wellbeing().unwrap()
            - produced.waiting.planning_wellbeing().unwrap();
        assert!(gain > 0.0);
        assert!((produced.average().unwrap() - gain * 0.5).abs() < 1e-12);
        assert_eq!(produced.waiting.market(), source.market());
        assert_eq!(produced.waiting.berries_units(), source.berries_units());
    }

    #[test]
    fn second_goal_preserves_the_original_uninterrupted_waiting_baseline() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_home_good(Good::BreadRoll, 30)
            .unwrap()
            .with_home_good(Good::Water, 1000)
            .unwrap();
        let eaten = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        let waited = best_variant_after(&eaten, Effect::Wait).unwrap().unwrap();
        let mut replay = eaten.clone();
        for action in &waited.actions {
            replay = replay.perform(*action, 0).unwrap().unwrap();
        }
        assert_eq!(waited.average().unwrap(), replay.average().unwrap());
        assert_eq!(waited.waiting, replay.waiting);
        assert_eq!(waited.waiting.home_drink_units(), source.home_drink_units());
        assert!(waited.goal_average().unwrap() > 0.0);
        let reset = best_variant(&eaten.citizen, Effect::Wait, eaten.cooldowns, 0)
            .unwrap()
            .unwrap();
        assert_eq!(reset.average().unwrap(), 0.0);
        assert_ne!(waited.waiting.hunger(), reset.waiting.hunger());
    }

    #[test]
    fn waiting_keeps_position_and_advances_needs_before_the_second_goal() {
        let mut citizen = Citizen::new(0.0)
            .unwrap()
            .with_thirst(0.0)
            .unwrap()
            .with_home_good(Good::BreadRoll, 30)
            .unwrap()
            .with_home_good(Good::Water, 1200)
            .unwrap();
        citizen.tiredness = 0.0;
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let waited = variants_after(&initial, Effect::Wait)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(waited.actions, [CitizenAction::Wait; 4]);
        assert_eq!(waited.elapsed_ms, WAIT_GOAL_DURATION_MS);
        assert_eq!(waited.citizen.position(), citizen.position());
        for goal in [
            Effect::ReduceHunger,
            Effect::ReduceThirst,
            Effect::ReduceTiredness,
        ] {
            assert!(variants_after(&initial, goal).unwrap().is_empty());
            assert!(
                !variants_after(&waited, goal).unwrap().is_empty(),
                "{goal:?}"
            );
        }
        let second_wait = variants_after(&waited, Effect::Wait)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(second_wait.elapsed_ms, WAIT_GOAL_DURATION_MS);
        assert_eq!(second_wait.actions.len(), 4);
    }

    #[test]
    fn standalone_drinking_requires_positive_thirst() {
        for (thirst, eligible) in [(-50.0, false), (0.0, false), (0.001, true)] {
            let citizen = Citizen::new(-50.0)
                .unwrap()
                .with_thirst(thirst)
                .unwrap()
                .with_home_good(Good::Water, 398)
                .unwrap();
            let variants = variants_after(
                &Prediction::new(&citizen, Cooldowns::default()),
                Effect::ReduceThirst,
            )
            .unwrap();
            assert_eq!(!variants.is_empty(), eligible, "thirst {thirst}");
        }
    }

    #[test]
    fn water_replenishment_triggers_below_four_hundred_per_member() {
        for (water, eligible) in [(399, true), (400, false), (401, false)] {
            let citizen = Citizen::new(-50.0)
                .unwrap()
                .with_thirst(-99.789)
                .unwrap()
                .with_home_good(Good::BreadRoll, 30)
                .unwrap()
                .with_home_good(Good::Fish, 3)
                .unwrap()
                .with_home_good(Good::Cabbage, 3)
                .unwrap()
                .with_home_good(Good::Water, water)
                .unwrap();
            let variants = variants_after(
                &Prediction::new(&citizen, Cooldowns::default()),
                Effect::ReplenishReserves,
            )
            .unwrap();
            assert_eq!(
                variants.iter().any(|p| p
                    .actions
                    .iter()
                    .any(|a| a.production_recipe() == Some(crate::production::Recipe::FetchWater))),
                eligible,
                "water {water}"
            );
            if eligible {
                assert!(
                    variants
                        .iter()
                        .all(|p| { reserve_units(&p.citizen, Good::Water) >= 400 })
                );
            }
        }
    }

    #[test]
    fn meals_still_drink_when_standalone_drinking_is_ineligible() {
        let citizen = Citizen::new(50.0)
            .unwrap()
            .with_thirst(-75.0)
            .unwrap()
            .with_home_good(Good::BreadRoll, 1)
            .unwrap()
            .with_home_good(Good::Water, 200)
            .unwrap();
        let variants = variants_after(
            &Prediction::new(&citizen, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(variants.iter().any(|p| {
            p.actions == [CitizenAction::Eat]
                && reserve_units(&p.citizen, Good::Water) == 100
                && p.citizen.thirst() < -99.0
        }));
    }

    #[test]
    fn blocked_purchases_keep_alternatives_and_maximum_affordable_requests() {
        use crate::marketplace::ShoppingList;
        let citizen = Citizen::new(0.0).unwrap().with_coins(1000).unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        for (good, units) in [
            (Good::BreadRoll, 10),
            (Good::BreadRoll, 5),
            (Good::BerryPie, 2),
        ] {
            assert!(
                shopping_trip(&initial, ShoppingList::single(good, units), 0, 0)
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(initial.blocked_requests.borrow().units(Good::BreadRoll), 10);
        assert_eq!(initial.blocked_requests.borrow().units(Good::BerryPie), 2);
        assert_eq!(
            citizen.market().requested(citizen.id()),
            ShoppingList::default()
        );
        let broke = Prediction::new(&citizen.with_coins(0).unwrap(), Cooldowns::default());
        record_blocked_purchase(&broke, ShoppingList::single(Good::FlaxGarment, 1)).unwrap();
        assert_eq!(*broke.blocked_requests.borrow(), ShoppingList::default());
    }

    #[test]
    fn blocked_purchases_include_present_stock_and_respect_own_food() {
        use crate::marketplace::ShoppingList;
        let citizen = Citizen::new(0.0).unwrap().with_coins(1000).unwrap();
        let mut market = citizen.market().clone();
        let place = citizen
            .map()
            .public_place(crate::locations::Location::Market);
        market
            .list(
                crate::AgentId(uuid::Uuid::new_v4()),
                place,
                Good::BreadRoll,
                1,
            )
            .unwrap();
        let initial = Prediction::new(&citizen.with_market(market.clone()), Cooldowns::default());
        record_blocked_purchase(&initial, ShoppingList::single(Good::BreadRoll, 10)).unwrap();
        assert_eq!(initial.blocked_requests.borrow().units(Good::BreadRoll), 10);
        market
            .list(citizen.id(), place, Good::Berries, 3000)
            .unwrap();
        let owned = Prediction::new(&citizen.with_market(market), Cooldowns::default());
        order_variants(&owned, CitizenAction::BuyFood(Good::BerryPie), 100.0, 0, 0).unwrap();
        assert_eq!(*owned.blocked_requests.borrow(), ShoppingList::default());
    }

    #[test]
    fn blocked_demand_keeps_raw_quantities_for_cheaper_candidate_quotes() {
        let citizen = Citizen::new(0.0).unwrap().with_coins(100).unwrap();
        let place = citizen
            .map()
            .public_place(crate::locations::Location::Market);
        let mut market = citizen.market().clone();
        market.prices = market.prices.with_price(Good::FlaxGarment, 1000.0).unwrap();
        let list = crate::marketplace::ShoppingList::single(Good::FlaxGarment, 1);
        let empty = Prediction::new(
            &citizen.clone().with_market(market.clone()),
            Cooldowns::default(),
        );
        shopping_trip(&empty, list, 0, 0).unwrap();
        assert_eq!(empty.blocked_requests.borrow().units(Good::FlaxGarment), 1);
        market
            .list_at_quote(
                crate::AgentId(uuid::Uuid::new_v4()),
                place,
                Good::FlaxGarment,
                1,
                1000.0,
            )
            .unwrap();
        let offered = Prediction::new(&citizen.with_market(market), Cooldowns::default());
        shopping_trip(&offered, list, 0, 0).unwrap();
        assert_eq!(
            offered.blocked_requests.borrow().units(Good::FlaxGarment),
            1
        );
    }

    #[test]
    fn shopping_routes_skip_unaffordable_intermediate_stock() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 10.0, y: 0.0 },
            Position { x: 20.0, y: 0.0 },
            Position::default(),
        )
        .unwrap();
        let c = map.public_place(Location::River);
        let a = map.public_place(Location::Market);
        let b = map.public_place(Location::Forest);
        let buyer = Citizen::new(0.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap()
            .with_coins(2)
            .unwrap();
        let mut market = buyer.market().clone();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        for (place, quote) in [(a, 1.0), (b, 2.0), (c, 1.0)] {
            market
                .list_at_quote(seller, place, Good::BreadRoll, 1, quote)
                .unwrap();
        }
        let buyer = buyer.with_market(market);
        let variants = shopping_trip(
            &Prediction::new(&buyer, Cooldowns::default()),
            crate::marketplace::ShoppingList::single(Good::BreadRoll, 2),
            0,
            0,
        )
        .unwrap();
        assert!(
            variants
                .iter()
                .any(|trip| trip.citizen.available_units(Good::BreadRoll) == 2
                    && trip.citizen.coins() == 0)
        );
        assert!(
            variants
                .iter()
                .all(|trip| !trip.actions.contains(&CitizenAction::Travel(b)))
        );
    }

    #[test]
    fn shopping_without_any_listings_still_records_affordable_demand() {
        let citizen = Citizen::new(0.0).unwrap().with_coins(1000).unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let list = crate::marketplace::ShoppingList::single(Good::FlaxGarment, 1);
        assert!(shopping_trip(&initial, list, 0, 0).unwrap().is_empty());
        assert_eq!(
            initial.blocked_requests.borrow().units(Good::FlaxGarment),
            1
        );
    }

    #[test]
    fn food_demand_uses_the_goal_target_without_suppressing_recipe_ingredients() {
        use crate::marketplace::ShoppingList;
        let citizen = Citizen::new(0.0).unwrap().with_coins(1000).unwrap();
        let place = citizen.selling_place();
        let mut market = citizen.market().clone();
        market
            .list(citizen.id(), place, Good::BreadRoll, 5)
            .unwrap();
        let initial = Prediction::new(&citizen.with_market(market), Cooldowns::default());
        order_variants(&initial, CitizenAction::BuyFood(Good::BerryPie), 50.0, 0, 0).unwrap();
        assert_eq!(*initial.blocked_requests.borrow(), ShoppingList::default());
        prepare_inputs(&initial, &ShoppingList::single(Good::Berries, 100), 0, 0).unwrap();
        assert_eq!(initial.blocked_requests.borrow().units(Good::Berries), 100);
    }

    #[test]
    fn remote_owned_listings_supply_clothing_and_food_without_blocked_demand() {
        use crate::marketplace::ShoppingList;
        let (citizen, mill, _) = workplace_shopper();
        assert_ne!(mill, citizen.selling_place());
        let mut market = citizen.market().clone();
        market
            .list(citizen.id(), mill, Good::FlaxGarment, 1)
            .unwrap();
        market
            .list(citizen.id(), mill, Good::Berries, 3000)
            .unwrap();
        let initial = Prediction::new(&citizen.with_market(market), Cooldowns::default());
        variants_after(&initial, Effect::ReduceClothingNeed).unwrap();
        assert_eq!(
            initial.blocked_requests.borrow().units(Good::FlaxGarment),
            0
        );
        let demand = food_purchase_demand(&initial, 100.0, Good::BerryPie);
        assert_eq!(demand, ShoppingList::default());
        let late = Prediction {
            elapsed_ms: GOAL_HORIZON_MS,
            ..initial.clone()
        };
        let previous = *late.blocked_requests.borrow();
        shopping_trip(&late, ShoppingList::single(Good::Cloth, 100), 0, 0).unwrap();
        assert_eq!(*late.blocked_requests.borrow(), previous);
    }

    fn workplace_shopper() -> (
        Citizen,
        crate::locations::PlaceId,
        crate::locations::PlaceId,
    ) {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position::default(),
            Position::default(),
            Position { x: 400.0, y: 0.0 },
        )
        .unwrap();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        let (map, mill) = map
            .with_place(
                Location::Mill,
                Position { x: 100.0, y: 0.0 },
                Some(seller),
                "Mill",
            )
            .unwrap();
        let market_place = map.public_place(Location::Market);
        let buyer = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_map(map)
            .unwrap()
            .with_coins(2000)
            .unwrap();
        let mut market = buyer.market().clone();
        market.list(seller, mill, Good::Flour, 100).unwrap();
        market.list(seller, mill, Good::BreadRoll, 5).unwrap();
        market.list(seller, market_place, Good::Flour, 100).unwrap();
        market.list(seller, market_place, Good::Wood, 25).unwrap();
        market
            .list(seller, market_place, Good::BerryPie, 1)
            .unwrap();
        (buyer.with_market(market), mill, market_place)
    }

    #[test]
    fn shopping_withdraws_all_owned_sites_before_buying_only_the_shortfall() {
        use crate::marketplace::ShoppingList;
        let (buyer, mill, market_place) = workplace_shopper();
        let mut market = buyer.market().clone();
        for (place, units) in [(mill, 20), (mill, 30), (market_place, 25)] {
            market.list(buyer.id(), place, Good::Flour, units).unwrap();
        }
        let buyer = buyer.with_market(market);
        assert_eq!(
            buyer
                .listed_sources(Good::Flour)
                .iter()
                .map(|(_, n)| n)
                .sum::<u64>(),
            75
        );
        let initial = Prediction::new(&buyer, Cooldowns::default());
        let trips = shopping_trip(&initial, ShoppingList::single(Good::Flour, 100), 0, 0).unwrap();
        assert!(!trips.is_empty());
        for trip in trips {
            assert_eq!(trip.citizen.available_units(Good::Flour), 100);
            assert_eq!(trip.citizen.withdrawable_units(Good::Flour), 0);
            assert!(trip.actions.contains(&CitizenAction::Travel(mill)));
            let bought: u64 = trip
                .actions
                .iter()
                .filter_map(|action| match action {
                    CitizenAction::BuyAt { list, .. } => Some(list.units(Good::Flour)),
                    _ => None,
                })
                .sum();
            assert_eq!(bought, 25);
        }
    }

    #[test]
    fn remote_owned_provisions_are_retrieved_without_coins_or_trade() {
        use crate::marketplace::ShoppingList;
        for (good, units) in [
            (Good::Flour, 100),
            (Good::Water, 200),
            (Good::BreadRoll, 10),
            (Good::FlaxGarment, 1),
        ] {
            let (buyer, mill, _) = workplace_shopper();
            let mut market = buyer.market().clone();
            market.list(buyer.id(), mill, good, units).unwrap();
            let buyer = buyer.with_coins(0).unwrap().with_market(market);
            let initial = Prediction::new(&buyer, Cooldowns::default());
            let trips = shopping_trip(&initial, ShoppingList::single(good, units), 0, 0).unwrap();
            assert!(!trips.is_empty(), "{good:?}");
            for trip in trips {
                assert_eq!(trip.citizen.available_units(good), units);
                assert_eq!(trip.citizen.coins(), 0);
                assert!(!trip.actions.iter().any(|action| matches!(
                    action,
                    CitizenAction::BuyAt { .. } | CitizenAction::Buy(_)
                )));
                assert_eq!(trip.citizen.market().trades().len(), 0);
            }
        }
    }

    #[test]
    fn shopping_trip_breaks_price_ties_by_distance_and_keeps_one_cooldown() {
        let (buyer, mill, market) = workplace_shopper();
        let initial = Prediction::new(&buyer, Cooldowns::default());
        let list =
            crate::marketplace::ShoppingList::new([(Good::Flour, 100), (Good::Wood, 25)]).unwrap();
        let trip = shopping_trip(&initial, list, 0, 0)
            .unwrap()
            .into_iter()
            .find(|trip| trip.actions.first() == Some(&CitizenAction::Travel(mill)))
            .unwrap();
        assert_eq!(
            trip.actions
                .iter()
                .copied()
                .map(unbounded_action)
                .collect::<Vec<_>>(),
            [
                CitizenAction::Travel(mill),
                CitizenAction::BuyAt {
                    place: mill,
                    list: crate::marketplace::ShoppingList::single(Good::Flour, 100)
                },
                CitizenAction::Travel(market),
                CitizenAction::BuyAt {
                    place: market,
                    list: crate::marketplace::ShoppingList::single(Good::Wood, 25)
                },
            ]
        );
        assert_eq!(trip.elapsed_ms, 240_000 + 2 * TRADE_DURATION_MS);
        assert_eq!(trip.cooldowns.buy, TRADE_PLAN_COOLDOWN_MS);
        assert_eq!(
            trip.citizen
                .market()
                .available_units_at(buyer.id(), market, Good::Flour),
            100
        );
        assert!(shopping_trip(&trip, list, 0, 0).unwrap().is_empty());
        assert_eq!(buyer.units(Good::Flour), 0);
        let mut blocked = initial;
        blocked.cooldowns.buy = 1;
        assert!(shopping_trip(&blocked, list, 0, 0).unwrap().is_empty());
    }

    #[test]
    fn shopping_trip_counts_small_purchases_above_reserved_inventory() {
        let (mut buyer, _, _) = workplace_shopper();
        buyer.inventory.insert(Good::Flour, 300);
        buyer.tax_reserved.insert(Good::Flour, 300);
        let initial = Prediction::new(&buyer, Cooldowns::default());
        let variants = shopping_trip(
            &initial,
            crate::marketplace::ShoppingList::single(Good::Flour, 100),
            0,
            0,
        )
        .unwrap();
        let trip = variants
            .first()
            .expect("usable purchase is smaller than reserved stock");
        assert_eq!(trip.citizen.available_units(Good::Flour), 100);
        assert_eq!(trip.citizen.units(Good::Flour), 400);
        assert_eq!(trip.citizen.tax_reserved_units(Good::Flour), 300);
    }

    #[test]
    fn shopping_trip_food_suppliers_fill_mixed_meals_from_multiple_places() {
        let (buyer, mill, market) = workplace_shopper();
        let initial = Prediction::new(&buyer, Cooldowns::default());
        let variants = satisfy(
            &initial,
            Requirement {
                resource: Effect::Food,
                amount: 100.0,
                coins: None,
            },
            0,
            0,
        )
        .unwrap();
        let trip = variants
            .iter()
            .find(|trip| {
                trip.citizen.available_units(Good::BreadRoll) == 5
                    && trip.citizen.available_units(Good::BerryPie) == 1
                    && trip.actions.len() == 4
                    && trip.actions.first() == Some(&CitizenAction::Travel(mill))
            })
            .unwrap();
        assert_eq!(
            trip.citizen.available_food_nutrition(),
            5.0 * Good::BreadRoll.nutrition_per_unit().unwrap()
                + Good::BerryPie.nutrition_per_unit().unwrap()
        );
        assert_eq!(
            trip.actions
                .iter()
                .filter(|action| matches!(action, CitizenAction::BuyAt { .. }))
                .count(),
            2
        );
        assert_eq!(trip.actions[0], CitizenAction::Travel(mill));
        assert_eq!(trip.citizen.position(), buyer.map().position(market));
    }

    #[test]
    fn shopping_trip_reserves_time_for_later_prerequisites_and_rejects_unfilled_baskets() {
        let (buyer, _, _) = workplace_shopper();
        let mut initial = Prediction::new(&buyer, Cooldowns::default());
        let list =
            crate::marketplace::ShoppingList::new([(Good::Flour, 100), (Good::Wood, 25)]).unwrap();
        let duration = 240_000 + 2 * TRADE_DURATION_MS;
        initial.elapsed_ms = GOAL_HORIZON_MS - duration;
        assert!(!shopping_trip(&initial, list, 0, 0).unwrap().is_empty());
        assert!(
            shopping_trip(&initial, list, duration + 1, 0)
                .unwrap()
                .is_empty()
        );
        let impossible = crate::marketplace::ShoppingList::single(Good::Wood, 26);
        assert!(
            shopping_trip(
                &Prediction::new(&buyer, Cooldowns::default()),
                impossible,
                0,
                0
            )
            .unwrap()
            .is_empty()
        );
    }

    fn unbounded_action(action: CitizenAction) -> CitizenAction {
        match action {
            CitizenAction::BuyAt { place, list } => CitizenAction::BuyAt {
                place,
                list: list.without_purchase_bounds(),
            },
            action => action,
        }
    }

    #[test]
    fn exhausted_ingredient_budgets_do_not_disable_input_free_work() {
        use crate::production::Recipe;
        let mut source = baker()
            .with_berries(0)
            .unwrap()
            .with_good(Good::BreadRoll, 1)
            .unwrap();
        source.refresh_production_targets(true).unwrap();
        let target = source.production_targets.as_mut().unwrap();
        for _ in 0..24 {
            target.complete(Recipe::Forage);
        }
        let initial = Prediction::new(&source, Cooldowns::default());
        let forage = CitizenAction::Produce(Recipe::Forage);
        assert!(
            production_prefixes(initial.clone(), 0, &mut [])
                .unwrap()
                .iter()
                .any(|v| v
                    .actions
                    .iter()
                    .any(|action| action.production_recipe() == Some(Recipe::Forage)))
        );
        assert!(
            variants_after(&initial, Effect::ReduceHunger)
                .unwrap()
                .is_empty()
        );
        assert!(
            variants_after(&initial, Effect::ReplenishReserves)
                .unwrap()
                .iter()
                .any(|v| v.actions.contains(&forage)
                    && matches!(v.actions.last(), Some(CitizenAction::DepositStored { .. })))
        );
        let ingredients = crate::marketplace::ShoppingList::single(Good::Berries, 50);
        assert!(
            prepare_inputs(&initial, &ingredients, 0, 0)
                .unwrap()
                .iter()
                .any(|v| v.actions.contains(&forage) && v.citizen.berries_units() >= 50)
        );
    }

    #[test]
    fn forage_production_includes_travel_and_keeps_half_hour_primitives() {
        use crate::locations::{Map, Position};
        for (travel_ms, counts) in [(0, vec![4]), (5 * 60_000, vec![4])] {
            let map = Map::new(
                Position {
                    x: travel_ms as f64 / crate::locations::WALK_MS_PER_METRE,
                    y: 0.0,
                },
                Position::default(),
                Position::default(),
            )
            .unwrap();
            let citizen = Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_map(map)
                .unwrap()
                .with_good(Good::Water, 10_000)
                .unwrap();
            let initial = Prediction::new(&citizen, Cooldowns::default());
            let variants = variants_from(initial.clone(), Effect::Production, 0).unwrap();
            assert_eq!(variants.len(), 1);
            for (candidate, count) in variants.iter().zip(counts) {
                assert_eq!(
                    candidate
                        .actions
                        .iter()
                        .filter(
                            |&&a| a.production_recipe() == Some(crate::production::Recipe::Forage)
                        )
                        .count(),
                    count
                );
                assert_eq!(
                    candidate.elapsed_ms,
                    travel_ms + count as u64 * ACTION_DURATION_MS
                );
                assert!(
                    candidate
                        .actions
                        .iter()
                        .zip(&candidate.action_durations_ms)
                        .all(|(action, duration)| action.production_recipe()
                            != Some(crate::production::Recipe::Forage)
                            || *duration == ACTION_DURATION_MS)
                );
            }
            let suppliers = order_variants(
                &initial,
                CitizenAction::Produce(crate::production::Recipe::Forage),
                0.0,
                0,
                0,
            )
            .unwrap();
            assert_eq!(suppliers.len(), 8);
            assert_eq!(suppliers[0].elapsed_ms, travel_ms + ACTION_DURATION_MS);
        }
    }

    fn bread_only(mut worker: Citizen) -> Citizen {
        worker = worker
            .with_good(Good::BerryPie, 10_000)
            .unwrap()
            .with_berries(10_000)
            .unwrap();
        worker.refresh_production_targets(true).unwrap();
        worker
    }

    #[test]
    fn production_after_an_earlier_goal_keeps_the_full_work_duration() {
        let worker = bread_only(baker());
        let initial = Prediction::new(&worker, Cooldowns::default());
        let direct = variants_after(&initial, Effect::Production).unwrap();
        let mut prefix = initial;
        prefix.elapsed_ms = 90 * 60_000;
        let later = variants_after(&prefix, Effect::Production).unwrap();
        assert_eq!(direct.len(), later.len());
        for (direct, later) in direct.iter().zip(&later) {
            assert_eq!(direct.actions, later.actions);
            assert_eq!(direct.elapsed_ms, later.elapsed_ms);
            assert!(later.elapsed_ms >= PRODUCTION_DURATION_MS);
        }
    }

    #[test]
    fn skilled_production_finishes_the_first_crossing_batch() {
        use crate::production::{Recipe, Skill};
        let worker = bread_only(
            baker()
                .with_skill_practice_ms(
                    Skill::Baking,
                    Some(crate::production::MAX_SKILL_PRACTICE_MS / 2),
                )
                .unwrap(),
        );
        let initial = Prediction::new(&worker, Cooldowns::default());
        let variants = [site_candidate(
            &worker,
            worker.production_place(Recipe::BakeRolls).unwrap(),
        )];
        assert_eq!(variants.len(), 1);
        for candidate in &variants {
            let checkpoint = PRODUCTION_DURATION_MS;
            let last_duration = *candidate.action_durations_ms.last().unwrap();
            assert!(candidate.elapsed_ms >= checkpoint);
            assert!(candidate.elapsed_ms - last_duration < checkpoint);
            assert!(
                candidate
                    .action_durations_ms
                    .windows(2)
                    .all(|pair| pair[1] < pair[0])
            );
            assert!(
                candidate
                    .actions
                    .iter()
                    .all(|action| action.production_recipe() == Some(Recipe::BakeRolls))
            );
            let replay = production_order(
                &initial,
                &vec![Recipe::BakeRolls; candidate.actions.len()],
                None,
                0,
                0,
            )
            .unwrap();
            assert_eq!(candidate.action_durations_ms, replay[0].action_durations_ms);
            assert_eq!(
                candidate.citizen.available_units(Good::BreadRoll),
                replay[0].citizen.available_units(Good::BreadRoll)
            );
            assert_eq!(candidate.average().unwrap(), replay[0].average().unwrap());
        }
    }

    #[test]
    fn long_production_jobs_finish_the_first_crossing_batch() {
        use crate::production::Recipe;
        let mut worker = baker().with_good(Good::BreadRoll, 10_000).unwrap();
        worker.refresh_production_targets(true).unwrap();
        let variants = [site_candidate(
            &worker,
            worker.production_place(Recipe::BakeRolls).unwrap(),
        )];
        assert_eq!(variants.len(), 1);
        let candidate = &variants[0];
        assert!(
            candidate
                .actions
                .iter()
                .all(|a| a.production_recipe() == Some(Recipe::BakeBerryPie))
        );
        assert!(candidate.elapsed_ms >= PRODUCTION_DURATION_MS);
        assert!(
            candidate.elapsed_ms - candidate.action_durations_ms.last().unwrap()
                < PRODUCTION_DURATION_MS
        );
    }

    #[test]
    fn production_stops_at_cap_when_only_universal_gathering_remains() {
        use crate::production::{Recipe, Skill};
        let mut worker = bread_only(
            baker()
                .with_skill_practice_ms(
                    Skill::Baking,
                    Some(crate::production::MAX_SKILL_PRACTICE_MS),
                )
                .unwrap(),
        )
        .with_good(Good::Water, 10_000)
        .unwrap();
        let batches =
            crate::production::DAILY_CAPACITY_MS / Recipe::BakeRolls.duration_ms(&worker).unwrap();
        worker = worker
            .with_good(Good::BreadRoll, (batches - 1) * 10)
            .unwrap();
        worker.prices = worker.prices.with_price(Good::BreadRoll, 100.0).unwrap();
        worker.refresh_production_targets(true).unwrap();
        assert_eq!(
            worker
                .production_targets()
                .unwrap()
                .remaining_batches(Recipe::BakeRolls),
            1
        );
        let variants =
            production_prefixes(Prediction::new(&worker, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert_eq!(variants.len(), 1);
        let candidate = &variants[0];
        assert_eq!(
            candidate
                .actions
                .iter()
                .filter(|&&action| action.production_recipe() == Some(Recipe::BakeRolls))
                .count(),
            1
        );
        assert!(candidate.elapsed_ms < PRODUCTION_DURATION_MS);
        assert!(!candidate.actions.iter().any(|action| matches!(
            action.production_recipe(),
            Some(Recipe::Forage | Recipe::FetchWater)
        )));
    }

    #[test]
    fn production_selects_a_feasible_acquisition_route_and_combines_its_recipe_inputs() {
        use crate::production::Recipe;
        let worker = bread_only(buying_baker(2000));
        let initial = Prediction::new(&worker, Cooldowns::default());
        let variants = [site_candidate(
            &worker,
            worker.production_place(Recipe::BakeRolls).unwrap(),
        )];
        assert_eq!(variants.len(), 1);
        let candidate = &variants[0];
        let recipes: Vec<_> = candidate
            .actions
            .iter()
            .filter_map(|action| match action {
                CitizenAction::ProduceAt { recipe, .. } => Some(*recipe),
                _ => None,
            })
            .collect();
        assert!(recipes.contains(&Recipe::BakeRolls));
        let bread_batches = recipes
            .iter()
            .filter(|&&recipe| recipe == Recipe::BakeRolls)
            .count() as u64;
        let flour_bought: u64 = candidate
            .actions
            .iter()
            .filter_map(|action| match action {
                CitizenAction::BuyAt { list, .. } => Some(list.units(Good::Flour)),
                _ => None,
            })
            .sum();
        assert!(flour_bought > bread_batches * 100);
        assert_eq!(
            flour_bought,
            crate::production::recipe_requirements_at(
                &worker,
                Some(worker.production_place(Recipe::BakeRolls).unwrap()),
                &crate::production::projected_recipes_at(
                    &worker,
                    Some(worker.production_place(Recipe::BakeRolls).unwrap()),
                    crate::production::DAILY_CAPACITY_MS
                )
                .unwrap()
            )
            .unwrap()
            .units(Good::Flour)
        );
        let mut replay = initial;
        for &action in &candidate.actions {
            replay = replay.perform(action, 0).unwrap().unwrap();
        }
        assert_eq!(candidate.elapsed_ms, replay.elapsed_ms);
        assert_eq!(candidate.action_durations_ms, replay.action_durations_ms);
        assert_eq!(candidate.citizen.coins(), replay.citizen.coins());
    }

    #[test]
    fn production_restock_skips_covered_blocks_and_buys_a_working_day() {
        let worker = bread_only(buying_baker(2000));
        let target = worker.production_restock_requirements().unwrap();
        let initial = Prediction::new(&worker, Cooldowns::default());
        let bought = production_restock(&initial, 0).unwrap();
        for (good, units) in target.items() {
            assert!(bought.citizen.available_units(good) >= units);
        }
        assert_eq!(
            bought
                .actions
                .iter()
                .filter(|action| matches!(action, CitizenAction::BuyAt { .. }))
                .count(),
            1
        );
        let recipes =
            crate::production::projected_recipes(&worker, PRODUCTION_DURATION_MS).unwrap();
        let requirements = recipe_requirements(&worker, &recipes).unwrap();
        let mut covered = worker;
        for (good, units) in requirements.items() {
            covered = covered.with_good(good, units).unwrap();
        }
        let untouched =
            production_restock(&Prediction::new(&covered, Cooldowns::default()), 0).unwrap();
        assert!(untouched.actions.is_empty());
        assert_eq!(untouched.citizen.coins(), covered.coins());
    }

    #[test]
    fn production_restock_respects_incoming_shared_workplace_supplies() {
        let mut worker = bread_only(buying_baker(2000));
        let site = worker
            .production_place(crate::production::Recipe::BakeRolls)
            .unwrap();
        let recipes = crate::production::projected_recipes_at(
            &worker,
            Some(site),
            crate::production::DAILY_CAPACITY_MS,
        )
        .unwrap();
        let target =
            crate::production::recipe_requirements_at(&worker, Some(site), &recipes).unwrap();
        worker.workplace_supplies.insert(site, (target, target));
        worker.own_workplace_supplies.insert(site, (target, target));
        worker.incoming_supplies.insert(site, target);
        let restock = production_restock_at(
            &Prediction::new(&worker, Cooldowns::default()),
            0,
            Some(site),
        )
        .unwrap();
        assert!(restock.actions.is_empty());
        assert_eq!(restock.citizen.coins(), worker.coins());
    }

    #[test]
    fn fresh_restock_replaces_only_own_cached_target_and_deducts_incoming() {
        let mut worker = bread_only(buying_baker(2000));
        let site = worker
            .production_place(crate::production::Recipe::BakeRolls)
            .unwrap();
        let recipes = crate::production::projected_recipes_at(
            &worker,
            Some(site),
            crate::production::DAILY_CAPACITY_MS,
        )
        .unwrap();
        let fresh = crate::production::recipe_requirements_at(&worker, Some(site), &recipes)
            .unwrap()
            .units(Good::Flour);
        assert!(fresh > 0 && fresh + 25 <= 2000);
        let own = crate::marketplace::ShoppingList::single(Good::Flour, 10_000);
        let shared = crate::marketplace::ShoppingList::single(Good::Flour, 10_050);
        worker.own_workplace_supplies.insert(site, (own, own));
        worker.workplace_supplies.insert(site, (shared, shared));
        worker.incoming_supplies.insert(
            site,
            crate::marketplace::ShoppingList::single(Good::Flour, 25),
        );
        let supplied = production_restock_at(
            &Prediction::new(&worker, Cooldowns::default()),
            0,
            Some(site),
        )
        .unwrap();
        assert_eq!(
            supplied.citizen.stored_available_units(site, Good::Flour),
            fresh + 25
        );
        worker.own_workplace_supplies.clear();
        worker.designated_workplace = Some(
            worker
                .map()
                .public_place(crate::locations::Location::Forest),
        );
        let others = crate::marketplace::ShoppingList::single(Good::Flour, 50);
        worker.workplace_supplies.insert(site, (others, others));
        let alternate = production_restock_at(
            &Prediction::new(&worker, Cooldowns::default()),
            0,
            Some(site),
        )
        .unwrap();
        assert_eq!(
            alternate.citizen.stored_available_units(site, Good::Flour),
            fresh + 25
        );
    }

    #[test]
    fn committed_production_restock_keeps_prices_during_travel_and_save() {
        use crate::locations::{Location, Position};
        let mut worker = bread_only(buying_baker(2000));
        let place = worker.map().public_place(Location::Market);
        let mut map = worker.map().clone();
        map.places_mut().get_mut(&place).unwrap().position = Position { x: 100.0, y: 0.0 };
        worker = worker.with_map(map).unwrap();
        let site = worker
            .production_place(crate::production::Recipe::BakeRolls)
            .unwrap();
        let candidate = site_candidate(&worker, site);
        assert_eq!(
            candidate.actions.first(),
            Some(&CitizenAction::Travel(place))
        );
        let (buy_index, list) = candidate
            .actions
            .iter()
            .enumerate()
            .find_map(|(index, action)| match action {
                CitizenAction::BuyAt { list, .. } => Some((index, *list)),
                _ => None,
            })
            .unwrap();
        let ceiling = list.quote_ceiling(Good::Flour);
        let original = worker
            .market
            .lowest_purchase_quote(worker.market_party(), Good::Flour, &worker.map())
            .unwrap();
        assert!(ceiling > original);
        let travelling = crate::planning::executing_fixture(&worker, candidate.actions.clone());
        let saved = ron::to_string(&travelling).unwrap();
        let loaded: Citizen = ron::from_str(&saved).unwrap();
        assert_eq!(
            loaded.active_plan().unwrap().plan().actions(),
            candidate.actions.as_slice()
        );
        for (quote, expected) in [
            ((original + ceiling) / 2.0, list.units(Good::Flour)),
            (ceiling * 1.01, 0),
        ] {
            let mut market = crate::marketplace::Market::default();
            let seller = crate::AgentId(uuid::Uuid::new_v4());
            for good in Good::ALL {
                if list.units(good) > 0 {
                    market
                        .list_at_quote(
                            seller,
                            place,
                            good,
                            list.units(good),
                            if good == Good::Flour {
                                quote
                            } else {
                                list.quote_ceiling(good)
                            },
                        )
                        .unwrap();
                }
            }
            let changed = loaded.with_market(market);
            let elapsed: u64 = candidate.action_durations_ms[..=buy_index].iter().sum();
            let bought = changed.advance(elapsed).unwrap();
            assert_eq!(bought.carried_available_units(Good::Flour), expected);
            assert_eq!(
                bought.active_plan().unwrap().plan().actions(),
                candidate.actions.as_slice()
            );
        }
    }

    #[test]
    fn production_shopping_keeps_ceilings_across_stops_and_refreshes_next_plan() {
        use crate::locations::Location;
        let mut worker = bread_only(buying_baker(50));
        let market_place = worker.map().public_place(Location::Market);
        let river = worker.map().public_place(Location::River);
        worker.market = crate::marketplace::Market::default();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        worker
            .market
            .list_at_quote(seller, market_place, Good::Flour, 50, 10.0)
            .unwrap();
        worker
            .market
            .list_at_quote(seller, river, Good::Flour, 50, 10.5)
            .unwrap();
        worker
            .market
            .list_at_quote(seller, river, Good::Flour, 50, 12.0)
            .unwrap();
        let mut initial = Prediction::new(&worker, Cooldowns::default());
        let mut ceilings = [0.0; Good::COUNT];
        ceilings[Good::Flour as usize] = 11.0;
        initial.production_quote_ceilings = Some(ceilings);
        let bought = shopping_trip_from_mode(
            &initial,
            crate::marketplace::ShoppingList::single(Good::Flour, 150),
            0,
            0,
            true,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(bought.citizen.available_units(Good::Flour), 100);
        let purchases: Vec<_> = bought
            .actions
            .iter()
            .filter_map(|action| match action {
                CitizenAction::BuyAt { list, .. } => Some(*list),
                _ => None,
            })
            .collect();
        assert_eq!(purchases.len(), 2);
        assert!(
            purchases
                .iter()
                .all(|list| list.quote_ceiling(Good::Flour) == 11.0)
        );
        let mut changed = bought.clone();
        changed.cooldowns.buy = 0;
        changed
            .citizen
            .market
            .list_at_quote(seller, river, Good::Flax, 100, 1.0)
            .unwrap();
        assert!(
            shopping_trip_from_mode(
                &changed,
                crate::marketplace::ShoppingList::single(Good::Flax, 100),
                0,
                0,
                true
            )
            .unwrap()
            .is_empty()
        );
        let mut next = Prediction::new(&changed.citizen, Cooldowns::default());
        let mut refreshed = ceilings;
        refreshed[Good::Flax as usize] = next
            .citizen
            .market
            .lowest_purchase_quote(next.citizen.market_party(), Good::Flax, &next.citizen.map())
            .unwrap()
            * 1.1;
        next.production_quote_ceilings = Some(refreshed);
        let acquired = shopping_trip_from_mode(
            &next,
            crate::marketplace::ShoppingList::single(Good::Flax, 100),
            0,
            0,
            true,
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(acquired.citizen.available_units(Good::Flax), 100);
    }

    #[test]
    fn production_restock_keeps_inputs_below_one_complete_batch() {
        let worker = bread_only(buying_baker(50));
        let bought =
            production_restock(&Prediction::new(&worker, Cooldowns::default()), 0).unwrap();
        assert_eq!(bought.citizen.available_units(Good::Flour), 50);
        assert!(
            bought
                .actions
                .iter()
                .any(|action| matches!(action, CitizenAction::BuyAt { .. }))
        );
        let site = worker
            .production_place(crate::production::Recipe::BakeRolls)
            .unwrap();
        let variants = production_at_site(
            Prediction::new(&worker, Cooldowns::default()),
            site,
            0,
            &mut [],
            None,
        )
        .unwrap();
        assert!(!variants.is_empty());
        assert!(variants.iter().any(|variant| {
            variant
                .actions
                .iter()
                .any(|action| matches!(action, CitizenAction::BuyAt { .. }))
        }));
        assert!(
            variants
                .iter()
                .any(|variant| variant.citizen.stored_available_units(site, Good::Flour) == 50)
        );
    }

    #[test]
    fn production_restock_keeps_a_complete_prefix_when_stock_is_limited() {
        let worker = bread_only(buying_baker(200));
        let bought =
            production_restock(&Prediction::new(&worker, Cooldowns::default()), 0).unwrap();
        assert_eq!(bought.citizen.available_units(Good::Flour), 200);
        assert!(
            bought
                .actions
                .iter()
                .all(|action| !matches!(action, CitizenAction::Produce(_)))
        );
        let candidate = site_candidate(
            &worker,
            worker
                .production_place(crate::production::Recipe::BakeRolls)
                .unwrap(),
        );
        assert!(
            candidate
                .actions
                .iter()
                .any(|action| action.production_recipe()
                    == Some(crate::production::Recipe::BakeRolls))
        );
    }

    #[test]
    fn fully_stored_production_inputs_are_collected_without_market_purchases() {
        use crate::storage::{GoodsOwner, StockKey};
        let mut worker = bread_only(buying_baker(2000)).with_coins(0).unwrap();
        for (good, units) in [(Good::Flour, 2000), (Good::Wood, 2000), (Good::Water, 2000)] {
            worker.set_inventory_units(good, 0);
            let key = StockKey {
                place: worker.map.public_place(crate::locations::Location::Market),
                owner: GoodsOwner::Agent(worker.id()),
                good,
            };
            worker
                .storage
                .set_units(key, units, &worker.map, &worker.market)
                .unwrap();
        }
        let site = worker
            .production_place(crate::production::Recipe::BakeRolls)
            .unwrap();
        let candidate = site_candidate(&worker, site);
        assert!(
            candidate
                .actions
                .iter()
                .any(|action| matches!(action, CitizenAction::CollectStored { .. }))
        );
        assert!(
            candidate
                .actions
                .iter()
                .any(|action| action.production_recipe()
                    == Some(crate::production::Recipe::BakeRolls))
        );
        assert!(
            candidate.actions.iter().all(|action| !matches!(
                action,
                CitizenAction::Buy(..) | CitizenAction::BuyAt { .. }
            ))
        );
    }

    #[test]
    fn baker_retrieves_household_mill_flour_before_using_imports() {
        use crate::locations::{Location, Position};
        let mut worker = bread_only(buying_baker(2000));
        worker.designated_workplace = Some(
            worker
                .production_place(crate::production::Recipe::BakeRolls)
                .unwrap(),
        );
        let household = crate::AgentId(uuid::Uuid::new_v4());
        worker.household = Some(crate::households::HouseholdContext {
            id: household,
            members: vec![worker.id()],
            allegiance: 0.0,
            other_carried: [0; Good::COUNT],
            other_consumption_value: 0.0,
            other_clothing_replacements: 0,
        });
        let (map, mill) = worker
            .map()
            .with_place(
                Location::Mill,
                Position { x: 30.0, y: 0.0 },
                Some(worker.id()),
                "Household mill",
            )
            .unwrap();
        worker.map = map;
        worker
            .market
            .list(worker.market_party(), mill, Good::Flour, 3060)
            .unwrap();
        let site = worker
            .production_place(crate::production::Recipe::BakeRolls)
            .unwrap();
        let candidate = site_candidate(&worker, site);
        assert!(candidate.actions.contains(&CitizenAction::Travel(mill)));
        assert!(candidate.actions.iter().any(|action| matches!(action, CitizenAction::WithdrawAt { place, good: Good::Flour, .. } if *place == mill)));
        assert!(
            candidate
                .actions
                .iter()
                .any(|action| action.production_recipe()
                    == Some(crate::production::Recipe::BakeRolls))
        );
        assert!(!candidate.actions.iter().any(|action| matches!(action, CitizenAction::BuyAt { list, .. } if list.units(Good::Flour) > 0)));
    }

    #[test]
    fn production_restock_collects_stored_inputs_before_buying() {
        use crate::storage::{GoodsOwner, StockKey};
        let mut worker = bread_only(buying_baker(2000));
        let place = worker.map.public_place(crate::locations::Location::Market);
        let key = StockKey {
            place,
            owner: GoodsOwner::Agent(worker.id()),
            good: Good::Flour,
        };
        worker
            .storage
            .set_units(key, 100, &worker.map, &worker.market)
            .unwrap();
        let snapshot = worker.clone();
        let stocked =
            production_restock(&Prediction::new(&worker, Cooldowns::default()), 0).unwrap();
        assert!(stocked.actions.iter().any(|action| matches!(action, CitizenAction::CollectStored { place: source, list } if *source == place && list.units(Good::Flour) == 100)));
        assert_eq!(
            stocked.citizen.stored_available_units(place, Good::Flour),
            0
        );
        assert!(stocked.citizen.available_units(Good::Flour) >= 100);
        assert_eq!(worker, snapshot);
    }

    #[test]
    fn production_restock_withdraws_owned_inputs_before_spending_a_limited_budget() {
        let mut worker = bread_only(buying_baker(2000));
        let market_place = worker.selling_place();
        let mut market = worker.market().clone();
        market
            .list(worker.id(), market_place, Good::Flour, 100)
            .unwrap();
        worker = worker.with_market(market);
        let recipes =
            crate::production::projected_recipes(&worker, PRODUCTION_DURATION_MS).unwrap();
        let requirements = recipe_requirements(&worker, &recipes).unwrap();
        let demand =
            crate::marketplace::ShoppingList::new(requirements.items().map(|(good, units)| {
                (good, units.saturating_sub(worker.usable_owned_units(good)))
            }))
            .unwrap();
        let coins = worker
            .market()
            .estimated_purchase_cost_from(worker.id(), &worker.map(), worker.position(), demand)
            .unwrap();
        worker = worker.with_coins(coins).unwrap();
        let bought =
            production_restock(&Prediction::new(&worker, Cooldowns::default()), 0).unwrap();
        assert!(matches!(
            bought.actions.first(),
            Some(CitizenAction::Withdraw(Good::Flour, 100))
        ));
        assert!(bought.citizen.available_units(Good::Flour) > 100);
        assert!(bought.citizen.coins() >= 0);
        assert!(
            bought
                .actions
                .iter()
                .any(|action| matches!(action, CitizenAction::BuyAt { .. }))
        );
    }

    #[test]
    fn clothing_goal_equips_carried_listed_and_bought_garments() {
        for source in 0..3 {
            let mut citizen = Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_coins(1000)
                .unwrap();
            if source == 0 {
                citizen = citizen.with_good(Good::FlaxGarment, 1).unwrap();
            } else {
                let mut market = citizen.market().clone();
                let seller = if source == 1 {
                    citizen.id()
                } else {
                    crate::AgentId(uuid::Uuid::new_v4())
                };
                market
                    .list(
                        seller,
                        citizen
                            .map()
                            .public_place(crate::locations::Location::Market),
                        Good::FlaxGarment,
                        1,
                    )
                    .unwrap();
                citizen = citizen.with_market(market);
            }
            let snapshot = citizen.clone();
            let variants = variants_after(
                &Prediction::new(&citizen, Cooldowns::default()),
                Effect::ReduceClothingNeed,
            )
            .unwrap();
            assert!(!variants.is_empty());
            for variant in variants {
                assert_eq!(variant.actions.last(), Some(&CitizenAction::EquipClothing));
                assert_eq!(variant.citizen.garment_condition(), Some(1.0));
                assert_eq!(variant.citizen.available_units(Good::FlaxGarment), 0);
                assert!(variant.citizen.clothing_need() < citizen.clothing_need());
            }
            assert_eq!(citizen, snapshot);
        }
    }

    #[test]
    fn clothing_goal_does_not_replace_a_fresh_garment_or_invent_supply() {
        for condition in [None, Some(1.0)] {
            let citizen = Citizen::new(0.0)
                .unwrap()
                .with_garment_condition(condition)
                .unwrap();
            assert!(
                variants_after(
                    &Prediction::new(&citizen, Cooldowns::default()),
                    Effect::ReduceClothingNeed,
                )
                .unwrap()
                .is_empty()
            );
        }
    }

    #[test]
    fn clothing_chain_requirements_reuse_upstream_outputs() {
        use crate::production::Recipe;
        let mut recipes = vec![Recipe::MakeClothingBlock; 8];
        recipes.push(Recipe::AssembleGarment);
        let requirements = recipe_requirements(&Citizen::new(0.0).unwrap(), &recipes).unwrap();
        assert_eq!(requirements.units(Good::Cloth), 200);
        assert_eq!(requirements.units(Good::FlaxBlock), 0);
    }

    #[test]
    fn tailor_prefixes_keep_blocks_and_allow_downstream_assembly() {
        use crate::{AgentKind, StartingRole, Universe, production::Recipe};
        let citizen = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Tailor)
            .with_coins(1000)
            .unwrap()
            .with_good(Good::Cloth, 200)
            .unwrap()
            .with_good(Good::FlaxBlock, 7)
            .unwrap();
        let (world, id) = Universe::with_map(citizen.map())
            .with_prices(
                crate::marketplace::Prices::default()
                    .with_price(Good::Cloth, 120.0)
                    .unwrap()
                    .with_price(Good::FlaxBlock, 11.0)
                    .unwrap()
                    .with_price(Good::FlaxGarment, 96.0)
                    .unwrap(),
            )
            .with_citizen("Tailor", citizen)
            .unwrap();
        let world = world
            .with_property(id, Recipe::AssembleGarment.location())
            .unwrap()
            .0;
        let AgentKind::Citizen(mut citizen) = world.agents()[&id].kind.clone();
        citizen.refresh_production_targets(true).unwrap();
        let variants =
            production_prefixes(Prediction::new(&citizen, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert!(!variants.is_empty());
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let progress = production_order(&initial, &[Recipe::MakeClothingBlock], None, 0, 0)
            .unwrap()
            .remove(0);
        let clothing = variants_after(
            &Prediction::new(&progress.citizen, Cooldowns::default()),
            Effect::ReduceClothingNeed,
        )
        .unwrap();
        assert!(clothing.iter().any(|variant| {
            variant
                .actions
                .contains(&CitizenAction::Produce(Recipe::AssembleGarment))
                && variant.actions.last() == Some(&CitizenAction::EquipClothing)
        }));
    }

    fn hungry(berries: u64) -> Citizen {
        let citizen = Citizen::with_needs(100.0, -100.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(100.0).unwrap())
            .with_berries(berries)
            .unwrap();
        let mut market = citizen.market().clone();
        market
            .list_at_quote(
                crate::AgentId(uuid::Uuid::new_v4()),
                citizen
                    .map()
                    .public_place(crate::locations::Location::Market),
                Good::Berries,
                1000,
                100.0,
            )
            .unwrap();
        citizen.with_market(market)
    }

    fn baker() -> Citizen {
        use crate::{AgentKind, StartingRole, Universe};
        let worker = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_good(Good::Flour, 2000)
            .unwrap()
            .with_good(Good::Wood, 500)
            .unwrap()
            .with_good(Good::Water, 2000)
            .unwrap()
            .with_good(Good::Berries, 2000)
            .unwrap();
        let (world, id) = Universe::with_map(crate::locations::Map::default())
            .with_citizen("Baker", worker)
            .unwrap();
        let world = world
            .with_property(id, crate::locations::Location::Bakery)
            .unwrap()
            .0;
        let AgentKind::Citizen(mut worker) = world.agents()[&id].kind.clone();
        worker.refresh_production_targets(true).unwrap();
        worker
    }

    #[test]
    fn production_balances_recipes_and_finishes_the_first_crossing_job() {
        use crate::production::Recipe;
        let worker = baker().with_good(Good::BreadRoll, 100).unwrap();
        let variants = [site_candidate(
            &worker,
            worker.production_place(Recipe::BakeRolls).unwrap(),
        )];
        assert_eq!(variants.len(), 1);
        let candidate = &variants[0];
        assert!(
            candidate
                .actions
                .iter()
                .any(|action| action.production_recipe() == Some(Recipe::BakeBerryPie))
        );
        assert!(candidate.elapsed_ms >= PRODUCTION_DURATION_MS);
        assert!(
            candidate.elapsed_ms - candidate.action_durations_ms.last().unwrap()
                < PRODUCTION_DURATION_MS
        );
    }

    #[test]
    fn production_crossing_budget_includes_travel_and_input_preparation() {
        use crate::locations::Position;
        let worker = baker()
            .with_position(Position { x: 1000.0, y: 0.0 })
            .unwrap();
        let variants = variants_after(
            &Prediction::new(&worker, Cooldowns::default()),
            Effect::Production,
        )
        .unwrap();
        assert!(!variants.is_empty());
        let longest = variants
            .iter()
            .max_by_key(|variant| variant.elapsed_ms)
            .unwrap();
        assert!(matches!(longest.actions[0], CitizenAction::Travel(_)));
        assert_eq!(longest.action_durations_ms[0], 600_000);
        assert!(longest.elapsed_ms >= PRODUCTION_DURATION_MS);
        assert!(
            longest.elapsed_ms - longest.action_durations_ms.last().unwrap()
                < PRODUCTION_DURATION_MS
        );
        let short = baker().with_good(Good::Water, 0).unwrap();
        let supplied = variants_after(
            &Prediction::new(&short, Cooldowns::default()),
            Effect::Production,
        )
        .unwrap();
        assert!(
            supplied.iter().any(|variant| variant
                .actions
                .iter()
                .any(|action| action.production_recipe()
                    == Some(crate::production::Recipe::FetchWater)))
        );
        assert!(supplied.iter().all(|variant| variant.elapsed_ms
            - variant.action_durations_ms.last().unwrap()
            < PRODUCTION_DURATION_MS));
    }

    fn buying_baker(flour_stock: u64) -> Citizen {
        let worker = baker()
            .with_good(Good::Flour, 0)
            .unwrap()
            .with_good(Good::Wood, 0)
            .unwrap()
            .with_good(Good::Water, 0)
            .unwrap()
            .with_good(Good::Berries, 0)
            .unwrap()
            .with_good(Good::BerryPie, 16)
            .unwrap()
            .with_coins(2000)
            .unwrap();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        let mut market = worker.market().clone();
        for (good, units) in [
            (Good::Flour, flour_stock),
            (Good::Wood, 1000),
            (Good::Water, 2000),
            (Good::Berries, 1000),
        ] {
            market
                .list(
                    seller,
                    worker
                        .map()
                        .public_place(crate::locations::Location::Market),
                    good,
                    units,
                )
                .unwrap();
        }
        let mut worker = worker.with_market(market);
        worker.refresh_production_targets(true).unwrap();
        worker
    }

    #[test]
    fn repeated_batches_buy_the_candidate_inputs_once_and_advance_each_batch() {
        use crate::production::{Recipe, Skill};
        let source = buying_baker(2000);
        let initial = Prediction::new(&source, Cooldowns::default());
        let variants = production_order(&initial, &[Recipe::BakeRolls; 2], None, 0, 0).unwrap();
        let candidate = variants
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.actions.as_slice(),
                    [
                        CitizenAction::BuyAt { .. },
                        CitizenAction::Produce(Recipe::BakeRolls),
                        CitizenAction::Produce(Recipe::BakeRolls)
                    ]
                )
            })
            .unwrap();
        let CitizenAction::BuyAt { list, .. } = candidate.actions[0] else {
            unreachable!()
        };
        assert_eq!(list.units(Good::Flour), 200);
        assert_eq!(list.units(Good::Wood), 50);
        assert_eq!(list.units(Good::Water), 200);
        assert_eq!(candidate.citizen.usable_owned_units(Good::BreadRoll), 20);
        assert_eq!(candidate.citizen.available_units(Good::Flour), 0);
        assert_eq!(
            candidate.citizen.skill_practice_ms(Skill::Baking).unwrap()
                - source.skill_practice_ms(Skill::Baking).unwrap(),
            candidate.action_durations_ms[1] + candidate.action_durations_ms[2]
        );
        assert_eq!(source.skill_practice_ms(Skill::Baking), Some(0));
        let mut actual = source.clone();
        for (&action, &duration) in candidate.actions.iter().zip(&candidate.action_durations_ms) {
            actual = actual
                .start_action(action)
                .unwrap()
                .advance(duration)
                .unwrap();
        }
        assert_eq!(
            actual.skill_practice_ms(Skill::Baking),
            candidate.citizen.skill_practice_ms(Skill::Baking)
        );
        assert!(candidate.action_durations_ms[2] < candidate.action_durations_ms[1]);
        assert!(
            candidate
                .perform(CitizenAction::Buy(list), 0)
                .unwrap()
                .is_none()
        );
        let one = production_order(&initial, &[Recipe::BakeRolls], None, 0, 0).unwrap();
        let CitizenAction::BuyAt { list, .. } = one[0].actions[0] else {
            unreachable!()
        };
        assert_eq!(list.units(Good::Flour), 100);
        assert_eq!(source.units(Good::Flour), 0);
    }

    #[test]
    fn mixed_recipe_order_combines_inputs_and_preserves_actions() {
        use crate::production::Recipe;
        let initial = Prediction::new(&buying_baker(2000), Cooldowns::default());
        let variants = production_order(
            &initial,
            &[Recipe::BakeRolls, Recipe::BakeBerryPie],
            None,
            0,
            0,
        )
        .unwrap();
        let candidate = variants
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.actions.as_slice(),
                    [
                        CitizenAction::BuyAt { .. },
                        CitizenAction::Produce(Recipe::BakeRolls),
                        CitizenAction::Produce(Recipe::BakeBerryPie)
                    ]
                )
            })
            .unwrap();
        let CitizenAction::BuyAt { list, .. } = candidate.actions[0] else {
            unreachable!()
        };
        assert_eq!(list.units(Good::Flour), 200);
        assert_eq!(list.units(Good::Wood), 50);
        assert_eq!(list.units(Good::Water), 150);
        assert_eq!(list.units(Good::Berries), 100);
        assert_eq!(candidate.citizen.usable_owned_units(Good::BreadRoll), 10);
        assert_eq!(
            candidate.citizen.usable_owned_units(Good::BerryPie),
            initial.citizen.usable_owned_units(Good::BerryPie) + 2
        );
    }

    #[test]
    fn combined_procurement_keeps_only_the_final_crossing_batch() {
        use crate::production::Recipe;
        let mut initial = Prediction::new(&buying_baker(2000), Cooldowns::default());
        initial.elapsed_ms = GOAL_HORIZON_MS - 60 * 60_000;
        let one = production_order(&initial, &[Recipe::BakeRolls], None, 0, 0).unwrap();
        assert!(
            one.iter()
                .any(|candidate| candidate.elapsed_ms >= GOAL_HORIZON_MS)
        );
        assert!(
            production_order(&initial, &[Recipe::BakeRolls; 2], None, 0, 0)
                .unwrap()
                .is_empty()
        );
        initial.elapsed_ms = GOAL_HORIZON_MS;
        assert!(
            production_order(&initial, &[Recipe::BakeRolls], None, 0, 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn production_evidence_keeps_variants_and_records_observed_window_exclusion() {
        use crate::production::Recipe;
        let source = buying_baker(2000);
        let initial = Prediction::new(&source, Cooldowns::default());
        let mut diagnostics: Vec<_> = source
            .available_recipes()
            .map(|recipe| super::super::ProductionDecision::new(&source, recipe))
            .collect();
        let observed = production_prefixes(initial.clone(), 0, &mut diagnostics).unwrap();
        let original = production_prefixes(initial, 0, &mut []).unwrap();
        assert_eq!(
            observed.iter().map(|p| &p.actions).collect::<Vec<_>>(),
            original.iter().map(|p| &p.actions).collect::<Vec<_>>()
        );
        assert!(diagnostics.iter().any(|d| d.prefix_attempted));

        let mut initial = Prediction::new(&source, Cooldowns::default());
        initial.elapsed_ms = GOAL_HORIZON_MS - 60 * 60_000;
        let mut excluded = false;
        assert!(
            production_order_with_evidence(
                &initial,
                &[Recipe::BakeRolls; 2],
                None,
                0,
                0,
                &mut excluded
            )
            .unwrap()
            .is_empty()
        );
        assert!(excluded);
    }

    #[test]
    fn input_stock_shortage_retains_shorter_prefixes_and_next_recipe_fallback() {
        use crate::production::Recipe;
        let initial = Prediction::new(&buying_baker(100), Cooldowns::default());
        assert!(
            !production_order(&initial, &[Recipe::BakeRolls], None, 0, 0)
                .unwrap()
                .is_empty()
        );
        assert!(
            production_order(&initial, &[Recipe::BakeRolls; 2], None, 0, 0)
                .unwrap()
                .is_empty()
        );
        assert!(!production_prefixes(initial, 0, &mut []).unwrap().is_empty());
        let source = buying_baker(2000).with_good(Good::Flour, 100).unwrap();
        let mut market = source.market().clone();
        // Pie cannot obtain its berries; bread remains available after the preferred recipe fails.
        let berries: Vec<_> = market
            .orders()
            .filter(|order| order.good == Good::Berries)
            .cloned()
            .collect();
        for order in berries {
            market
                .withdraw(
                    order.seller.citizen().unwrap(),
                    order.place,
                    Good::Berries,
                    order.units,
                )
                .unwrap();
        }
        let mut source = source
            .with_market(market)
            .with_good(Good::Berries, 0)
            .unwrap();
        source.refresh_production_targets(true).unwrap();
        let targets = source.production_targets().unwrap();
        assert_eq!(targets.remaining_batches(Recipe::BakeBerryPie), 0);
        assert!(targets.remaining_batches(Recipe::BakeRolls) > 0);
        assert!(
            production_prefixes(Prediction::new(&source, Cooldowns::default()), 0, &mut [])
                .unwrap()
                .iter()
                .any(|candidate| candidate
                    .actions
                    .iter()
                    .any(|action| action.production_recipe() == Some(Recipe::BakeRolls)))
        );
    }

    #[test]
    fn gathering_order_bounds_follow_the_primitive_duration() {
        assert_eq!(gathering_segments(GOAL_HORIZON_MS, 30 * 60_000), 8);
        assert_eq!(gathering_segments(GOAL_HORIZON_MS, 60 * 60_000), 4);
    }

    #[test]
    fn gathering_order_metadata_survives_goals_and_nested_supply() {
        let initial = Prediction::new(&hungry(30), Cooldowns::default());
        let orders = order_variants(
            &initial,
            CitizenAction::Produce(crate::production::Recipe::Forage),
            0.0,
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            orders.len(),
            gathering_segments(GOAL_HORIZON_MS, ACTION_DURATION_MS) as usize
        );
        let gathered = &orders[1];
        assert_eq!(
            gathered.actions,
            [CitizenAction::Produce(crate::production::Recipe::Forage); 2]
        );
        assert!(
            order_variants(
                gathered,
                CitizenAction::Produce(crate::production::Recipe::Forage),
                0.0,
                0,
                0
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            best_variant_after(gathered, Effect::Production)
                .unwrap()
                .is_none_or(|v| !v
                    .actions
                    .contains(&CitizenAction::Produce(crate::production::Recipe::Forage)))
        );
        let mut funded = gathered.clone();
        funded.citizen = funded.citizen.with_coins(100).unwrap();
        let funded = deliver_reserves(&funded, 0).unwrap().unwrap();
        let hunger = best_variant(
            &funded.citizen,
            Effect::ReduceHunger,
            Cooldowns::default(),
            0,
        )
        .unwrap()
        .unwrap();
        assert!(
            !hunger
                .actions
                .contains(&CitizenAction::Produce(crate::production::Recipe::Forage))
        );
        let delivered = deliver_reserves(&orders[5], 0).unwrap().unwrap();
        let eaten = delivered.perform(CitizenAction::Eat, 0).unwrap().unwrap();
        assert!(
            !order_variants(
                &eaten,
                CitizenAction::Produce(crate::production::Recipe::Forage),
                0.0,
                0,
                0
            )
            .unwrap()
            .is_empty()
        );
        let reset = Prediction::new(&gathered.citizen, Cooldowns::default());
        assert!(
            !order_variants(
                &reset,
                CitizenAction::Produce(crate::production::Recipe::Forage),
                0.0,
                0,
                0
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn gathering_orders_charge_travel_once_and_preserve_segment_predictions() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 100.0, y: 0.0 },
            Position::default(),
            Position::default(),
        )
        .unwrap();
        let source = hungry(0).with_map(map.clone()).unwrap();
        let initial = Prediction::new(
            &source,
            Cooldowns {
                eat: 100_000_000,
                buy: 0,
                sell: 0,
            },
        );
        let orders = order_variants(
            &initial,
            CitizenAction::Produce(crate::production::Recipe::Forage),
            0.0,
            0,
            0,
        )
        .unwrap();
        let order = &orders[2];
        assert_eq!(
            order.actions,
            [
                CitizenAction::Travel(map.public_place(Location::Forest)),
                CitizenAction::Produce(crate::production::Recipe::Forage),
                CitizenAction::Produce(crate::production::Recipe::Forage),
                CitizenAction::Produce(crate::production::Recipe::Forage)
            ]
        );
        let mut replay = initial;
        for action in &order.actions {
            replay = replay.perform(*action, 0).unwrap().unwrap();
        }
        assert_eq!(order.citizen, replay.citizen);
        assert_eq!(order.elapsed_ms, replay.elapsed_ms);
        assert_eq!(order.average().unwrap(), replay.average().unwrap());
        assert_eq!(order.cooldowns.eat, replay.cooldowns.eat);
    }

    #[test]
    fn cooldowns_are_separate_start_at_completion_and_reset_for_new_plans() {
        let source = hungry(100)
            .with_coins(100)
            .unwrap()
            .with_home_good(Good::Berries, 1)
            .unwrap();
        let initial = Prediction::new(&source, Cooldowns::default());
        let generic = initial
            .perform(
                CitizenAction::Buy(crate::marketplace::ShoppingList::single(Good::Berries, 10)),
                0,
            )
            .unwrap()
            .unwrap();
        assert_eq!(generic.cooldowns.buy, TRADE_PLAN_COOLDOWN_MS);
        assert!(
            generic
                .perform(
                    CitizenAction::Buy(crate::marketplace::ShoppingList::single(Good::Berries, 10)),
                    0
                )
                .unwrap()
                .is_none()
        );
        assert!(
            generic
                .perform(CitizenAction::BuyFood(crate::marketplace::Good::Berries), 0)
                .unwrap()
                .is_none()
        );
        let sold = initial
            .perform(CitizenAction::List(Good::Berries, 100), 0)
            .unwrap()
            .unwrap();
        assert_eq!(sold.cooldowns.sell, TRADE_PLAN_COOLDOWN_MS);
        let bought = sold
            .perform(CitizenAction::BuyFood(crate::marketplace::Good::Berries), 0)
            .unwrap()
            .unwrap();
        assert_eq!(bought.cooldowns.buy, TRADE_PLAN_COOLDOWN_MS);
        assert_eq!(
            bought.cooldowns.sell,
            TRADE_PLAN_COOLDOWN_MS - TRADE_DURATION_MS
        );
        let delivered = deliver_reserves(&bought, 0).unwrap().unwrap();
        let eaten = delivered.perform(CitizenAction::Eat, 0).unwrap().unwrap();
        assert_eq!(eaten.cooldowns.eat, EAT_PLAN_COOLDOWN_MS);
        assert!(
            eaten
                .perform(CitizenAction::BuyFood(crate::marketplace::Good::Berries), 0)
                .unwrap()
                .is_none()
        );
        let mut elapsed = eaten.clone();
        for _ in 0..4 {
            elapsed = elapsed.perform(CitizenAction::Wait, 0).unwrap().unwrap();
        }
        assert!(
            elapsed
                .perform(CitizenAction::BuyFood(crate::marketplace::Good::Berries), 0)
                .unwrap()
                .is_some()
        );
        elapsed.citizen = elapsed.citizen.with_good(Good::Berries, 10).unwrap();
        assert!(
            elapsed
                .perform(CitizenAction::List(Good::Berries, 10), 0)
                .unwrap()
                .is_some()
        );
        let mut boundary = Cooldowns {
            eat: 1,
            buy: 1,
            sell: 1,
        };
        boundary = boundary.after(CitizenAction::Wait, 1);
        assert_eq!(boundary.eat, 0);
        assert_eq!(boundary.buy, 0);
        assert_eq!(boundary.sell, 0);
        let reset = Prediction::new(&eaten.citizen, Cooldowns::default());
        assert!(
            reset
                .perform(CitizenAction::BuyFood(crate::marketplace::Good::Berries), 0)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn splitting_linear_travel_preserves_its_duration_weighted_score() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 150.0, y: 0.0 },
            Position { x: 50.0, y: 0.0 },
            Position::default(),
        )
        .unwrap();
        let citizen = Citizen::with_needs(10.0, -100.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let direct = initial
            .perform(CitizenAction::Travel(map.public_place(Location::Forest)), 0)
            .unwrap()
            .unwrap();
        let middle = initial
            .perform(CitizenAction::Travel(map.public_place(Location::River)), 0)
            .unwrap()
            .unwrap();
        assert_eq!(middle.score.elapsed_ms, 30_000);
        let split = middle
            .perform(CitizenAction::Travel(map.public_place(Location::Forest)), 0)
            .unwrap()
            .unwrap();
        assert!((direct.average().unwrap() - split.average().unwrap()).abs() < 1e-12);
        assert_eq!(direct.average().unwrap(), 0.0);
        assert_eq!(direct.score.elapsed_ms, 90_000);
    }

    #[test]
    fn goal_scores_cover_their_own_duration_and_plan_scores_preserve_the_prefix() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 50.0, y: 0.0 },
            Position::default(),
            Position::default(),
        )
        .unwrap();
        let citizen = Citizen::with_needs(10.0, -100.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let prefix = initial
            .perform(CitizenAction::Travel(map.public_place(Location::Forest)), 0)
            .unwrap()
            .unwrap();
        let next = best_variant_after(&prefix, Effect::Production)
            .unwrap()
            .unwrap();
        let mut replay = prefix.clone();
        for action in &next.actions {
            replay = replay.perform(*action, 0).unwrap().unwrap();
        }
        assert_eq!(next.average().unwrap(), replay.average().unwrap());
        assert_eq!(next.goal_score.elapsed_ms, next.elapsed_ms);
        assert_eq!(next.score.elapsed_ms, prefix.elapsed_ms + next.elapsed_ms);
        let local = best_variant(
            &prefix.citizen,
            Effect::Production,
            prefix.cooldowns,
            prefix.actions.len(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(next.actions, local.actions);
        assert_eq!(next.goal_average().unwrap(), local.average().unwrap());
    }

    #[test]
    fn water_reserves_withdraw_own_stock_before_obtaining_the_remainder() {
        let buyer = Citizen::new(-50.0)
            .unwrap()
            .with_good(Good::BreadRoll, 30)
            .unwrap()
            .with_good(Good::Water, 200)
            .unwrap();
        let listed = buyer
            .start_action(CitizenAction::List(Good::Water, 200))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let variants = variants_after(
            &Prediction::new(&listed, Cooldowns::default()),
            Effect::ReplenishReserves,
        )
        .unwrap();
        assert!(!variants.is_empty());
        assert!(
            variants
                .iter()
                .all(|p| p.actions.first() == Some(&CitizenAction::Withdraw(Good::Water, 200)))
        );
        assert!(
            variants
                .iter()
                .all(|p| reserve_units(&p.citizen, Good::Water) == 400)
        );
        assert_eq!(listed.market().listed_units(listed.id(), Good::Water), 200);
    }

    #[test]
    fn water_provisioning_uses_available_purchases_and_accepts_partial_target() {
        let buyer = reserve_shopper(100, &[(Good::Water, 1000, 1.0)])
            .with_home_good(Good::BreadRoll, 30)
            .unwrap()
            .with_home_good(Good::Fish, 3)
            .unwrap()
            .with_home_good(Good::Cabbage, 3)
            .unwrap();
        let variants =
            provision_household(&Prediction::new(&buyer, Cooldowns::default()), 0).unwrap();
        assert!(variants[0].actions.iter().any(|action| matches!(action, CitizenAction::BuyAt { list, .. } if list.units(Good::Water) > 0)));
        assert_eq!(variants[0].citizen.home_drink_units(), 1000);
        assert!(matches!(
            variants[0].actions.last(),
            Some(CitizenAction::DepositStored { .. })
        ));
        let stocked = buyer.with_home_good(Good::Water, 1200).unwrap();
        assert!(
            provision_household(&Prediction::new(&stocked, Cooldowns::default()), 0)
                .unwrap()
                .is_empty()
        );
    }

    fn reserve_shopper(coins: crate::Coins, listings: &[(Good, u64, f64)]) -> Citizen {
        let buyer = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_coins(coins)
            .unwrap();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        let place = buyer.map().public_place(crate::locations::Location::Market);
        let mut market = buyer.market().clone();
        for &(good, units, price) in listings {
            market.prices = market.prices.with_price(good, price).unwrap();
            market
                .list_at_quote(seller, place, good, units, price)
                .unwrap();
        }
        buyer.with_market(market)
    }

    #[test]
    fn listing_excess_does_not_create_a_reason_to_withdraw_the_same_rolls() {
        for beer in [0, 1200] {
            let mut buyer = reserve_shopper(2299, &[])
                .with_good(Good::BreadRoll, 53)
                .unwrap()
                .with_good(Good::Fish, 3)
                .unwrap()
                .with_good(Good::Berries, 16)
                .unwrap()
                .with_good(Good::Beer, beer)
                .unwrap()
                .with_good(Good::Water, 400)
                .unwrap();
            buyer.hunger = -70.25510457735808;
            buyer.thirst = -64.60994328703555;
            buyer.tiredness = -11.20994328703681;
            buyer.diet = [23.798763888888455, 37.92093749999927, 0.0, 0.0];
            buyer.garment_condition = Some(0.9092504432870485);
            buyer.market_time_ms = 587857009;
            buyer.last_meal_started_ms = Some(573617806);
            for (good, price) in [
                (Good::BreadRoll, 2.213327433727574),
                (Good::Fish, 7.464077507330149),
                (Good::Berries, 109.85952772127864),
                (Good::Water, 17.271832500000002),
                (Good::FlaxGarment, 207.2008388340001),
            ] {
                buyer.prices = buyer.prices.with_price(good, price).unwrap();
            }
            let excess = buyer.excess_goods().unwrap().units(Good::BreadRoll);
            assert!(excess > 0);
            let listed = buyer
                .start_action(CitizenAction::ListExcess)
                .unwrap()
                .advance(TRADE_DURATION_MS)
                .unwrap();
            assert_eq!(
                listed.market().listed_units(listed.id(), Good::BreadRoll),
                excess
            );
            assert!(listed.food_nutrition() >= crate::FOOD_RESERVE_CAP_NUTRITION);
            let variants = variants_after(
                &Prediction::new(&listed, Cooldowns::default()),
                Effect::ReplenishReserves,
            )
            .unwrap();
            assert!(
                variants
                    .iter()
                    .all(|variant| !variant.actions.iter().any(|action| {
                        matches!(
                            action,
                            CitizenAction::Withdraw(Good::BreadRoll | Good::Beer, _)
                        )
                    })),
                "{:?}",
                variants
                    .iter()
                    .map(|variant| &variant.actions)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn diet_aware_reserves_buy_variety_with_a_week_of_rolls() {
        let mut buyer = reserve_shopper(
            1000,
            &[
                (Good::Fish, 20, Good::Fish.core_price()),
                (Good::Cabbage, 20, Good::Cabbage.core_price()),
                (Good::BreadRoll, 100, Good::BreadRoll.core_price()),
            ],
        )
        .with_home_good(Good::BreadRoll, 70)
        .unwrap()
        .with_home_good(Good::Water, crate::WATER_RESERVE_GRAMS)
        .unwrap();
        buyer.diet = [0.0, 100.0, 0.0, 0.0];
        let snapshot = buyer.clone();
        let chosen = best_variant(&buyer, Effect::ReplenishReserves, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert!(chosen.actions.iter().any(|action| matches!(
            action,
            CitizenAction::BuyAt { list, .. }
                if list.units(Good::Fish) > 0 || list.units(Good::Cabbage) > 0
        )));
        assert!(
            reserve_units(&chosen.citizen, Good::Fish) > 0
                || reserve_units(&chosen.citizen, Good::Cabbage) > 0
        );
        assert!(
            pantry_categories(&chosen.citizen)
                .into_iter()
                .filter(|points| *points >= 25.0)
                .count()
                >= 3
        );
        assert_eq!(buyer, snapshot);
        let reserves = chosen.citizen.reserved_goods().unwrap();
        assert!(reserves.units(Good::Fish) > 0 || reserves.units(Good::Cabbage) > 0);
    }

    #[test]
    fn unaffordable_variety_is_declined_and_purchase_cooldowns_still_apply() {
        let mut buyer = reserve_shopper(1, &[(Good::Fish, 20, 100.0), (Good::Cabbage, 20, 100.0)])
            .with_home_good(Good::BreadRoll, 70)
            .unwrap()
            .with_home_good(Good::Water, crate::WATER_RESERVE_GRAMS)
            .unwrap();
        buyer.diet = [0.0, 100.0, 0.0, 0.0];
        let plan = super::super::plan(&buyer).unwrap();
        assert!(!plan.actions().iter().any(|action| matches!(action,
            CitizenAction::BuyAt { list, .. } | CitizenAction::Buy(list)
            if list.units(Good::Fish) > 0 || list.units(Good::Cabbage) > 0)));
        let mut initial = Prediction::new(&buyer, Cooldowns::default());
        initial.cooldowns.buy = TRADE_PLAN_COOLDOWN_MS;
        assert!(
            variants_after(&initial, Effect::ReplenishReserves)
                .unwrap()
                .iter()
                .all(|p| !p
                    .actions
                    .iter()
                    .any(|a| matches!(a, CitizenAction::BuyAt { .. } | CitizenAction::Buy(_))))
        );
    }

    #[test]
    fn stocked_variety_demand_excludes_unneeded_calories_and_expensive_fourth() {
        let buyer = reserve_shopper(1000, &[])
            .with_home_good(Good::BreadRoll, 70)
            .unwrap()
            .with_home_good(Good::Water, crate::WATER_RESERVE_GRAMS)
            .unwrap();
        let initial = Prediction::new(&buyer, Cooldowns::default());
        variants_after(&initial, Effect::ReplenishReserves).unwrap();
        assert_eq!(initial.blocked_requests.borrow().units(Good::BreadRoll), 0);
        let mut buyer = buyer;
        buyer = buyer.with_prices(
            buyer
                .prices
                .with_price(Good::Fish, 100.0)
                .unwrap()
                .with_price(Good::Cabbage, 100.0)
                .unwrap(),
        );
        let initial = Prediction::new(&buyer, Cooldowns::default());
        variants_after(&initial, Effect::ReplenishReserves).unwrap();
        let request = *initial.blocked_requests.borrow();
        assert_eq!(
            usize::from(request.units(Good::Fish) > 0)
                + usize::from(request.units(Good::Cabbage) > 0),
            1
        );
        assert_eq!(request.units(Good::BreadRoll), 0);
        assert!(request.units(Good::Berries) > 0);
    }

    #[test]
    fn stocked_variety_withdraws_owned_food_before_shopping() {
        let mut buyer = reserve_shopper(1000, &[(Good::Fish, 20, Good::Fish.core_price())])
            .with_good(Good::BreadRoll, 70)
            .unwrap()
            .with_good(Good::Fish, 4)
            .unwrap()
            .with_good(Good::Water, crate::WATER_RESERVE_GRAMS)
            .unwrap();
        buyer.diet = [0.0, 100.0, 0.0, 0.0];
        let listed = buyer
            .start_action(CitizenAction::List(Good::Fish, 4))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let chosen = best_variant(&listed, Effect::ReplenishReserves, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert!(matches!(
            chosen.actions.first(),
            Some(CitizenAction::Withdraw(Good::Fish, _))
        ));
        assert!(reserve_units(&chosen.citizen, Good::Fish) > 0);
    }

    #[test]
    fn listed_rolls_do_not_preempt_owned_category_stock() {
        let mut buyer = reserve_shopper(1000, &[(Good::Fish, 20, Good::Fish.core_price())])
            .with_home_good(Good::BreadRoll, 100)
            .unwrap()
            .with_good(Good::Fish, 4)
            .unwrap()
            .with_home_good(Good::Water, crate::WATER_RESERVE_GRAMS)
            .unwrap();
        buyer.diet = [0.0, 100.0, 0.0, 0.0];
        let listed = buyer
            .start_action(CitizenAction::List(Good::BreadRoll, 30))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap()
            .start_action(CitizenAction::List(Good::Fish, 4))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let chosen = best_variant(&listed, Effect::ReplenishReserves, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert!(matches!(
            chosen.actions.first(),
            Some(CitizenAction::Withdraw(Good::Fish, 3))
        ));
        assert!(!chosen.actions.iter().any(|action| matches!(action, CitizenAction::BuyAt { list, .. } if list.units(Good::Fish) > 0)));
        assert_eq!(
            chosen
                .citizen
                .market()
                .listed_units(chosen.citizen.id(), Good::BreadRoll),
            30
        );
    }

    #[test]
    fn reserve_acquisition_can_withdraw_own_food_and_omits_completed_targets() {
        let buyer = Citizen::new(0.0)
            .unwrap()
            .with_good(Good::BreadRoll, 20)
            .unwrap();
        let listed = buyer
            .start_action(CitizenAction::List(Good::BreadRoll, 20))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let variants = variants_after(
            &Prediction::new(&listed, Cooldowns::default()),
            Effect::ReplenishReserves,
        )
        .unwrap();
        assert!(variants.iter().any(|variant| variant.actions.first()
            == Some(&CitizenAction::Withdraw(Good::BreadRoll, 10))));
        assert!(variants.iter().any(|variant| {
            variant
                .actions
                .iter()
                .map(|action| match action {
                    CitizenAction::Withdraw(Good::BreadRoll, units) => *units,
                    _ => 0,
                })
                .sum::<u64>()
                == 20
        }));
        let stocked = buyer
            .with_home_good(Good::BreadRoll, 30)
            .unwrap()
            .with_home_good(Good::Fish, 3)
            .unwrap()
            .with_home_good(Good::Cabbage, 3)
            .unwrap()
            .with_home_good(Good::Water, crate::WATER_RESERVE_GRAMS)
            .unwrap();
        assert!(
            variants_after(
                &Prediction::new(&stocked, Cooldowns::default()),
                Effect::ReplenishReserves
            )
            .unwrap()
            .iter()
            .all(|p| p.actions.iter().all(|a| matches!(
                a,
                CitizenAction::DepositStored { .. } | CitizenAction::Travel(_)
            )))
        );
    }

    #[test]
    fn gathering_completion_wealth_and_reserves_are_spread_across_the_action() {
        let citizen = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(100.0).unwrap());
        let gathered = Prediction::new(&citizen, Cooldowns::default())
            .perform(CitizenAction::Produce(crate::production::Recipe::Forage), 0)
            .unwrap()
            .unwrap();
        assert!((gathered.average().unwrap() - 0.05).abs() < 1e-12);
    }

    #[test]
    fn consumption_scores_use_component_boundaries_and_keep_full_advance_state() {
        for action in [CitizenAction::Eat, CitizenAction::Drink] {
            let source = Citizen::new(80.0)
                .unwrap()
                .with_thirst(80.0)
                .unwrap()
                .with_home_good(Good::Fish, 3)
                .unwrap()
                .with_home_good(Good::Cabbage, 3)
                .unwrap()
                .with_home_good(Good::BreadRoll, 10)
                .unwrap()
                .with_home_good(Good::Water, 1000)
                .unwrap();
            let started = source.start_action(action).unwrap();
            let active = started.active_action().unwrap();
            let duration = active.duration_ms();
            let first_step = active.next_boundary_ms();
            let middle = started.advance_predicted(first_step).unwrap();
            let final_state = started.advance_predicted(duration).unwrap();
            let waiting_middle = source.advance_predicted(first_step).unwrap();
            let initial_score =
                started.planning_wellbeing().unwrap() - source.planning_wellbeing().unwrap();
            let middle_score =
                middle.planning_wellbeing().unwrap() - waiting_middle.planning_wellbeing().unwrap();
            let expected = if action == CitizenAction::Eat {
                assert!(active.meal_units.iter().filter(|units| **units > 0).count() > 1);
                assert!(first_step < duration);
                let end = middle.advance_predicted(duration - first_step).unwrap();
                let waiting_end = waiting_middle
                    .advance_predicted(duration - first_step)
                    .unwrap();
                let end_score =
                    end.planning_wellbeing().unwrap() - waiting_end.planning_wellbeing().unwrap();
                ((initial_score * 0.5 + middle_score * 0.5) * first_step as f64
                    + (middle_score * 0.5 + end_score * 0.5) * (duration - first_step) as f64)
                    / duration as f64
            } else {
                assert_eq!(first_step, duration);
                initial_score * 0.5 + middle_score * 0.5
            };
            let predicted = Prediction::new(&source, Cooldowns::default())
                .perform(action, 0)
                .unwrap()
                .unwrap();
            assert!((predicted.average().unwrap() - expected).abs() < 1e-12);
            assert_eq!(predicted.goal_score.mean, predicted.score.mean);
            assert_eq!(predicted.score.elapsed_ms, duration);
            assert_eq!(
                ron::to_string(&predicted.citizen).unwrap(),
                ron::to_string(&final_state).unwrap()
            );
        }
    }

    #[test]
    fn consumption_predictions_preserve_validation_errors() {
        let source = Citizen::new(80.0)
            .unwrap()
            .with_thirst(80.0)
            .unwrap()
            .with_home_good(Good::BreadRoll, 10)
            .unwrap()
            .with_home_good(Good::Water, 1000)
            .unwrap();
        for action in [CitizenAction::Eat, CitizenAction::Drink] {
            let mut overflowing = source.clone();
            overflowing.market_time_ms = u64::MAX;
            assert!(matches!(
                Prediction::new(&overflowing, Cooldowns::default()).perform(action, 0),
                Err(SimulationError::TimeOverflow)
            ));
            let overflowing = source
                .with_good(Good::Flax, u64::MAX)
                .unwrap()
                .with_home_good(Good::Flax, 1)
                .unwrap();
            assert!(matches!(
                Prediction::new(&overflowing, Cooldowns::default()).perform(action, 0),
                Err(SimulationError::WealthOverflow)
            ));
        }
    }

    #[test]
    fn action_scores_are_weighted_by_duration() {
        let mut score = WeightedWellbeing::default();
        score.add(0.0, 10.0, 60_000).unwrap();
        score.add(10.0, 30.0, 180_000).unwrap();
        assert_eq!(score.mean, 16.25);
        assert_eq!(score.elapsed_ms, 240_000);
    }

    #[test]
    fn scoring_handles_opposite_extremes_without_overflow() {
        for first in [-f64::MAX, f64::MAX] {
            let mut score = WeightedWellbeing::default();
            score.add(first, first, 60_000).unwrap();
            score.add(-first, -first, 60_000).unwrap();
            assert_eq!(score.mean, 0.0);
            let mut endpoints = WeightedWellbeing::default();
            endpoints.add(first, -first, 60_000).unwrap();
            assert_eq!(endpoints.mean, 0.0);
        }
    }

    #[test]
    fn forage_production_uses_current_prices_and_expected_berry_yield() {
        for berries in [1.0, 4.0] {
            let source = Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_prices(crate::marketplace::Prices::new(berries).unwrap())
                .with_good(Good::Water, 10_000)
                .unwrap();
            let mut market = source.market().clone();
            let mut policy = market.caravan_policy();
            policy.exports[Good::Water as usize].allowed = false;
            market.set_caravan_policy(policy).unwrap();
            let source = source.with_market(market);
            let chosen = best_variant(&source, Effect::Production, Cooldowns::default(), 0)
                .unwrap()
                .unwrap();
            let segments = gathering_segments(PRODUCTION_DURATION_MS, ACTION_DURATION_MS);
            assert_eq!(
                chosen.actions,
                vec![
                    CitizenAction::ProduceAt {
                        place: source
                            .map()
                            .public_place(crate::locations::Location::Forest),
                        recipe: crate::production::Recipe::Forage
                    };
                    segments as usize
                ]
            );
            assert_eq!(chosen.citizen.prices(), source.prices());
            assert_eq!(chosen.citizen.berries_units(), 10 * segments);
        }
    }
}
