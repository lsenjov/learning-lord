use super::{EAT_PLAN_COOLDOWN_MS, REPLAN_MIN_NUTRITION};
use crate::{
    ACTION_DURATION_MS, BERRY_EATING_MS_PER_GRAM, BERRY_NUTRITION_PER_GRAM, Citizen, CitizenAction,
    MEAL_NOURISHMENT, SLEEP_DURATION_MS, SimulationError, TRADE_DURATION_MS, marketplace::Good,
};

pub const GOAL_HORIZON_MS: u64 = 4 * 60 * 60 * 1000;
pub const TRADE_PLAN_COOLDOWN_MS: u64 = 2 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Food,
    Coins,
    ReduceHunger,
    ReduceTiredness,
    ReduceClothingNeed,
    ReplenishReserves,
    Production,
    ListExcess,
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
            Self::Sleep => &[ReduceTiredness],
            Self::EquipClothing => &[ReduceClothingNeed],
            Self::BuyFood(good) if good.nutrition_per_unit().is_some() => {
                &[Food, ReplenishReserves]
            }
            Self::BuyFood(_) => &[],
            Self::Produce(recipe)
                if recipe
                    .outputs()
                    .iter()
                    .any(|(good, _)| good.nutrition_per_unit().is_some()) =>
            {
                &[Food, ReplenishReserves, Production]
            }
            Self::Withdraw(good, _) if good.nutrition_per_unit().is_some() => {
                &[Food, ReplenishReserves]
            }
            Self::Buy(list) | Self::BuyAt { list, .. }
                if list
                    .items()
                    .any(|(good, _)| good.nutrition_per_unit().is_some()) =>
            {
                &[Food, ReplenishReserves]
            }
            Self::Produce(_) => &[Production],
            Self::Wait
            | Self::Travel(_)
            | Self::ListExcess
            | Self::List(..)
            | Self::Buy(..)
            | Self::BuyAt { .. }
            | Self::Withdraw(..) => &[],
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
                citizen.id(),
                good,
                ((amount - citizen.food_nutrition()).max(0.0) / good.nutrition_per_unit()?).ceil()
                    as crate::Quantity,
            ),
            Self::Buy(list) => list.items().try_fold(0_i64, |sum, (good, units)| {
                sum.checked_add(citizen.market().purchase_cost(citizen.id(), good, units)?)
            }),
            Self::BuyAt { place, list } => list.items().try_fold(0_i64, |sum, (good, units)| {
                sum.checked_add(citizen.market().purchase_cost_at(
                    citizen.id(),
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
            Self::BuyFood(_)
            | Self::ListExcess
            | Self::List(..)
            | Self::Buy(..)
            | Self::BuyAt { .. }
            | Self::Withdraw(..) => TRADE_DURATION_MS,
            _ => ACTION_DURATION_MS,
        }
    }
}

const ACTIONS: [CitizenAction; 6] = [
    CitizenAction::Eat,
    CitizenAction::Sleep,
    CitizenAction::Produce(crate::production::Recipe::Forage),
    CitizenAction::BuyFood(Good::Berries),
    CitizenAction::BuyFood(Good::Bread),
    CitizenAction::BuyFood(Good::BerryPie),
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
    pub action_durations_ms: Vec<u64>,
    pub elapsed_ms: u64,
    score: WeightedWellbeing,
    goal_score: WeightedWellbeing,
    pub cooldowns: Cooldowns,
    last_gathering_order: Option<CitizenAction>,
}

impl Prediction {
    pub fn new(citizen: &Citizen, cooldowns: Cooldowns) -> Self {
        Self {
            citizen: citizen.clone(),
            actions: Vec::new(),
            action_durations_ms: Vec::new(),
            elapsed_ms: 0,
            score: WeightedWellbeing::default(),
            goal_score: WeightedWellbeing::default(),
            cooldowns,
            last_gathering_order: None,
        }
    }

    fn perform(
        &self,
        action: CitizenAction,
        prior_actions: usize,
    ) -> Result<Option<Self>, SimulationError> {
        if self.cooldowns.remaining(action) > 0
            || super::replan_check(&self.citizen, action, prior_actions + self.actions.len())
        {
            return Ok(None);
        }
        let started = self.citizen.start_action(action)?;
        let Some(active) = started.active_action() else {
            return Ok(None);
        };
        let duration = active.duration_ms();
        let citizen = started.advance_predicted(duration)?;
        let before = self.citizen.personal_wellbeing()?;
        let after = citizen.personal_wellbeing()?;
        let mut next = self.clone();
        if matches!(
            action,
            CitizenAction::BuyFood(_) | CitizenAction::Buy(_) | CitizenAction::BuyAt { .. }
        ) && action.effects().contains(&Effect::Food)
            && citizen.food_nutrition() <= self.citizen.food_nutrition()
        {
            return Ok(None);
        }
        next.citizen = citizen;
        next.score.add(before, after, duration)?;
        next.goal_score.add(before, after, duration)?;
        if !matches!(
            action,
            CitizenAction::Produce(crate::production::Recipe::Forage) | CitizenAction::Travel(_)
        ) {
            next.last_gathering_order = None;
        }
        next.actions.push(action);
        next.action_durations_ms.push(duration);
        next.elapsed_ms += duration;
        next.cooldowns = self.cooldowns.after(action, duration);
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
        Effect::Food => citizen.food_nutrition(),
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
                    ((requirement.amount - state.citizen.food_nutrition()).max(0.0)
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
    let [a, b, c] = Good::FOOD;
    for order in [
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ] {
        let mut remaining = (requirement.amount - state.citizen.food_nutrition()).max(0.0);
        let mut items = Vec::new();
        for good in order {
            let nutrition = good.nutrition_per_unit().unwrap();
            let units = state
                .citizen
                .market()
                .available_units(state.citizen.id(), good)
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
        let units = state
            .citizen
            .market()
            .listed_units(state.citizen.id(), good);
        if units > 0 {
            actions.push(CitizenAction::Withdraw(
                good,
                units.min(
                    ((requirement.amount - state.citizen.food_nutrition()).max(0.0)
                        / good.nutrition_per_unit().unwrap())
                    .ceil() as crate::Quantity,
                ),
            ));
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

fn replenish(
    state: &Prediction,
    target: f64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if state.elapsed_ms >= GOAL_HORIZON_MS {
        return Ok(Vec::new());
    }
    let requirement = Requirement {
        resource: Effect::Food,
        amount: target,
        coins: None,
    };
    let mut variants = Vec::new();
    for action in suppliers(state, requirement)? {
        for next in order_variants(state, action, target, 0, prior_actions)? {
            let available = next.citizen.food_nutrition();
            if available <= state.citizen.food_nutrition() {
                continue;
            }
            let tolerance = 32.0 * f64::EPSILON * available.abs().max(target);
            if available >= target || target - available <= tolerance {
                variants.push(next);
            } else {
                variants.extend(replenish(&next, target, prior_actions)?);
            }
        }
    }
    Ok(variants)
}

fn gathering_segments(horizon_ms: u64, duration_ms: u64) -> u64 {
    horizon_ms / duration_ms
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
            ((amount.max(MEAL_NOURISHMENT) - state.citizen.food_nutrition()).max(0.0)
                / good.nutrition_per_unit().unwrap())
            .ceil() as crate::Quantity,
        )),
        _ => None,
    };
    if let Some(list) = list {
        return shopping_trip(state, list, reserved_ms, prior_actions);
    }
    if let CitizenAction::Produce(recipe) = action
        && recipe != crate::production::Recipe::Forage
        && amount > state.citizen.food_nutrition()
    {
        return food_production_variants(state, recipe, amount, reserved_ms, prior_actions);
    }
    let gathering = matches!(
        action,
        CitizenAction::Produce(crate::production::Recipe::Forage)
    );
    if gathering && state.last_gathering_order == Some(action) {
        return Ok(Vec::new());
    }
    let mut variants = Vec::new();
    for ready in prepare(state, action, amount, reserved_ms, prior_actions)? {
        // Nested resource acquisition can change the preceding order.
        if gathering && ready.last_gathering_order == Some(action) {
            continue;
        }
        let limit = if gathering {
            gathering_segments(super::HORIZON_MS, action.predicted_duration())
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

fn shopping_trip(
    initial: &Prediction,
    list: crate::marketplace::ShoppingList,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if initial.cooldowns.buy > 0 {
        return Ok(Vec::new());
    }
    let mut state = initial.clone();
    let mut remaining = list;
    while remaining.items().next().is_some() {
        let citizen = &state.citizen;
        let place = citizen
            .map()
            .places()
            .values()
            .filter(|place| place.kind != crate::locations::Location::Home)
            .filter(|place| {
                remaining.items().any(|(good, _)| {
                    citizen
                        .market()
                        .available_units_at(citizen.id(), place.id, good)
                        > 0
                })
            })
            .min_by(|a, b| {
                citizen
                    .position()
                    .distance(a.position)
                    .total_cmp(&citizen.position().distance(b.position))
                    .then_with(|| a.id.0.cmp(&b.id.0))
            })
            .map(|place| place.id);
        let Some(place) = place else {
            return Ok(Vec::new());
        };
        let basket =
            crate::marketplace::ShoppingList::new(remaining.items().map(|(good, units)| {
                (
                    good,
                    units.min(
                        citizen
                            .market()
                            .available_units_at(citizen.id(), place, good),
                    ),
                )
            }))?;
        if citizen.position() != citizen.map().position(place) {
            let Some(travelled) = state.perform(CitizenAction::Travel(place), prior_actions)?
            else {
                return Ok(Vec::new());
            };
            state = travelled;
        }
        if state.elapsed_ms + reserved_ms > GOAL_HORIZON_MS {
            return Ok(Vec::new());
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
            return Ok(Vec::new());
        };
        remaining =
            crate::marketplace::ShoppingList::new(remaining.items().map(|(good, units)| {
                (
                    good,
                    units.saturating_sub(
                        bought
                            .citizen
                            .units(good)
                            .saturating_sub(inventory.units(good)),
                    ),
                )
            }))?;
        if !basket
            .items()
            .any(|(good, _)| bought.citizen.units(good) > inventory.units(good))
        {
            return Ok(Vec::new());
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
    let prepared = if let CitizenAction::Produce(recipe) = action {
        prepare_inputs(
            state,
            &recipe_requirements(&[recipe])?,
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
    recipes: &[crate::production::Recipe],
) -> Result<crate::marketplace::ShoppingList, SimulationError> {
    let mut balance = [0_i128; Good::COUNT];
    let mut needed = [0_i128; Good::COUNT];
    for recipe in recipes {
        for &(good, units) in recipe.inputs() {
            balance[good as usize] -= units as i128;
            needed[good as usize] = needed[good as usize].max(-balance[good as usize]);
        }
        for &(good, units) in recipe.outputs() {
            balance[good as usize] += units as i128;
        }
    }
    crate::marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
        (
            good,
            crate::Quantity::try_from(needed[good as usize]).unwrap_or(crate::Quantity::MAX),
        )
    }))
}

fn production_order(
    initial: &Prediction,
    recipes: &[crate::production::Recipe],
    food_target: Option<f64>,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    Ok(production_order_with_evidence(
        initial,
        recipes,
        food_target,
        reserved_ms,
        prior_actions,
        &mut false,
    )?
    .into_iter()
    .map(|(_, prediction)| prediction)
    .collect())
}

fn production_order_with_evidence(
    initial: &Prediction,
    recipes: &[crate::production::Recipe],
    food_target: Option<f64>,
    reserved_ms: u64,
    prior_actions: usize,
    preparation_limit: &mut bool,
) -> Result<Vec<(Vec<CitizenAction>, Prediction)>, SimulationError> {
    let requirements = recipe_requirements(recipes)?;
    let mut variants = Vec::new();
    for mut state in prepare_inputs(initial, &requirements, reserved_ms, prior_actions)? {
        let path = acquisition_path(&state.actions);
        let mut complete = requirements
            .items()
            .all(|(good, units)| state.citizen.units(good) >= units);
        for (index, &recipe) in recipes.iter().enumerate() {
            if !complete
                || index > 0
                    && food_target.is_some_and(|target| state.citizen.food_nutrition() >= target)
            {
                complete = false;
                break;
            }
            if food_target.is_none()
                && state
                    .citizen
                    .production_targets()
                    .is_some_and(|targets| targets.remaining_batches(recipe) == 0)
            {
                complete = false;
                break;
            }
            let action = CitizenAction::Produce(recipe);
            if recipe == crate::production::Recipe::Forage
                && (index == 0 || recipes[index - 1] != recipe)
                && state.last_gathering_order == Some(action)
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
            if state.elapsed_ms >= GOAL_HORIZON_MS
                || state.elapsed_ms + reserved_ms > GOAL_HORIZON_MS
            {
                *preparation_limit = true;
                complete = false;
                break;
            }
            let Some(next) = state.perform(action, prior_actions)? else {
                complete = false;
                break;
            };
            state = next;
            if recipe == crate::production::Recipe::Forage {
                state.last_gathering_order = Some(action);
            }
        }
        if complete {
            variants.push((path, state));
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
    let count = ((target - state.citizen.food_nutrition()) / gain).ceil() as usize;
    let mut recipes = Vec::new();
    let mut variants = Vec::new();
    for _ in 0..count {
        recipes.push(recipe);
        let next = production_order(state, &recipes, Some(target), reserved_ms, prior_actions)?;
        if next.is_empty() {
            break;
        }
        let can_extend = next.iter().any(|candidate| {
            candidate.elapsed_ms < GOAL_HORIZON_MS && candidate.citizen.food_nutrition() < target
        });
        variants.extend(next);
        if !can_extend {
            break;
        }
    }
    Ok(variants)
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
            let missing = units.saturating_sub(current.citizen.units(good));
            if missing == 0 {
                next_states.push(current);
                continue;
            }
            let listed = current
                .citizen
                .market()
                .listed_units(current.citizen.id(), good)
                .min(missing);
            let mut current = current;
            if listed > 0 {
                let action = CitizenAction::Withdraw(good, listed);
                let mut retrieved =
                    order_variants(&current, action, 0.0, reserved_ms, prior_actions)?;
                if let Some(ready) = retrieved.pop() {
                    current = ready;
                } else {
                    continue;
                }
            }
            next_states.push(current.clone());
            if current.citizen.units(good) < units {
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
        let list = crate::marketplace::ShoppingList::new(
            requirements
                .items()
                .map(|(good, units)| (good, units.saturating_sub(state.citizen.units(good)))),
        )?;
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
                .all(|(good, units)| bought.citizen.units(good) >= units)
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
    for next in order_variants(state, action, 0.0, reserved_ms, prior_actions)? {
        if next.elapsed_ms + reserved_ms > GOAL_HORIZON_MS
            || next.citizen.units(good) <= state.citizen.units(good)
        {
            continue;
        }
        if next.citizen.units(good) >= units {
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
    initial.score = prefix.score;
    initial.last_gathering_order = prefix.last_gathering_order;
    variants_from(initial, goal, prefix.actions.len())
}

const WORK_CHECKPOINT_MS: u64 = 60 * 60 * 1000;

fn acquisition_path(actions: &[CitizenAction]) -> Vec<CitizenAction> {
    let mut path = Vec::new();
    for &action in actions {
        let action = match action {
            CitizenAction::Buy(list) => CitizenAction::Buy(
                crate::marketplace::ShoppingList::new(list.items().map(|(good, _)| (good, 1)))
                    .expect("an existing basket contains valid goods"),
            ),
            CitizenAction::BuyAt { place, list } => CitizenAction::BuyAt {
                place,
                list: crate::marketplace::ShoppingList::new(
                    list.items().map(|(good, _)| (good, 1)),
                )
                .expect("an existing basket contains valid goods"),
            },
            CitizenAction::Withdraw(good, _) => CitizenAction::Withdraw(good, 1),
            other => other,
        };
        if path.last() != Some(&action) {
            path.push(action);
        }
    }
    path
}

#[derive(Default)]
struct WorkCheckpoints {
    paths: Vec<WorkPath>,
}

struct WorkPath {
    acquisition: Vec<CitizenAction>,
    checkpoint_ms: u64,
    selected: Vec<Prediction>,
    last: Prediction,
}

impl WorkCheckpoints {
    fn consider(&mut self, path: Vec<CitizenAction>, candidate: Prediction) {
        let index = self.paths.iter().position(|old| old.acquisition == path);
        let entry = match index {
            Some(index) => &mut self.paths[index],
            None => {
                self.paths.push(WorkPath {
                    acquisition: path,
                    checkpoint_ms: WORK_CHECKPOINT_MS,
                    selected: Vec::new(),
                    last: candidate.clone(),
                });
                self.paths.last_mut().unwrap()
            }
        };
        let WorkPath {
            checkpoint_ms: checkpoint,
            selected,
            last,
            ..
        } = entry;
        if *checkpoint > super::HORIZON_MS {
            return;
        }
        if candidate.elapsed_ms >= *checkpoint {
            selected.push(candidate.clone());
            while *checkpoint <= candidate.elapsed_ms && *checkpoint <= super::HORIZON_MS {
                *checkpoint += WORK_CHECKPOINT_MS;
            }
        }
        *last = candidate;
    }

    fn finish(self) -> Vec<Prediction> {
        let mut variants: Vec<Prediction> = Vec::new();
        for WorkPath {
            checkpoint_ms: checkpoint,
            mut selected,
            last,
            ..
        } in self.paths
        {
            if checkpoint <= super::HORIZON_MS
                && selected
                    .last()
                    .is_none_or(|old| old.actions != last.actions)
            {
                selected.push(last);
            }
            for candidate in selected {
                if !variants.iter().any(|old| old.actions == candidate.actions) {
                    variants.push(candidate);
                }
            }
        }
        variants
    }
}

pub(super) fn production_prefixes(
    initial: Prediction,
    prior_actions: usize,
    diagnostics: &mut [super::ProductionDecision],
) -> Result<Vec<Prediction>, SimulationError> {
    let calculated;
    let targets = match initial.citizen.production_targets() {
        Some(targets) => targets,
        None => {
            calculated = crate::production::ProductionTargets::calculate(
                &initial.citizen,
                initial.citizen.market_time_ms,
            )?;
            &calculated
        }
    };
    let mut recipes: Vec<_> = targets
        .batches()
        .filter_map(|(recipe, _)| {
            let profit = crate::production::recipe_profit(recipe, &initial.citizen)?;
            (profit > 0.0).then_some((
                recipe,
                profit / recipe.duration_ms(&initial.citizen).ok()? as f64,
            ))
        })
        .collect();
    recipes.sort_by(|(a, ap), (b, bp)| {
        bp.total_cmp(ap)
            .then_with(|| (*a as usize).cmp(&(*b as usize)))
    });
    let mut paths = WorkCheckpoints::default();
    let mut pending = vec![(Vec::new(), 0)];
    while let Some((order, mut index)) = pending.pop() {
        while index < recipes.len()
            && order.iter().filter(|&&r| r == recipes[index].0).count() as u64
                >= targets.remaining_batches(recipes[index].0)
        {
            index += 1;
        }
        let Some(&(recipe, _)) = recipes.get(index) else {
            continue;
        };
        let mut extended = order.clone();
        extended.push(recipe);
        let mut preparation_limit = false;
        let next = production_order_with_evidence(
            &initial,
            &extended,
            None,
            0,
            prior_actions,
            &mut preparation_limit,
        )?;
        if let Some(diagnostic) = diagnostics.iter_mut().find(|d| d.recipe == recipe) {
            diagnostic.prefix_attempted = true;
            diagnostic.preparation_limit_observed |= preparation_limit;
        }
        if next.is_empty() {
            pending.push((order, index + 1));
        } else {
            let can_extend = next
                .iter()
                .any(|(_, candidate)| candidate.elapsed_ms < super::HORIZON_MS);
            for (path, candidate) in next {
                paths.consider(path, candidate);
            }
            if can_extend {
                pending.push((extended, index));
            }
        }
    }
    Ok(paths.finish())
}

fn variants_from(
    initial: Prediction,
    goal: Effect,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let citizen = &initial.citizen;
    let mut variants: Vec<Prediction> = Vec::new();
    let mut consider = |candidate: Prediction| {
        // Identical primitive paths have identical predictions, regardless of the requested target.
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
        for target in [
            crate::FOOD_RESERVE_FULL_BONUS_NUTRITION,
            200.0,
            crate::FOOD_RESERVE_CAP_NUTRITION,
        ] {
            if citizen.food_nutrition() >= target {
                continue;
            }
            for candidate in replenish(&initial, target, prior_actions)? {
                consider(candidate);
            }
        }
        return Ok(variants);
    }
    let mut goal_actions: Vec<_> = ACTIONS
        .into_iter()
        .filter(|action| action.effects().contains(&goal))
        .collect();
    if goal == Effect::ListExcess && citizen.excess_value()? >= crate::production::MIN_LISTING_VALUE
    {
        goal_actions.push(CitizenAction::ListExcess);
    }
    for action in goal_actions {
        let candidates = order_variants(&initial, action, MEAL_NOURISHMENT, 0, prior_actions)?;
        for candidate in candidates {
            consider(candidate);
        }
        // Existing small meals remain useful when acquiring a full meal would delay relief.
        if action == CitizenAction::Eat
            && citizen.food_nutrition() < MEAL_NOURISHMENT
            && (prior_actions == 0 || citizen.food_nutrition() >= REPLAN_MIN_NUTRITION)
            && let Some(candidate) = initial.perform(action, prior_actions)?
        {
            consider(candidate);
        }
    }
    Ok(variants)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        market.list(seller, mill, Good::Bread, 1).unwrap();
        market.list(seller, market_place, Good::Flour, 100).unwrap();
        market.list(seller, market_place, Good::Wood, 25).unwrap();
        market
            .list(seller, market_place, Good::BerryPie, 1)
            .unwrap();
        (buyer.with_market(market), mill, market_place)
    }

    #[test]
    fn shopping_trip_visits_nearest_useful_stock_and_keeps_one_cooldown() {
        let (buyer, mill, market) = workplace_shopper();
        let initial = Prediction::new(&buyer, Cooldowns::default());
        let list =
            crate::marketplace::ShoppingList::new([(Good::Flour, 100), (Good::Wood, 25)]).unwrap();
        let trip = shopping_trip(&initial, list, 0, 0).unwrap().pop().unwrap();
        assert_eq!(
            trip.actions,
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
                trip.citizen.units(Good::Bread) == 1
                    && trip.citizen.units(Good::BerryPie) == 1
                    && trip.actions.len() == 4
            })
            .unwrap();
        assert_eq!(
            trip.citizen.food_nutrition(),
            Good::Bread.nutrition_per_unit().unwrap()
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
            shopping_trip(&initial, list, TRADE_DURATION_MS + 1, 0)
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

    fn food_purchase(citizen: &Citizen, good: Good) -> CitizenAction {
        CitizenAction::BuyAt {
            place: citizen
                .map()
                .public_place(crate::locations::Location::Market),
            list: crate::marketplace::ShoppingList::single(
                good,
                ((MEAL_NOURISHMENT - citizen.food_nutrition()).max(0.0)
                    / good.nutrition_per_unit().unwrap())
                .ceil() as crate::Quantity,
            ),
        }
    }

    fn relocate_stock(citizen: &Citizen) -> Citizen {
        let mut market = citizen.market().clone();
        let orders: Vec<_> = market.orders().cloned().collect();
        for order in orders {
            market
                .withdraw(order.seller, order.place, order.good, order.units)
                .unwrap();
            market
                .list(
                    order.seller,
                    citizen
                        .map()
                        .public_place(crate::locations::Location::Market),
                    order.good,
                    order.units,
                )
                .unwrap();
        }
        citizen.with_market(market)
    }

    #[test]
    fn exhausted_forage_targets_allow_food_and_recipe_preparation() {
        use crate::production::Recipe;
        let mut source = baker().with_berries(0).unwrap();
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
                .all(|v| !v.actions.contains(&forage))
        );
        for goal in [Effect::ReduceHunger, Effect::ReplenishReserves] {
            assert!(
                variants_after(&initial, goal)
                    .unwrap()
                    .iter()
                    .any(|v| v.actions.contains(&forage))
            );
        }
        let ingredients = crate::marketplace::ShoppingList::single(Good::Berries, 50);
        assert!(
            prepare_inputs(&initial, &ingredients, 0, 0)
                .unwrap()
                .iter()
                .any(|v| v.actions.contains(&forage) && v.citizen.berries_units() >= 50)
        );
    }

    #[test]
    fn forage_production_checkpoints_include_travel_and_keep_half_hour_primitives() {
        use crate::locations::{Map, Position};
        for (travel_ms, counts) in [(0, vec![2, 4, 6, 8]), (5 * 60_000, vec![2, 4, 6, 8])] {
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
            assert_eq!(variants.len(), 4);
            for (candidate, count) in variants.iter().zip(counts) {
                assert_eq!(
                    candidate
                        .actions
                        .iter()
                        .filter(|&&a| a == CitizenAction::Produce(crate::production::Recipe::Forage))
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
                        .all(|(action, duration)| *action
                            != CitizenAction::Produce(crate::production::Recipe::Forage)
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
    fn skilled_production_checkpoints_finish_the_first_crossing_batch() {
        use crate::production::{Recipe, Skill};
        let worker = bread_only(baker().with_skill(Skill::Baking, 24.6).unwrap());
        let initial = Prediction::new(&worker, Cooldowns::default());
        let variants = production_prefixes(initial.clone(), 0, &mut []).unwrap();
        assert_eq!(variants.len(), 4);
        for (index, candidate) in variants.iter().enumerate() {
            let checkpoint = (index as u64 + 1) * WORK_CHECKPOINT_MS;
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
                    .all(|action| *action == CitizenAction::Produce(Recipe::BakeBread))
            );
            let replay = production_order(
                &initial,
                &vec![Recipe::BakeBread; candidate.actions.len()],
                None,
                0,
                0,
            )
            .unwrap();
            assert_eq!(candidate.action_durations_ms, replay[0].action_durations_ms);
            assert_eq!(
                candidate.citizen.units(Good::Bread),
                replay[0].citizen.units(Good::Bread)
            );
            assert_eq!(candidate.average().unwrap(), replay[0].average().unwrap());
        }
    }

    #[test]
    fn long_production_jobs_deduplicate_checkpoints_and_finish_after_the_horizon() {
        use crate::production::Recipe;
        let mut worker = baker().with_good(Good::Bread, 10_000).unwrap();
        worker.refresh_production_targets(true).unwrap();
        let variants =
            production_prefixes(Prediction::new(&worker, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert_eq!(variants.len(), 3);
        assert_eq!(
            variants.iter().map(|p| p.actions.len()).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(variants.iter().all(|p| {
            p.actions
                .iter()
                .all(|a| *a == CitizenAction::Produce(Recipe::BakeBerryPie))
        }));
        let candidate = variants.last().unwrap();
        assert!(candidate.elapsed_ms > super::super::HORIZON_MS);
        assert!(
            candidate.elapsed_ms - candidate.action_durations_ms.last().unwrap()
                < 3 * WORK_CHECKPOINT_MS
        );
        let expected = candidate.actions.clone();
        let mut plan = super::super::empty_plan();
        super::super::search(candidate.clone(), super::super::HORIZON_MS, &mut plan).unwrap();
        assert_eq!(plan.actions, expected);
    }

    #[test]
    fn production_keeps_short_exhausted_caps_and_supply_paths() {
        use crate::production::{Recipe, Skill};
        let mut worker = bread_only(baker().with_skill(Skill::Baking, 24.6).unwrap())
            .with_good(Good::Water, 10_000)
            .unwrap();
        let batches =
            crate::production::DAILY_CAPACITY_MS / Recipe::BakeBread.duration_ms(&worker).unwrap();
        worker = worker.with_good(Good::Bread, (batches - 1) * 2).unwrap();
        worker.refresh_production_targets(true).unwrap();
        let variants =
            production_prefixes(Prediction::new(&worker, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(
            variants[0].actions,
            [CitizenAction::Produce(Recipe::BakeBread)]
        );
        assert!(variants[0].elapsed_ms < WORK_CHECKPOINT_MS);

        let worker = bread_only(
            buying_baker(100)
                .with_skill(Skill::Baking, 24.6)
                .unwrap()
                .with_good(Good::Water, 10_000)
                .unwrap(),
        );
        let variants =
            production_prefixes(Prediction::new(&worker, Cooldowns::default()), 0, &mut [])
                .unwrap();
        assert!(!variants.is_empty());
        assert!(
            variants
                .iter()
                .any(|candidate| candidate.elapsed_ms < WORK_CHECKPOINT_MS)
        );
        assert!(variants.iter().all(|candidate| {
            candidate
                .actions
                .iter()
                .filter(|&&a| a == CitizenAction::Produce(Recipe::BakeBread))
                .count()
                == 1
        }));
    }

    #[test]
    fn production_preserves_buying_and_gathering_acquisition_families() {
        use crate::production::Recipe;
        let worker = bread_only(buying_baker(2000));
        let initial = Prediction::new(&worker, Cooldowns::default());
        let variants = production_prefixes(initial.clone(), 0, &mut []).unwrap();
        for gather in [false, true] {
            let family: Vec<_> = variants
                .iter()
                .filter(|candidate| {
                    candidate
                        .actions
                        .contains(&CitizenAction::Produce(Recipe::FetchWater))
                        == gather
                })
                .collect();
            assert!(!family.is_empty());
            assert!(family.len() <= 4);
            let mut previous_count = 0;
            for candidate in family {
                let count = candidate
                    .actions
                    .iter()
                    .filter(|&&a| a == CitizenAction::Produce(Recipe::BakeBread))
                    .count();
                assert!(count > previous_count);
                previous_count = count;
                let exact = production_order(&initial, &vec![Recipe::BakeBread; count], None, 0, 0)
                    .unwrap();
                let replay = exact
                    .iter()
                    .find(|replay| replay.actions == candidate.actions)
                    .unwrap();
                assert_eq!(candidate.elapsed_ms, replay.elapsed_ms);
                assert_eq!(candidate.action_durations_ms, replay.action_durations_ms);
                assert_eq!(candidate.citizen.coins(), replay.citizen.coins());
                let CitizenAction::BuyAt { list, .. } = candidate
                    .actions
                    .iter()
                    .find(|a| matches!(a, CitizenAction::BuyAt { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                assert_eq!(list.units(Good::Flour), count as u64 * 100);
                assert_eq!(
                    list.units(Good::Water),
                    if gather { 0 } else { count as u64 * 100 }
                );
            }
        }
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
                assert_eq!(variant.citizen.units(Good::FlaxGarment), 0);
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
        let requirements = recipe_requirements(&recipes).unwrap();
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
        let progress = variants
            .iter()
            .find(|variant| {
                variant.actions.last() == Some(&CitizenAction::Produce(Recipe::MakeClothingBlock))
                    && variant.citizen.units(Good::FlaxBlock) >= 8
            })
            .unwrap();
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
            .list(
                crate::AgentId(uuid::Uuid::new_v4()),
                citizen
                    .map()
                    .public_place(crate::locations::Location::Market),
                Good::Berries,
                1000,
            )
            .unwrap();
        citizen.with_market(market)
    }

    fn full_meals(citizen: &Citizen) -> Vec<Prediction> {
        meals(citizen, MEAL_NOURISHMENT)
    }

    fn meals(citizen: &Citizen, nutrition: f64) -> Vec<Prediction> {
        prepare(
            &Prediction::new(citizen, Cooldowns::default()),
            CitizenAction::Eat,
            nutrition,
            0,
            0,
        )
        .unwrap()
        .into_iter()
        .filter_map(|ready| ready.perform(CitizenAction::Eat, 0).unwrap())
        .collect()
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
    fn production_prefixes_repeat_batches_stop_after_crossing_and_follow_caps() {
        use crate::production::{Recipe, recipe_profit};
        let mut worker = baker();
        let bread_rate = recipe_profit(Recipe::BakeBread, &worker).unwrap()
            / Recipe::BakeBread.duration_ms(&worker).unwrap() as f64;
        let pie_rate = recipe_profit(Recipe::BakeBerryPie, &worker).unwrap()
            / Recipe::BakeBerryPie.duration_ms(&worker).unwrap() as f64;
        let first = if bread_rate >= pie_rate {
            Recipe::BakeBread
        } else {
            Recipe::BakeBerryPie
        };
        let output = first.outputs()[0];
        let daily_batches =
            crate::production::DAILY_CAPACITY_MS / first.duration_ms(&worker).unwrap();
        worker = worker
            .with_good(output.0, output.1 * (daily_batches - 2))
            .unwrap();
        worker.refresh_production_targets(true).unwrap();
        let variants = variants_after(
            &Prediction::new(&worker, Cooldowns::default()),
            Effect::Production,
        )
        .unwrap();
        assert!(variants.len() >= 3);
        assert_eq!(
            variants[0].actions.last(),
            Some(&CitizenAction::Produce(first))
        );
        let longest = variants
            .iter()
            .max_by_key(|variant| variant.elapsed_ms)
            .unwrap();
        let produced: Vec<_> = longest
            .actions
            .iter()
            .filter_map(|action| {
                if let CitizenAction::Produce(recipe) = action {
                    Some(*recipe)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(&produced[..2], &[first, first]);
        assert_ne!(produced[2], first);
        assert!(longest.elapsed_ms >= GOAL_HORIZON_MS);
        assert!(longest.elapsed_ms - longest.action_durations_ms.last().unwrap() < GOAL_HORIZON_MS);
        assert!(variants.iter().all(|variant| variant.elapsed_ms
            - variant.action_durations_ms.last().unwrap()
            < GOAL_HORIZON_MS));
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
        assert!(longest.elapsed_ms >= GOAL_HORIZON_MS);
        assert!(longest.elapsed_ms - longest.action_durations_ms.last().unwrap() < GOAL_HORIZON_MS);
        let short = baker().with_good(Good::Water, 0).unwrap();
        let supplied = variants_after(
            &Prediction::new(&short, Cooldowns::default()),
            Effect::Production,
        )
        .unwrap();
        assert!(
            supplied
                .iter()
                .any(|variant| variant.actions.contains(&CitizenAction::Produce(
                    crate::production::Recipe::FetchWater
                )))
        );
        assert!(supplied.iter().all(|variant| variant.elapsed_ms
            - variant.action_durations_ms.last().unwrap()
            < GOAL_HORIZON_MS));
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
        let variants = production_order(&initial, &[Recipe::BakeBread; 2], None, 0, 0).unwrap();
        let candidate = variants
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.actions.as_slice(),
                    [
                        CitizenAction::BuyAt { .. },
                        CitizenAction::Produce(Recipe::BakeBread),
                        CitizenAction::Produce(Recipe::BakeBread)
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
        assert_eq!(candidate.citizen.units(Good::Bread), 4);
        assert_eq!(candidate.citizen.units(Good::Flour), 0);
        assert!(
            (candidate.citizen.skill_level(Skill::Baking)
                - source.skill_level(Skill::Baking)
                - 0.2)
                .abs()
                < 1e-12
        );
        assert!(candidate.action_durations_ms[2] < candidate.action_durations_ms[1]);
        assert!(
            candidate
                .perform(CitizenAction::Buy(list), 0)
                .unwrap()
                .is_none()
        );
        let one = production_order(&initial, &[Recipe::BakeBread], None, 0, 0).unwrap();
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
            &[Recipe::BakeBread, Recipe::BakeBerryPie],
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
                        CitizenAction::Produce(Recipe::BakeBread),
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
        assert_eq!(candidate.citizen.units(Good::Bread), 2);
        assert_eq!(
            candidate.citizen.units(Good::BerryPie),
            initial.citizen.units(Good::BerryPie) + 2
        );
    }

    #[test]
    fn reserve_production_procures_repeated_batches_and_counts_edible_inputs() {
        use crate::production::Recipe;
        let initial = Prediction::new(
            &buying_baker(2000).with_good(Good::BerryPie, 0).unwrap(),
            Cooldowns::default(),
        );
        let variants = replenish(&initial, 200.0, 0).unwrap();
        assert!(variants.iter().any(|candidate| matches!(candidate.actions.as_slice(), [CitizenAction::BuyAt { list, .. }, CitizenAction::Produce(Recipe::BakeBread), CitizenAction::Produce(Recipe::BakeBread)] if list.units(Good::Flour) == 200) && candidate.citizen.food_nutrition() >= 200.0));
        let source = buying_baker(2000)
            .with_good(Good::BerryPie, 0)
            .unwrap()
            .with_good(Good::Berries, 100)
            .unwrap();
        let variants = food_production_variants(
            &Prediction::new(&source, Cooldowns::default()),
            Recipe::BakeBerryPie,
            200.0,
            0,
            0,
        )
        .unwrap();
        assert!(variants.iter().any(|candidate| matches!(
            candidate.actions.as_slice(),
            [
                CitizenAction::BuyAt { .. },
                CitizenAction::Produce(Recipe::BakeBerryPie),
                CitizenAction::Produce(Recipe::BakeBerryPie)
            ]
        ) && candidate.citizen.food_nutrition() >= 200.0));
        assert!(
            variants
                .iter()
                .filter(|candidate| candidate
                    .actions
                    .iter()
                    .filter(|action| **action == CitizenAction::Produce(Recipe::BakeBerryPie))
                    .count()
                    == 1)
                .all(|candidate| candidate.citizen.food_nutrition() < 200.0)
        );
    }

    #[test]
    fn combined_procurement_keeps_only_the_final_crossing_batch() {
        use crate::production::Recipe;
        let mut initial = Prediction::new(&buying_baker(2000), Cooldowns::default());
        initial.elapsed_ms = GOAL_HORIZON_MS - 60 * 60_000;
        let one = production_order(&initial, &[Recipe::BakeBread], None, 0, 0).unwrap();
        assert!(
            one.iter()
                .any(|candidate| candidate.elapsed_ms >= GOAL_HORIZON_MS)
        );
        assert!(
            production_order(&initial, &[Recipe::BakeBread; 2], None, 0, 0)
                .unwrap()
                .is_empty()
        );
        initial.elapsed_ms = GOAL_HORIZON_MS;
        assert!(
            production_order(&initial, &[Recipe::BakeBread], None, 0, 0)
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
                &[Recipe::BakeBread; 2],
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
            !production_order(&initial, &[Recipe::BakeBread], None, 0, 0)
                .unwrap()
                .is_empty()
        );
        assert!(
            production_order(&initial, &[Recipe::BakeBread; 2], None, 0, 0)
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
                .withdraw(order.seller, order.place, Good::Berries, order.units)
                .unwrap();
        }
        let mut source = source
            .with_market(market)
            .with_good(Good::Berries, 0)
            .unwrap();
        source.refresh_production_targets(true).unwrap();
        let targets = source.production_targets().unwrap();
        assert!(targets.remaining_batches(Recipe::BakeBerryPie) > 0);
        assert!(targets.remaining_batches(Recipe::BakeBread) > 0);
        assert!(
            production_prefixes(Prediction::new(&source, Cooldowns::default()), 0, &mut [])
                .unwrap()
                .iter()
                .any(|candidate| candidate
                    .actions
                    .contains(&CitizenAction::Produce(Recipe::BakeBread)))
        );
    }

    #[test]
    fn gathering_order_bounds_follow_the_primitive_duration() {
        assert_eq!(gathering_segments(super::super::HORIZON_MS, 30 * 60_000), 8);
        assert_eq!(gathering_segments(super::super::HORIZON_MS, 60 * 60_000), 4);
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
            gathering_segments(super::super::HORIZON_MS, ACTION_DURATION_MS) as usize
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
        let hunger = best_variant_after(&funded, Effect::ReduceHunger)
            .unwrap()
            .unwrap();
        assert!(
            !hunger
                .actions
                .contains(&CitizenAction::Produce(crate::production::Recipe::Forage))
        );
        let eaten = orders[5].perform(CitizenAction::Eat, 0).unwrap().unwrap();
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
    fn full_meal_acquisition_requires_initial_food_but_immediate_partial_meals_remain() {
        let empty = hungry(0);
        assert!(full_meals(&empty).is_empty());
        assert!(
            variants_after(
                &Prediction::new(&empty, Cooldowns::default()),
                Effect::ReduceHunger
            )
            .unwrap()
            .is_empty()
        );
        let source = hungry(75);
        let acquired = full_meals(&source);
        assert!(acquired.iter().any(|v| {
            v.actions
                == [
                    vec![CitizenAction::Produce(crate::production::Recipe::Forage); 8],
                    vec![CitizenAction::Eat],
                ]
                .concat()
        }));
        let variants = variants_after(
            &Prediction::new(&source, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(
            variants
                .iter()
                .any(|v| v.actions == [CitizenAction::Eat] && v.elapsed_ms == 75_000)
        );
        for acquired in acquired {
            assert!(variants.iter().any(|v| v.actions == acquired.actions));
        }
    }

    #[test]
    fn full_target_combines_partial_inventory_and_purchases_without_capping_eating() {
        let funded = hungry(62).with_coins(10).unwrap();
        let variants = full_meals(&funded);
        let bought = variants
            .iter()
            .find(|v| v.actions == [food_purchase(&funded, Good::Berries), CitizenAction::Eat])
            .unwrap();
        assert_eq!(bought.elapsed_ms, TRADE_DURATION_MS + 155_000);
        assert_eq!(bought.citizen.berries_units(), 0);
        let rich = hungry(0).with_coins(100).unwrap();
        let variants = full_meals(&rich);
        let bought = variants
            .iter()
            .find(|v| v.actions == [food_purchase(&rich, Good::Berries), CitizenAction::Eat])
            .unwrap();
        assert_eq!(bought.elapsed_ms, TRADE_DURATION_MS + 155_000);
        let existing = variants_after(
            &Prediction::new(&hungry(124), Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(
            existing
                .iter()
                .any(|v| v.actions == [CitizenAction::Eat] && v.elapsed_ms == 124_000)
        );
    }

    #[test]
    fn requirements_discover_gathering_trading_and_mixed_supply_chains() {
        let source = hungry(95);
        let variants = full_meals(&source);
        let mut foraged = vec![CitizenAction::Produce(crate::production::Recipe::Forage); 6];
        foraged.push(CitizenAction::Eat);
        assert!(variants.iter().any(|v| v.actions == foraged));
        for variant in variants {
            assert!(variant.citizen.food_nutrition().is_finite());
            assert_eq!(variant.actions.last(), Some(&CitizenAction::Eat));
            assert!(variant.citizen.hunger() < source.hunger());
        }
        let paid = full_meals(&source.with_coins(9).unwrap());
        assert!(
            paid.iter()
                .any(|v| v.actions == [food_purchase(&source, Good::Berries), CitizenAction::Eat])
        );
    }

    #[test]
    fn goal_variants_retain_full_meal_suppliers_and_immediate_partial_meals() {
        let source = hungry(75).with_prices(crate::marketplace::Prices::new(100.0).unwrap());
        let variants = full_meals(&source);
        let best_score = variants
            .iter()
            .map(|v| v.average().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        let retained = variants_after(
            &Prediction::new(&source, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(retained.iter().any(|v| v.average().unwrap() == best_score));
        for variant in variants {
            assert!(retained.iter().any(|v| v.actions == variant.actions));
        }
        let immediate = variants_after(
            &Prediction::new(&hungry(10), Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(immediate.iter().any(|v| v.actions == [CitizenAction::Eat]));
    }

    #[test]
    fn conversion_rounding_does_not_add_gathering_prerequisites() {
        let source = hungry(0).with_coins(16).unwrap();
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen.actions,
            [food_purchase(&source, Good::Berries), CitizenAction::Eat]
        );
        assert!(chosen.citizen.coins() >= 0);
        assert_eq!(chosen.citizen.berries_units(), 0);
        assert!(chosen.citizen.market().trades().is_empty());
        let short = hungry(0).with_coins(9).unwrap();
        let full = full_meals(&short);
        assert!(!full.is_empty());
        assert!(full.iter().all(|v| {
            v.actions
                .contains(&CitizenAction::Produce(crate::production::Recipe::Forage))
        }));
    }

    #[test]
    fn preparation_limit_allows_final_activity_but_not_missing_prerequisites() {
        let variants = full_meals(&hungry(75));
        let full_forage = variants
            .iter()
            .find(|v| {
                v.actions
                    .iter()
                    .filter(|&&a| a == CitizenAction::Produce(crate::production::Recipe::Forage))
                    .count()
                    == 8
            })
            .unwrap();
        assert_eq!(full_forage.elapsed_ms, GOAL_HORIZON_MS + 155_000);
        assert!(!full_meals(&hungry(65)).iter().any(|v| {
            v.actions
                .iter()
                .filter(|&&a| a == CitizenAction::Produce(crate::production::Recipe::Forage))
                .count()
                == 9
        }));
        let sleep = best_variant(&hungry(0), Effect::ReduceTiredness, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(sleep.actions, [CitizenAction::Sleep]);
        assert_eq!(sleep.elapsed_ms, SLEEP_DURATION_MS);
        let impossible = Prediction::new(&hungry(0), Cooldowns::default());
        assert!(
            satisfy(
                &impossible,
                Requirement {
                    resource: Effect::Food,
                    amount: 50.0,
                    coins: None
                },
                GOAL_HORIZON_MS,
                0
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn final_goal_is_completed_after_the_outer_horizon() {
        let citizen = Citizen::with_needs(50.0, -100.0)
            .unwrap()
            .with_berries(75)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(100.0).unwrap());
        let mut prefix = Prediction::new(&citizen, Cooldowns::default());
        for _ in 0..7 {
            prefix = prefix.perform(CitizenAction::Wait, 0).unwrap().unwrap();
        }
        let mut goal = variants_after(&prefix, Effect::ReduceHunger)
            .unwrap()
            .into_iter()
            .find(|goal| goal.elapsed_ms > ACTION_DURATION_MS)
            .unwrap();
        assert!(goal.elapsed_ms > ACTION_DURATION_MS);
        goal.elapsed_ms += prefix.elapsed_ms;
        let mut actions = prefix.actions;
        actions.append(&mut goal.actions);
        goal.actions = actions;
        let expected = goal.actions.clone();
        let expected_score = goal.average().unwrap();
        let mut best = super::super::empty_plan();
        super::super::search(goal, super::super::HORIZON_MS, &mut best).unwrap();
        assert_eq!(best.actions, expected);
        assert_eq!(best.average_wellbeing, expected_score);
    }

    #[test]
    fn cooldowns_are_separate_start_at_completion_and_reset_for_new_plans() {
        let source = hungry(100).with_coins(100).unwrap();
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
        let eaten = bought.perform(CitizenAction::Eat, 0).unwrap().unwrap();
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
        let rate_per_ms = 100.0 / (24.0 * 60.0 * 60_000.0);
        let expected = -30.0 - 45_000.0 * rate_per_ms;
        assert!((direct.average().unwrap() - expected).abs() < 1e-12);
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
    fn reserve_targets_use_exact_baskets_and_complete_final_trades_after_the_limit() {
        use crate::locations::{Map, Position};
        let travel_ms = 2 * 60_000;
        let market_position = Position {
            x: travel_ms as f64 / crate::locations::WALK_MS_PER_METRE,
            y: 0.0,
        };
        let map = Map::new(Position::default(), Position::default(), market_position).unwrap();
        let buyer = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap()
            .with_coins(300)
            .unwrap()
            .with_good(Good::Bread, 1)
            .unwrap();
        let seller = Citizen::new(0.0).unwrap().with_map(map).unwrap();
        let mut market = buyer.market().clone();
        market
            .list(
                seller.id(),
                buyer.map().public_place(crate::locations::Location::Market),
                Good::Bread,
                1000,
            )
            .unwrap();
        let buyer = buyer.with_market(market);
        let mut initial = Prediction::new(&buyer, Cooldowns::default());
        initial.elapsed_ms = GOAL_HORIZON_MS - 3 * 60_000;
        for target in [100.0, 200.0, 300.0] {
            let variants = replenish(&initial, target, 0).unwrap();
            let variant = variants
                .iter()
                .find(|variant| {
                    (variant.citizen.food_nutrition() - target).abs() < 1e-9
                        && matches!(
                            variant.actions.as_slice(),
                            [CitizenAction::Travel(_), CitizenAction::BuyAt { .. }]
                        )
                })
                .expect("each stock target has a completing purchase");
            assert_eq!(
                variant.elapsed_ms,
                initial.elapsed_ms + travel_ms + TRADE_DURATION_MS
            );
            assert!(variant.elapsed_ms > GOAL_HORIZON_MS);
            let CitizenAction::BuyAt { list: basket, .. } = variant.actions[1] else {
                unreachable!()
            };
            assert_eq!(
                basket.units(Good::Bread),
                ((target - 50.0) / 50.0).ceil() as u64
            );
        }
    }

    #[test]
    fn reserve_acquisition_can_withdraw_own_food_and_omits_completed_targets() {
        let buyer = Citizen::new(0.0)
            .unwrap()
            .with_good(Good::Bread, 4)
            .unwrap();
        let listed = buyer
            .start_action(CitizenAction::List(Good::Bread, 4))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let variants = variants_after(
            &Prediction::new(&listed, Cooldowns::default()),
            Effect::ReplenishReserves,
        )
        .unwrap();
        assert!(
            variants
                .iter()
                .any(|variant| variant.actions == [CitizenAction::Withdraw(Good::Bread, 2)])
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.actions == [CitizenAction::Withdraw(Good::Bread, 4)])
        );
        let stocked = buyer.with_good(Good::Bread, 6).unwrap();
        assert!(
            variants_after(
                &Prediction::new(&stocked, Cooldowns::default()),
                Effect::ReplenishReserves
            )
            .unwrap()
            .is_empty()
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
        assert!(
            (gathered.average().unwrap() - (-20.0 + 0.05 + 0.05 * 10.0 * BERRY_NUTRITION_PER_GRAM))
                .abs()
                < 1e-12
        );
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
    fn market_food_path_is_compared_against_the_forest_detour() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 900.0, y: 0.0 },
            Position::default(),
            Position { x: 10.0, y: 0.0 },
        )
        .unwrap();
        let source = hungry(0)
            .with_coins(100)
            .unwrap()
            .with_map(map.clone())
            .unwrap();
        let source = relocate_stock(&source);
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen.actions,
            [
                CitizenAction::Travel(map.public_place(Location::Market)),
                food_purchase(&source, Good::Berries),
                CitizenAction::Eat
            ]
        );
        assert_eq!(
            chosen
                .citizen
                .market()
                .available_units(source.id(), Good::Berries),
            845
        );
        assert_eq!(
            source.market().available_units(source.id(), Good::Berries),
            1000
        );
        assert!(chosen.citizen.market().trades().is_empty());
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
    fn goal_routes_follow_prior_positions_and_charge_travel_once_per_visit() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 300.0, y: 400.0 },
            Position { x: -400.0, y: 0.0 },
            Position { x: 0.0, y: 600.0 },
        )
        .unwrap();
        let source = relocate_stock(&hungry(95).with_map(map.clone()).unwrap());
        let variants = full_meals(&source);
        let mut foraged = vec![CitizenAction::Travel(map.public_place(Location::Forest))];
        foraged.extend([CitizenAction::Produce(crate::production::Recipe::Forage); 6]);
        foraged.push(CitizenAction::Eat);
        let gathered = variants.iter().find(|v| v.actions == foraged).unwrap();
        assert_eq!(
            gathered.elapsed_ms,
            300_000 + 6 * ACTION_DURATION_MS + 155_000
        );
        let funded = source.with_coins(100).unwrap();
        let trade = full_meals(&funded)
            .into_iter()
            .find(|v| {
                v.actions
                    == [
                        CitizenAction::Travel(map.public_place(Location::Market)),
                        food_purchase(&funded, Good::Berries),
                        CitizenAction::Eat,
                    ]
            })
            .unwrap();
        assert_eq!(trade.elapsed_ms, 360_000 + TRADE_DURATION_MS + 155_000);
        assert_eq!(
            trade.citizen.position(),
            map.position(map.public_place(Location::Market))
        );
        assert_eq!(trade.score.elapsed_ms, trade.elapsed_ms);
        assert_eq!(source.position(), Position::default());
    }

    #[test]
    fn preparation_budget_includes_travel_but_excludes_the_final_activity() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 300.0, y: 400.0 },
            Position { x: -400.0, y: 0.0 },
            Position { x: 0.0, y: 600.0 },
        )
        .unwrap();
        let away = hungry(75)
            .with_map(map.clone())
            .unwrap()
            .with_position(map.position(map.public_place(Location::Forest)))
            .unwrap();
        let sleep = best_variant(&away, Effect::ReduceTiredness, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            sleep.actions,
            [CitizenAction::Travel(away.home()), CitizenAction::Sleep]
        );
        assert_eq!(sleep.elapsed_ms, 300_000 + SLEEP_DURATION_MS);
        let at_home = hungry(75).with_map(map.clone()).unwrap();
        assert!(!full_meals(&at_home).iter().any(|v| {
            v.actions
                .iter()
                .filter(|&&a| a == CitizenAction::Produce(crate::production::Recipe::Forage))
                .count()
                == 8
        }));
        assert!(full_meals(&away).iter().any(|v| {
            v.actions
                == [
                    vec![CitizenAction::Produce(crate::production::Recipe::Forage); 8],
                    vec![CitizenAction::Eat],
                ]
                .concat()
        }));
    }

    #[test]
    fn forage_production_uses_current_prices_and_expected_berry_yield() {
        for berries in [1.0, 4.0] {
            let source = Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_prices(crate::marketplace::Prices::new(berries).unwrap())
                .with_good(Good::Water, 10_000)
                .unwrap();
            let chosen = best_variant(&source, Effect::Production, Cooldowns::default(), 0)
                .unwrap()
                .unwrap();
            let segments = gathering_segments(super::super::HORIZON_MS, ACTION_DURATION_MS);
            assert_eq!(
                chosen.actions,
                vec![CitizenAction::Produce(crate::production::Recipe::Forage); segments as usize]
            );
            assert_eq!(chosen.citizen.prices(), source.prices());
            assert_eq!(chosen.citizen.berries_units(), 10 * segments);
        }
    }
    #[test]
    fn counted_market_food_supplies_a_complete_meal_without_mixing() {
        let mut market = crate::marketplace::Market::default();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        let source = hungry(0);
        let place = source
            .map()
            .public_place(crate::locations::Location::Market);
        market.list(seller, place, Good::Bread, 1).unwrap();
        market.list(seller, place, Good::BerryPie, 1).unwrap();
        let source = source.with_coins(100).unwrap().with_market(market.clone());
        let variants = full_meals(&source);
        let basket = variants
            .iter()
            .find(|variant| {
                matches!(
                    variant.actions.as_slice(),
                    [CitizenAction::BuyAt { .. }, CitizenAction::Eat]
                )
            })
            .unwrap();
        assert!(basket.elapsed_ms <= TRADE_DURATION_MS + 155_000);
        assert!(basket.citizen.hunger() < source.hunger() - 49.0);
        assert!(basket.citizen.market().trades().is_empty());
        assert_eq!(source.market(), &market);
        let start = std::time::Instant::now();
        let plan = super::super::plan(&source).unwrap();
        println!(
            "Mixed-food full plan searched in {:?} ({} actions)",
            start.elapsed(),
            plan.actions().len()
        );
        assert!(plan.average_wellbeing().is_finite());
    }
}
