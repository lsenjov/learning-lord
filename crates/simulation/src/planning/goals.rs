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
    IncreaseWealth,
    ReplenishReserves,
    Production,
    ListExcess,
}

#[derive(Clone, Copy, Debug)]
pub struct Requirement {
    pub resource: Effect,
    pub amount: f64,
}

impl CitizenAction {
    pub fn effects(self) -> &'static [Effect] {
        use Effect::*;
        match self {
            Self::Eat => &[ReduceHunger],
            Self::Sleep => &[ReduceTiredness],
            Self::Forage => &[Food, IncreaseWealth, ReplenishReserves],
            Self::BuyFood(good) if good.nutrition_per_gram().is_some() => {
                &[Food, ReplenishReserves]
            }
            Self::BuyFood(_) => &[],
            Self::Produce(recipe)
                if recipe
                    .outputs()
                    .iter()
                    .any(|(good, _)| good.nutrition_per_gram().is_some()) =>
            {
                &[Food, ReplenishReserves]
            }
            Self::Withdraw(good, _) if good.nutrition_per_gram().is_some() => {
                &[Food, ReplenishReserves]
            }
            Self::Buy(list)
                if list
                    .items()
                    .any(|(good, _)| good.nutrition_per_gram().is_some()) =>
            {
                &[Food, ReplenishReserves]
            }
            Self::Wait
            | Self::Travel(_)
            | Self::Produce(_)
            | Self::ListExcess
            | Self::List(..)
            | Self::Buy(..)
            | Self::Withdraw(..) => &[],
        }
    }

    /// Resources needed to supply the requested quantity during prediction.
    pub fn input_for(self, citizen: &Citizen, amount: f64) -> Option<Requirement> {
        let (resource, amount) = match self {
            Self::Eat => (Effect::Food, amount),
            Self::BuyFood(good) => (
                Effect::Coins,
                citizen
                    .market()
                    .purchase_cost(
                        citizen.id(),
                        good,
                        (amount - citizen.food_nutrition()).max(0.0) / good.nutrition_per_gram()?,
                    )
                    .unwrap_or(f64::INFINITY),
            ),
            Self::Buy(list) => (
                Effect::Coins,
                list.items()
                    .map(|(good, grams)| {
                        citizen
                            .market()
                            .purchase_cost(citizen.id(), good, grams)
                            .unwrap_or(f64::INFINITY)
                    })
                    .sum(),
            ),
            _ => return None,
        };
        Some(Requirement { resource, amount })
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
            | Self::Withdraw(..) => TRADE_DURATION_MS,
            _ => ACTION_DURATION_MS,
        }
    }
}

