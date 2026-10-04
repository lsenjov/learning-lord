use super::{EAT_PLAN_COOLDOWN_MS, REPLAN_MIN_NUTRITION};
use crate::{
    ACTION_DURATION_MS, BERRY_EATING_MS_PER_GRAM, BERRY_NUTRITION_PER_GRAM, Citizen, CitizenAction,
    MEAL_NOURISHMENT, SLEEP_DURATION_MS, SimulationError, TRADE_DURATION_MS, marketplace::Good,
};

pub const GOAL_HORIZON_MS: u64 = 4 * 60 * 60 * 1000;
pub const TRADE_PLAN_COOLDOWN_MS: u64 = 2 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Berries,
    Coins,
    Pebbles,
    ReduceHunger,
    ReduceTiredness,
    IncreaseWealth,
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
            Self::Forage => &[Berries, IncreaseWealth],
            Self::FindRocks => &[Pebbles, IncreaseWealth],
            Self::BuyBerries => &[Berries],
            Self::SellPebbles => &[Coins],
            Self::Wait | Self::Travel(_) => &[],
        }
    }

    /// Resources needed to supply the requested quantity during prediction.
    pub fn input_for(self, citizen: &Citizen, amount: f64) -> Option<Requirement> {
        let (resource, amount) = match self {
            Self::Eat => (Effect::Berries, amount / BERRY_NUTRITION_PER_GRAM),
            Self::BuyBerries => (
                Effect::Coins,
                citizen
                    .prices()
                    .value(Good::Berries, (amount - citizen.berries_grams()).max(0.0)),
            ),
            Self::SellPebbles => (
                Effect::Pebbles,
                (amount - citizen.coins()).max(0.0) / citizen.prices().coins_per_kg(Good::Pebbles)
                    * 1000.0,
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
            Self::BuyBerries | Self::SellPebbles => TRADE_DURATION_MS,
            _ => ACTION_DURATION_MS,
        }
    }
}