const ACTIONS: [CitizenAction; 6] = [
    CitizenAction::Eat,
    CitizenAction::Sleep,
    CitizenAction::Forage,
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
            CitizenAction::BuyFood(_) | CitizenAction::Buy(..) => self.buy,
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
            buy: if matches!(action, CitizenAction::BuyFood(_) | CitizenAction::Buy(..)) {
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
        if matches!(action, CitizenAction::BuyFood(_) | CitizenAction::Buy(_))
            && action.effects().contains(&Effect::Food)
            && citizen.food_nutrition() <= self.citizen.food_nutrition()
        {
            return Ok(None);
        }
        next.citizen = citizen;
        next.score.add(before, after, duration)?;
        next.goal_score.add(before, after, duration)?;
        if !matches!(action, CitizenAction::Forage | CitizenAction::Travel(_)) {
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
        Effect::Coins => citizen.coins(),
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
                    (requirement.amount - state.citizen.food_nutrition()).max(0.0)
                        / good.nutrition_per_gram().unwrap(),
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
            let nutrition = good.nutrition_per_gram().unwrap();
            let grams = state
                .citizen
                .market()
                .available_grams(state.citizen.id(), good)
                .min(remaining / nutrition);
            if grams > 0.0 {
                items.push((good, grams));
                remaining = (remaining - grams * nutrition).max(0.0);
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
            .any(|(good, _)| good.nutrition_per_gram().is_some())
        {
            actions.push(CitizenAction::Produce(recipe));
        }
    }
    for good in Good::FOOD {
        let grams = state
            .citizen
            .market()
            .listed_grams(state.citizen.id(), good);
        if grams > 0.0 {
            actions.push(CitizenAction::Withdraw(
                good,
                grams.min(
                    (requirement.amount - state.citizen.food_nutrition()).max(0.0)
                        / good.nutrition_per_gram().unwrap(),
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
    // Conversion dust should not introduce another gathering action. Execution still caps spending.
    let tolerance = 32.0 * f64::EPSILON * available.abs().max(requirement.amount.abs());
    if available >= requirement.amount
        || requirement.amount.is_finite() && requirement.amount - available <= tolerance
    {
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
    let gathering = matches!(action, CitizenAction::Forage);
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

fn prepare(
    state: &Prediction,
    action: CitizenAction,
    amount: f64,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let prepared = if let CitizenAction::Produce(recipe) = action {
        prepare_inputs(state, recipe, reserved_ms, prior_actions)?
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

fn prepare_inputs(
    state: &Prediction,
    recipe: crate::production::Recipe,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let mut states = vec![state.clone()];
    for &(good, grams) in recipe.inputs() {
        let mut next_states = Vec::new();
        for current in states {
            if current.elapsed_ms + reserved_ms > GOAL_HORIZON_MS {
                continue;
            }
            let missing = (grams - current.citizen.grams(good)).max(0.0);
            if missing == 0.0 {
                next_states.push(current);
                continue;
            }
            let listed = current
                .citizen
                .market()
                .listed_grams(current.citizen.id(), good)
                .min(missing);
            let mut current = current;
            if listed > 0.0 {
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
            if current.citizen.grams(good) < grams {
                let mut suppliers: Vec<_> = current
                    .citizen
                    .available_recipes()
                    .filter(|r| r.outputs().iter().any(|(output, _)| *output == good))
                    .map(CitizenAction::Produce)
                    .collect();
                if good == Good::Berries {
                    suppliers.push(CitizenAction::Forage);
                }
                for supplier in suppliers {
                    next_states.extend(gather_input(
                        &current,
                        supplier,
                        good,
                        grams,
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
            recipe
                .inputs()
                .iter()
                .map(|&(good, grams)| (good, (grams - state.citizen.grams(good)).max(0.0))),
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
            if recipe
                .inputs()
                .iter()
                .all(|&(good, grams)| bought.citizen.grams(good) >= grams)
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
    grams: f64,
    reserved_ms: u64,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    if state.elapsed_ms + reserved_ms >= GOAL_HORIZON_MS {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for next in order_variants(state, action, 0.0, reserved_ms, prior_actions)? {
        if next.elapsed_ms + reserved_ms > GOAL_HORIZON_MS
            || next.citizen.grams(good) <= state.citizen.grams(good)
        {
            continue;
        }
        if next.citizen.grams(good) >= grams {
            result.push(next);
        } else {
            result.extend(gather_input(
                &next,
                action,
                good,
                grams,
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

fn production_prefixes(
    initial: Prediction,
    prior_actions: usize,
) -> Result<Vec<Prediction>, SimulationError> {
    let Some(targets) = initial.citizen.production_targets() else {
        return Ok(Vec::new());
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
    let mut variants = Vec::new();
    let mut pending = vec![(initial, 0)];
    while let Some((state, mut index)) = pending.pop() {
        if state.elapsed_ms >= GOAL_HORIZON_MS {
            continue;
        }
        while index < recipes.len()
            && state
                .citizen
                .production_targets()
                .is_none_or(|targets| targets.remaining_batches(recipes[index].0) <= 0.0)
        {
            index += 1;
        }
        let Some(&(recipe, _)) = recipes.get(index) else {
            continue;
        };
        let next = order_variants(
            &state,
            CitizenAction::Produce(recipe),
            0.0,
            0,
            prior_actions,
        )?;
        if next.is_empty() {
            pending.push((state, index + 1));
        } else {
            for candidate in next {
                if !variants
                    .iter()
                    .any(|old: &Prediction| old.actions == candidate.actions)
                {
                    variants.push(candidate.clone());
                    pending.push((candidate, index));
                }
            }
        }
    }
    Ok(variants)
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
    if goal == Effect::Production {
        return production_prefixes(initial, prior_actions);
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
        for candidate in order_variants(&initial, action, MEAL_NOURISHMENT, 0, prior_actions)? {
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

    fn hungry(berries: f64) -> Citizen {
        let citizen = Citizen::with_needs(100.0, -100.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(1.0).unwrap())
            .with_berries(berries)
            .unwrap();
        let mut market = citizen.market().clone();
        market
            .list(crate::AgentId(uuid::Uuid::new_v4()), Good::Berries, 1000.0)
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
            .with_good(Good::Flour, 2000.0)
            .unwrap()
            .with_good(Good::Wood, 500.0)
            .unwrap()
            .with_good(Good::Water, 2000.0)
            .unwrap()
            .with_good(Good::Berries, 2000.0)
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
            .with_good(output.0, output.1 * (daily_batches - 2) as f64)
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
        let short = baker().with_good(Good::Water, 0.0).unwrap();
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

    #[test]
    fn gathering_order_bounds_follow_the_primitive_duration() {
        assert_eq!(gathering_segments(super::super::HORIZON_MS, 30 * 60_000), 8);
        assert_eq!(gathering_segments(super::super::HORIZON_MS, 60 * 60_000), 4);
    }

    #[test]
    fn gathering_order_metadata_survives_goals_and_nested_supply() {
        let initial = Prediction::new(&hungry(30.0), Cooldowns::default());
        let orders = order_variants(&initial, CitizenAction::Forage, 0.0, 0, 0).unwrap();
        assert_eq!(
            orders.len(),
            gathering_segments(super::super::HORIZON_MS, ACTION_DURATION_MS) as usize
        );
        let gathered = &orders[1];
        assert_eq!(gathered.actions, [CitizenAction::Forage; 2]);
        assert!(
            order_variants(gathered, CitizenAction::Forage, 0.0, 0, 0)
                .unwrap()
                .is_empty()
        );
        assert!(
            best_variant_after(gathered, Effect::IncreaseWealth)
                .unwrap()
                .is_none()
        );
        let mut funded = gathered.clone();
        funded.citizen = funded.citizen.with_coins(1.0).unwrap();
        let hunger = best_variant_after(&funded, Effect::ReduceHunger)
            .unwrap()
            .unwrap();
        assert!(!hunger.actions.contains(&CitizenAction::Forage));
        let eaten = orders[5].perform(CitizenAction::Eat, 0).unwrap().unwrap();
        assert!(
            !order_variants(&eaten, CitizenAction::Forage, 0.0, 0, 0)
                .unwrap()
                .is_empty()
        );
        let reset = Prediction::new(&gathered.citizen, Cooldowns::default());
        assert!(
            !order_variants(&reset, CitizenAction::Forage, 0.0, 0, 0)
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
        let source = hungry(0.0).with_map(map.clone()).unwrap();
        let initial = Prediction::new(
            &source,
            Cooldowns {
                eat: 100_000_000,
                buy: 0,
                sell: 0,
            },
        );
        let orders = order_variants(&initial, CitizenAction::Forage, 0.0, 0, 0).unwrap();
        let order = &orders[2];
        assert_eq!(
            order.actions,
            [
                CitizenAction::Travel(map.public_place(Location::Forest)),
                CitizenAction::Forage,
                CitizenAction::Forage,
                CitizenAction::Forage
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
        let empty = hungry(0.0);
        assert!(full_meals(&empty).is_empty());
        assert!(
            variants_after(
                &Prediction::new(&empty, Cooldowns::default()),
                Effect::ReduceHunger
            )
            .unwrap()
            .is_empty()
        );
        let source = hungry(75.0);
        let acquired = full_meals(&source);
        assert!(
            acquired.iter().any(|v| v.actions
                == [vec![CitizenAction::Forage; 8], vec![CitizenAction::Eat]].concat())
        );
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
        let funded = hungry(62.0).with_coins(0.093).unwrap();
        let variants = full_meals(&funded);
        let bought = variants
            .iter()
            .find(|v| {
                v.actions
                    == [
                        CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                        CitizenAction::Eat,
                    ]
            })
            .unwrap();
        assert_eq!(bought.elapsed_ms, TRADE_DURATION_MS + 155_000);
        assert_eq!(bought.citizen.berries_grams(), 0.0);
        let rich = hungry(0.0).with_coins(1.0).unwrap();
        let variants = full_meals(&rich);
        let bought = variants
            .iter()
            .find(|v| {
                v.actions
                    == [
                        CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                        CitizenAction::Eat,
                    ]
            })
            .unwrap();
        assert_eq!(bought.elapsed_ms, TRADE_DURATION_MS + 155_000);
        let existing = variants_after(
            &Prediction::new(&hungry(124.0), Cooldowns::default()),
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
        let source = hungry(95.0);
        let variants = full_meals(&source);
        let mut foraged = vec![CitizenAction::Forage; 6];
        foraged.push(CitizenAction::Eat);
        assert!(variants.iter().any(|v| v.actions == foraged));
        for variant in variants {
            assert!(variant.citizen.berries_grams() >= 0.0);
            assert_eq!(variant.actions.last(), Some(&CitizenAction::Eat));
            assert!(variant.citizen.hunger() < source.hunger());
        }
        let paid = full_meals(&source.with_coins(0.093).unwrap());
        assert!(paid.iter().any(|v| v.actions
            == [
                CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                CitizenAction::Eat
            ]));
    }

    #[test]
    fn goal_variants_retain_full_meal_suppliers_and_immediate_partial_meals() {
        let source = hungry(75.0).with_prices(crate::marketplace::Prices::new(1.0).unwrap());
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
            &Prediction::new(&hungry(10.0), Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        assert!(immediate.iter().any(|v| v.actions == [CitizenAction::Eat]));
    }

    #[test]
    fn conversion_rounding_does_not_add_gathering_prerequisites() {
        let source = hungry(0.0).with_coins(0.155).unwrap();
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen.actions,
            [
                CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                CitizenAction::Eat
            ]
        );
        assert!(chosen.citizen.coins() >= 0.0);
        assert!(chosen.citizen.berries_grams() < 1e-10);
        assert!(chosen.citizen.market().trades().is_empty());
        let short = hungry(0.0).with_coins(0.09).unwrap();
        let full = full_meals(&short);
        assert!(!full.is_empty());
        assert!(
            full.iter()
                .all(|v| v.actions.contains(&CitizenAction::Forage))
        );
    }

    #[test]
    fn preparation_limit_allows_final_activity_but_not_missing_prerequisites() {
        let variants = full_meals(&hungry(75.0));
        let full_forage = variants
            .iter()
            .find(|v| {
                v.actions
                    .iter()
                    .filter(|&&a| a == CitizenAction::Forage)
                    .count()
                    == 8
            })
            .unwrap();
        assert_eq!(full_forage.elapsed_ms, GOAL_HORIZON_MS + 155_000);
        assert!(!full_meals(&hungry(65.0)).iter().any(|v| {
            v.actions
                .iter()
                .filter(|&&a| a == CitizenAction::Forage)
                .count()
                == 9
        }));
        let sleep = best_variant(
            &hungry(0.0),
            Effect::ReduceTiredness,
            Cooldowns::default(),
            0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(sleep.actions, [CitizenAction::Sleep]);
        assert_eq!(sleep.elapsed_ms, SLEEP_DURATION_MS);
        let impossible = Prediction::new(&hungry(0.0), Cooldowns::default());
        assert!(
            satisfy(
                &impossible,
                Requirement {
                    resource: Effect::Food,
                    amount: 50.0
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
            .with_berries(75.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(1.0).unwrap());
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
        let source = hungry(100.0).with_coins(1.0).unwrap();
        let initial = Prediction::new(&source, Cooldowns::default());
        let generic = initial
            .perform(
                CitizenAction::Buy(crate::marketplace::ShoppingList::single(
                    Good::Berries,
                    10.0,
                )),
                0,
            )
            .unwrap()
            .unwrap();
        assert_eq!(generic.cooldowns.buy, TRADE_PLAN_COOLDOWN_MS);
        assert!(
            generic
                .perform(
                    CitizenAction::Buy(crate::marketplace::ShoppingList::single(
                        Good::Berries,
                        10.0
                    )),
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
            .perform(CitizenAction::List(Good::Berries, 100.0), 0)
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
                .perform(CitizenAction::List(Good::Berries, 10.0), 0)
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
        let expected = -10.0 - 45_000.0 * rate_per_ms;
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
        let next = best_variant_after(&prefix, Effect::IncreaseWealth)
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
            Effect::IncreaseWealth,
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
            .with_coins(3.0)
            .unwrap()
            .with_good(Good::Bread, 140.0)
            .unwrap();
        let seller = Citizen::new(0.0).unwrap().with_map(map).unwrap();
        let mut market = buyer.market().clone();
        market.list(seller.id(), Good::Bread, 1000.0).unwrap();
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
                            [CitizenAction::Travel(_), CitizenAction::Buy(_)]
                        )
                })
                .expect("each stock target has a completing purchase");
            assert_eq!(
                variant.elapsed_ms,
                initial.elapsed_ms + travel_ms + TRADE_DURATION_MS
            );
            assert!(variant.elapsed_ms > GOAL_HORIZON_MS);
            let CitizenAction::Buy(basket) = variant.actions[1] else {
                unreachable!()
            };
            assert_eq!(basket.grams(Good::Bread), (target - 70.0) / 0.5);
        }
    }

    #[test]
    fn reserve_acquisition_can_withdraw_own_food_and_omits_completed_targets() {
        let buyer = Citizen::new(0.0)
            .unwrap()
            .with_good(Good::Bread, 400.0)
            .unwrap();
        let listed = buyer
            .start_action(CitizenAction::List(Good::Bread, 400.0))
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
                .any(|variant| variant.actions == [CitizenAction::Withdraw(Good::Bread, 200.0)])
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.actions == [CitizenAction::Withdraw(Good::Bread, 400.0)])
        );
        let stocked = buyer.with_good(Good::Bread, 600.0).unwrap();
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
            .with_prices(crate::marketplace::Prices::new(1.0).unwrap());
        let gathered = Prediction::new(&citizen, Cooldowns::default())
            .perform(CitizenAction::Forage, 0)
            .unwrap()
            .unwrap();
        assert!(
            (gathered.average().unwrap() - (0.05 + 0.05 * 10.0 * BERRY_NUTRITION_PER_GRAM)).abs()
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
        let source = hungry(0.0)
            .with_coins(1.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap();
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen.actions,
            [
                CitizenAction::Travel(map.public_place(Location::Market)),
                CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                CitizenAction::Eat
            ]
        );
        assert_eq!(
            chosen
                .citizen
                .market()
                .available_grams(source.id(), Good::Berries),
            845.0
        );
        assert_eq!(
            source.market().available_grams(source.id(), Good::Berries),
            1000.0
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
        let source = hungry(95.0).with_map(map.clone()).unwrap();
        let variants = full_meals(&source);
        let mut foraged = vec![CitizenAction::Travel(map.public_place(Location::Forest))];
        foraged.extend([CitizenAction::Forage; 6]);
        foraged.push(CitizenAction::Eat);
        let gathered = variants.iter().find(|v| v.actions == foraged).unwrap();
        assert_eq!(
            gathered.elapsed_ms,
            300_000 + 6 * ACTION_DURATION_MS + 155_000
        );
        let funded = source.with_coins(1.0).unwrap();
        let trade = full_meals(&funded)
            .into_iter()
            .find(|v| {
                v.actions
                    == [
                        CitizenAction::Travel(map.public_place(Location::Market)),
                        CitizenAction::BuyFood(crate::marketplace::Good::Berries),
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
        let away = hungry(75.0)
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
        let at_home = hungry(75.0).with_map(map.clone()).unwrap();
        assert!(!full_meals(&at_home).iter().any(|v| {
            v.actions
                .iter()
                .filter(|&&a| a == CitizenAction::Forage)
                .count()
                == 8
        }));
        assert!(
            full_meals(&away).iter().any(|v| v.actions
                == [vec![CitizenAction::Forage; 8], vec![CitizenAction::Eat]].concat())
        );
    }

    #[test]
    fn wealth_goal_uses_current_prices_and_expected_berry_yield() {
        for berries in [1.0, 4.0] {
            let source = Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_prices(crate::marketplace::Prices::new(berries).unwrap());
            let chosen = best_variant(&source, Effect::IncreaseWealth, Cooldowns::default(), 0)
                .unwrap()
                .unwrap();
            let segments = gathering_segments(super::super::HORIZON_MS, ACTION_DURATION_MS);
            assert_eq!(
                chosen.actions,
                vec![CitizenAction::Forage; segments as usize]
            );
            assert_eq!(chosen.citizen.prices(), source.prices());
            assert_eq!(chosen.citizen.berries_grams(), 10.0 * segments as f64);
        }
    }
    #[test]
    fn mixed_market_food_supplies_a_full_meal_in_one_shopping_action() {
        let mut market = crate::marketplace::Market::default();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        market.list(seller, Good::Bread, 70.0).unwrap();
        market.list(seller, Good::BerryPie, 25.0).unwrap();
        let source = hungry(0.0)
            .with_coins(1.0)
            .unwrap()
            .with_market(market.clone());
        let variants = full_meals(&source);
        let basket = variants
            .iter()
            .find(|variant| {
                matches!(
                    variant.actions.as_slice(),
                    [CitizenAction::Buy(_), CitizenAction::Eat]
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