const ACTIONS: [CitizenAction; 6] = [
    CitizenAction::Eat,
    CitizenAction::Sleep,
    CitizenAction::Forage,
    CitizenAction::FindRocks,
    CitizenAction::BuyBerries,
    CitizenAction::SellPebbles,
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
            CitizenAction::BuyBerries => self.buy,
            CitizenAction::SellPebbles => self.sell,
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
            buy: if action == CitizenAction::BuyBerries {
                TRADE_PLAN_COOLDOWN_MS
            } else {
                self.buy.saturating_sub(duration)
            },
            sell: if action == CitizenAction::SellPebbles {
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
        next.citizen = citizen;
        next.score.add(before, after, duration)?;
        next.goal_score.add(before, after, duration)?;
        if !matches!(
            action,
            CitizenAction::Forage | CitizenAction::FindRocks | CitizenAction::Travel(_)
        ) {
            next.last_gathering_order = None;
        }
        next.actions.push(action);
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
        Effect::Berries => citizen.berries_grams(),
        Effect::Coins => citizen.coins(),
        Effect::Pebbles => citizen.pebbles_grams(),
        _ => unreachable!("only inventory resources are prerequisites"),
    }
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
    for action in ACTIONS
        .into_iter()
        .filter(|action| action.effects().contains(&requirement.resource))
    {
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
    let gathering = matches!(action, CitizenAction::Forage | CitizenAction::FindRocks);
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
    let prepared = match action.input_for(&state.citizen, amount) {
        Some(requirement) => satisfy(state, requirement, reserved_ms, prior_actions)?,
        None => vec![state.clone()],
    };
    let mut variants = Vec::new();
    for mut ready in prepared {
        if let Some(location) = action.required_location()
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
    for action in ACTIONS
        .into_iter()
        .filter(|action| action.effects().contains(&goal))
    {
        let targets: &[f64] = if action == CitizenAction::Eat {
            &[30.0, MEAL_NOURISHMENT]
        } else {
            &[MEAL_NOURISHMENT]
        };
        for &target in targets {
            for candidate in order_variants(&initial, action, target, 0, prior_actions)? {
                consider(candidate);
            }
        }
        // Existing small meals remain useful when acquiring a full meal would delay relief.
        if action == CitizenAction::Eat
            && citizen.berries_grams() < MEAL_NOURISHMENT / BERRY_NUTRITION_PER_GRAM
            && (prior_actions == 0
                || citizen.berries_grams() * BERRY_NUTRITION_PER_GRAM >= REPLAN_MIN_NUTRITION)
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
        Citizen::with_needs(100.0, -100.0)
            .unwrap()
            .with_berries(berries)
            .unwrap()
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

    #[test]
    fn gathering_order_bounds_follow_the_primitive_duration() {
        assert_eq!(gathering_segments(super::super::HORIZON_MS, 30 * 60_000), 8);
        assert_eq!(gathering_segments(super::super::HORIZON_MS, 60 * 60_000), 4);
    }

    #[test]
    fn gathering_order_metadata_survives_goals_and_nested_supply() {
        let initial = Prediction::new(&hungry(0.0), Cooldowns::default());
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
        let wealth = best_variant_after(gathered, Effect::IncreaseWealth)
            .unwrap()
            .unwrap();
        assert!(
            wealth
                .actions
                .iter()
                .all(|action| *action == CitizenAction::FindRocks)
        );
        let hunger = best_variant_after(gathered, Effect::ReduceHunger)
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
        let source = hungry(0.0).with_map(map).unwrap();
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
                CitizenAction::Travel(Location::Forest),
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
    fn smaller_meal_is_reachable_from_empty_and_full_meal_after_gathering() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 300.0, y: 400.0 },
            Position { x: -400.0, y: 0.0 },
            Position { x: 0.0, y: 600.0 },
        )
        .unwrap();
        let source = hungry(0.0).with_map(map).unwrap();
        assert!(full_meals(&source).is_empty());
        let mut expected = vec![CitizenAction::Travel(Location::Forest)];
        expected.extend([CitizenAction::Forage; 6]);
        expected.push(CitizenAction::Eat);
        let variants = meals(&source, 30.0);
        let foraged = variants.iter().find(|v| v.actions == expected).unwrap();
        assert_eq!(
            foraged.elapsed_ms,
            5 * 60_000 + 6 * ACTION_DURATION_MS + 60_000
        );
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert!(variants.iter().any(|v| v.actions == chosen.actions));
        assert!(
            super::super::plan(&source)
                .unwrap()
                .decision()
                .unwrap()
                .candidates
                .iter()
                .find(|candidate| candidate.goal == Effect::ReduceHunger)
                .unwrap()
                .forecast
                .is_some()
        );

        let mut prefix = Prediction::new(&source, Cooldowns::default())
            .perform(CitizenAction::Travel(Location::Forest), 0)
            .unwrap()
            .unwrap();
        for _ in 0..4 {
            prefix = prefix.perform(CitizenAction::Forage, 0).unwrap().unwrap();
        }
        assert!(prefix.elapsed_ms >= super::super::COMMITMENT_MS);
        assert_eq!(prefix.citizen.berries_grams(), 40.0);
        let mut expected_full = vec![CitizenAction::Forage; 6];
        expected_full.push(CitizenAction::Eat);
        assert!(
            full_meals(&prefix.citizen)
                .iter()
                .any(|v| v.actions == expected_full)
        );
    }

    #[test]
    fn smaller_target_allows_partial_purchase_without_capping_eating_or_buying() {
        let funded = hungry(0.0).with_coins(0.06).unwrap();
        let variants = meals(&funded, 30.0);
        let bought = variants
            .iter()
            .find(|v| v.actions == [CitizenAction::BuyBerries, CitizenAction::Eat])
            .unwrap();
        assert_eq!(bought.elapsed_ms, TRADE_DURATION_MS + 60_000);
        assert_eq!(bought.citizen.berries_grams(), 0.0);
        let rich = hungry(0.0).with_coins(1.0).unwrap();
        let variants = meals(&rich, 30.0);
        let bought = variants
            .iter()
            .find(|v| v.actions == [CitizenAction::BuyBerries, CitizenAction::Eat])
            .unwrap();
        assert_eq!(bought.elapsed_ms, TRADE_DURATION_MS + 100_000);
        let existing = meals(&hungry(80.0), 30.0);
        assert!(
            existing
                .iter()
                .any(|v| v.actions == [CitizenAction::Eat] && v.elapsed_ms == 80_000)
        );
    }

    #[test]
    fn requirements_discover_gathering_trading_and_mixed_supply_chains() {
        let source = hungry(40.0);
        let variants = full_meals(&source);
        let mut foraged = vec![CitizenAction::Forage; 6];
        foraged.push(CitizenAction::Eat);
        assert!(variants.iter().any(|v| v.actions == foraged));
        let mut traded = vec![CitizenAction::FindRocks; 6];
        traded.extend([
            CitizenAction::SellPebbles,
            CitizenAction::BuyBerries,
            CitizenAction::Eat,
        ]);
        assert!(variants.iter().any(|v| v.actions == traded));
        assert!(
            variants
                .iter()
                .any(|v| v.actions.contains(&CitizenAction::Forage)
                    && v.actions.contains(&CitizenAction::BuyBerries))
        );
        for variant in variants {
            assert!(variant.citizen.berries_grams() >= 0.0);
            assert_eq!(variant.actions.last(), Some(&CitizenAction::Eat));
            assert!(variant.citizen.hunger() < source.hunger());
        }
        let paid = full_meals(&source.with_coins(0.06).unwrap());
        assert!(
            paid.iter()
                .any(|v| v.actions == [CitizenAction::BuyBerries, CitizenAction::Eat])
        );
    }

    #[test]
    fn goal_variants_retain_every_supplier_target_and_immediate_meal() {
        let source = hungry(0.0).with_prices(crate::marketplace::Prices::new(1.0, 4.0).unwrap());
        let mut variants = meals(&source, 30.0);
        variants.extend(full_meals(&source));
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
        let source = hungry(0.0)
            .with_coins(0.00069)
            .unwrap()
            .with_pebbles(49.655)
            .unwrap();
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen.actions,
            [
                CitizenAction::SellPebbles,
                CitizenAction::BuyBerries,
                CitizenAction::Eat
            ]
        );
        assert!(chosen.citizen.coins() >= 0.0);
        assert_eq!(chosen.citizen.pebbles_grams(), 0.0);
        assert!(chosen.citizen.berries_grams() < 1e-10);
        let short = hungry(0.0).with_pebbles(49.0).unwrap();
        let full = full_meals(&short);
        assert!(!full.is_empty());
        assert!(full.iter().all(
            |variant| variant.actions.contains(&CitizenAction::FindRocks)
                || variant.actions.contains(&CitizenAction::Forage)
        ));
    }

    #[test]
    fn preparation_limit_allows_final_activity_but_not_missing_prerequisites() {
        let variants = full_meals(&hungry(20.0));
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
        assert_eq!(full_forage.elapsed_ms, GOAL_HORIZON_MS + 100_000);
        assert!(!full_meals(&hungry(10.0)).iter().any(|v| {
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
                    resource: Effect::Berries,
                    amount: 100.0
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
            .with_pebbles(5.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(1.0, 4.0).unwrap());
        let mut prefix = Prediction::new(&citizen, Cooldowns::default());
        for _ in 0..7 {
            prefix = prefix.perform(CitizenAction::Wait, 0).unwrap().unwrap();
        }
        let mut goal = best_variant_after(&prefix, Effect::ReduceHunger)
            .unwrap()
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
        let source = hungry(0.0).with_pebbles(100.0).unwrap();
        let initial = Prediction::new(&source, Cooldowns::default());
        let sold = initial
            .perform(CitizenAction::SellPebbles, 0)
            .unwrap()
            .unwrap();
        assert_eq!(sold.cooldowns.sell, TRADE_PLAN_COOLDOWN_MS);
        let bought = sold.perform(CitizenAction::BuyBerries, 0).unwrap().unwrap();
        assert_eq!(bought.cooldowns.buy, TRADE_PLAN_COOLDOWN_MS);
        assert_eq!(
            bought.cooldowns.sell,
            TRADE_PLAN_COOLDOWN_MS - TRADE_DURATION_MS
        );
        let eaten = bought.perform(CitizenAction::Eat, 0).unwrap().unwrap();
        assert_eq!(eaten.cooldowns.eat, EAT_PLAN_COOLDOWN_MS);
        assert!(
            eaten
                .perform(CitizenAction::BuyBerries, 0)
                .unwrap()
                .is_none()
        );
        let mut elapsed = eaten.clone();
        for _ in 0..4 {
            elapsed = elapsed
                .perform(CitizenAction::FindRocks, 0)
                .unwrap()
                .unwrap();
        }
        assert!(
            elapsed
                .perform(CitizenAction::BuyBerries, 0)
                .unwrap()
                .is_some()
        );
        assert!(
            elapsed
                .perform(CitizenAction::SellPebbles, 0)
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
                .perform(CitizenAction::BuyBerries, 0)
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
            .with_map(map)
            .unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let direct = initial
            .perform(CitizenAction::Travel(Location::Forest), 0)
            .unwrap()
            .unwrap();
        let middle = initial
            .perform(CitizenAction::Travel(Location::River), 0)
            .unwrap()
            .unwrap();
        assert_eq!(middle.score.elapsed_ms, 30_000);
        let split = middle
            .perform(CitizenAction::Travel(Location::Forest), 0)
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
            .with_map(map)
            .unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let prefix = initial
            .perform(CitizenAction::Travel(Location::Forest), 0)
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
    fn gathering_completion_wealth_is_spread_across_the_action() {
        let citizen = Citizen::with_needs(-50.0, -100.0).unwrap();
        let gathered = Prediction::new(&citizen, Cooldowns::default())
            .perform(CitizenAction::Forage, 0)
            .unwrap()
            .unwrap();
        assert!((gathered.average().unwrap() - 0.05).abs() < 1e-12);
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
    fn exported_market_state_prefers_direct_rocks_over_the_forest_detour() {
        use crate::locations::{Location, Map, Position};
        use crate::marketplace::Prices;
        let map = Map::new(
            Position {
                x: -725.1376571747487,
                y: 97.96764606609361,
            },
            Position {
                x: -10.912906801495751,
                y: -727.3559818665126,
            },
            Position {
                x: 233.53900050684547,
                y: -686.1697663686859,
            },
        )
        .unwrap();
        // The dump was captured 105,740 ms into the first trip; restore its planning state.
        let need_growth = 105_740.0 * 100.0 / 86_400_000.0;
        let source = Citizen::with_needs(
            16.671912174402078 - need_growth,
            -45.326035879629764 - need_growth,
        )
        .unwrap()
        .with_map(map)
        .unwrap()
        .with_position(map.position(Location::River))
        .unwrap()
        .with_prices(Prices::new(1.2090152088522885, 3.905220675837458).unwrap())
        .with_pebbles(19.454608762314617)
        .unwrap();
        let mut prefix = Prediction::new(&source, Cooldowns::default());
        for action in [
            CitizenAction::Travel(Location::Market),
            CitizenAction::SellPebbles,
            CitizenAction::BuyBerries,
            CitizenAction::Eat,
        ] {
            prefix = prefix.perform(action, 0).unwrap().unwrap();
        }
        assert_eq!(prefix.elapsed_ms, 811_580);
        let forage = prefix
            .perform(CitizenAction::Travel(Location::Forest), 0)
            .unwrap()
            .unwrap()
            .perform(CitizenAction::Forage, 0)
            .unwrap()
            .unwrap();
        let rocks = prefix
            .perform(CitizenAction::Travel(Location::River), 0)
            .unwrap()
            .unwrap()
            .perform(CitizenAction::FindRocks, 0)
            .unwrap()
            .unwrap();
        let local_forage = forage.score.mean * forage.score.elapsed_ms as f64
            - prefix.score.mean * prefix.score.elapsed_ms as f64;
        let local_rocks = rocks.score.mean * rocks.score.elapsed_ms as f64
            - prefix.score.mean * prefix.score.elapsed_ms as f64;
        let forage_average = local_forage / (forage.elapsed_ms - prefix.elapsed_ms) as f64;
        let rocks_average = local_rocks / (rocks.elapsed_ms - prefix.elapsed_ms) as f64;
        assert!(rocks_average > forage_average);
        let chosen = best_variant_after(&prefix, Effect::IncreaseWealth)
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen.actions.first(),
            Some(&CitizenAction::Travel(Location::River))
        );
        assert!(
            chosen.actions[1..]
                .iter()
                .all(|action| *action == CitizenAction::FindRocks)
        );
        assert!(chosen.actions.len() > 2);
        assert!(chosen.goal_average().unwrap() > rocks_average);
        let fresh = best_variant(&prefix.citizen, Effect::IncreaseWealth, prefix.cooldowns, 0)
            .unwrap()
            .unwrap();
        assert_eq!(chosen.actions, fresh.actions);
        assert_eq!(
            chosen.goal_average().unwrap(),
            fresh.goal_average().unwrap()
        );
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
        let source = hungry(40.0).with_map(map).unwrap();
        let variants = full_meals(&source);
        let mut foraged = vec![CitizenAction::Travel(Location::Forest)];
        foraged.extend([CitizenAction::Forage; 6]);
        foraged.push(CitizenAction::Eat);
        let gathered = variants.iter().find(|v| v.actions == foraged).unwrap();
        assert_eq!(
            gathered.elapsed_ms,
            300_000 + 6 * ACTION_DURATION_MS + 100_000
        );
        let mut traded = vec![CitizenAction::Travel(Location::River)];
        traded.extend([CitizenAction::FindRocks; 6]);
        traded.extend([
            CitizenAction::Travel(Location::Market),
            CitizenAction::SellPebbles,
            CitizenAction::BuyBerries,
            CitizenAction::Eat,
        ]);
        let trade = variants.iter().find(|v| v.actions == traded).unwrap();
        let river_to_market = (map
            .position(Location::River)
            .distance(map.position(Location::Market))
            * crate::locations::WALK_MS_PER_METRE)
            .ceil() as u64;
        assert_eq!(
            trade.elapsed_ms,
            240_000 + 6 * ACTION_DURATION_MS + river_to_market + 2 * TRADE_DURATION_MS + 100_000
        );
        assert_eq!(trade.citizen.position(), map.position(Location::Market));
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
        let away = hungry(20.0)
            .with_map(map)
            .unwrap()
            .with_position(map.position(Location::Forest))
            .unwrap();
        let sleep = best_variant(&away, Effect::ReduceTiredness, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            sleep.actions,
            [CitizenAction::Travel(Location::House), CitizenAction::Sleep]
        );
        assert_eq!(sleep.elapsed_ms, 300_000 + SLEEP_DURATION_MS);
        let at_home = hungry(20.0).with_map(map).unwrap();
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
    fn wealth_goal_uses_current_prices_and_half_pebble_yield() {
        use crate::marketplace::Prices;
        for (berries, pebbles, expected) in [
            (1.0, 1.0, CitizenAction::Forage),
            (1.0, 4.0, CitizenAction::FindRocks),
        ] {
            let source = Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_prices(Prices::new(berries, pebbles).unwrap());
            let chosen = best_variant(&source, Effect::IncreaseWealth, Cooldowns::default(), 0)
                .unwrap()
                .unwrap();
            let segments = gathering_segments(super::super::HORIZON_MS, ACTION_DURATION_MS);
            assert_eq!(chosen.actions, vec![expected; segments as usize]);
            assert_eq!(chosen.citizen.prices(), source.prices());
            if expected == CitizenAction::FindRocks {
                assert_eq!(chosen.citizen.pebbles_grams(), 5.0 * segments as f64);
            } else {
                assert_eq!(chosen.citizen.berries_grams(), 10.0 * segments as f64);
            }
        }
    }
}
