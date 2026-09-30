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
    score: SampledWellbeing,
    goal_score: SampledWellbeing,
    pub cooldowns: Cooldowns,
}

impl Prediction {
    pub fn new(citizen: &Citizen, cooldowns: Cooldowns) -> Self {
        Self {
            citizen: citizen.clone(),
            actions: Vec::new(),
            elapsed_ms: 0,
            score: SampledWellbeing::default(),
            goal_score: SampledWellbeing::default(),
            cooldowns,
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
        let mut next = self.clone();
        next.citizen = started;
        let mut remaining = duration;
        while remaining > 0 {
            let step = remaining.min(SAMPLE_INTERVAL_MS - next.score.pending_ms);
            next.citizen = next.citizen.advance_predicted(step)?;
            next.score.pending_ms += step;
            next.goal_score.pending_ms += step;
            remaining -= step;
            if next.score.pending_ms == SAMPLE_INTERVAL_MS {
                let value = next.citizen.personal_wellbeing()?;
                next.score.sample(value)?;
                next.goal_score.sample(value)?;
            }
        }
        next.actions.push(action);
        next.elapsed_ms += duration;
        next.cooldowns = self.cooldowns.after(action, duration);
        Ok(Some(next))
    }

    pub fn average(&self) -> Result<f64, SimulationError> {
        self.score.average(self.citizen.personal_wellbeing()?)
    }

    fn goal_average(&self) -> Result<f64, SimulationError> {
        self.goal_score.average(self.citizen.personal_wellbeing()?)
    }
}

const SAMPLE_INTERVAL_MS: u64 = 60_000;

#[derive(Clone, Copy, Default)]
struct SampledWellbeing {
    mean: f64,
    sampled_ms: u64,
    pending_ms: u64,
}

impl SampledWellbeing {
    fn sample(&mut self, value: f64) -> Result<(), SimulationError> {
        if self.pending_ms == 0 {
            return Ok(());
        }
        let total = self.sampled_ms + self.pending_ms;
        let weight = self.pending_ms as f64 / total as f64;
        // Avoid overflow when finite scores have opposite signs.
        self.mean = if self.mean.is_sign_positive() == value.is_sign_positive() {
            self.mean + (value - self.mean) * weight
        } else {
            self.mean * (self.sampled_ms as f64 / total as f64) + value * weight
        };
        if !self.mean.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        self.sampled_ms = total;
        self.pending_ms = 0;
        Ok(())
    }

    fn average(mut self, endpoint: f64) -> Result<f64, SimulationError> {
        // A provisional endpoint must not become an extra sample if the plan continues.
        self.sample(endpoint)?;
        Ok(self.mean)
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
        let prepared = prepare(
            state,
            action,
            requirement.amount,
            reserved_ms + action.predicted_duration(),
            prior_actions,
        )?;
        for ready in prepared {
            if let Some(next) = ready.perform(action, prior_actions)?
                && quantity(&next.citizen, requirement.resource)
                    > quantity(&state.citizen, requirement.resource)
            {
                variants.extend(satisfy(&next, requirement, reserved_ms, prior_actions)?);
            }
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

pub(super) fn best_variant(
    citizen: &Citizen,
    goal: Effect,
    cooldowns: Cooldowns,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    best_variant_from(Prediction::new(citizen, cooldowns), goal, prior_actions)
}

pub(super) fn best_variant_after(
    prefix: &Prediction,
    goal: Effect,
) -> Result<Option<Prediction>, SimulationError> {
    let mut initial = Prediction::new(&prefix.citizen, prefix.cooldowns);
    initial.score = prefix.score;
    best_variant_from(initial, goal, prefix.actions.len())
}

fn best_variant_from(
    initial: Prediction,
    goal: Effect,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    let citizen = &initial.citizen;
    let mut best: Option<Prediction> = None;
    let mut consider = |candidate: Prediction| -> Result<(), SimulationError> {
        let candidate_score = candidate.goal_average()?;
        if best
            .as_ref()
            .map(|old| old.goal_average())
            .transpose()?
            .is_none_or(|score| candidate_score > score)
        {
            best = Some(candidate);
        }
        Ok(())
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
            for ready in prepare(&initial, action, target, 0, prior_actions)? {
                if let Some(candidate) = ready.perform(action, prior_actions)? {
                    consider(candidate)?;
                }
            }
        }
        // Existing small meals remain useful when acquiring a full meal would delay relief.
        if action == CitizenAction::Eat
            && citizen.berries_grams() < MEAL_NOURISHMENT / BERRY_NUTRITION_PER_GRAM
            && (prior_actions == 0
                || citizen.berries_grams() * BERRY_NUTRITION_PER_GRAM >= REPLAN_MIN_NUTRITION)
            && let Some(candidate) = initial.perform(action, prior_actions)?
        {
            consider(candidate)?;
        }
    }
    Ok(best)
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
            assert_eq!(variant.citizen.berries_grams(), 0.0);
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
    fn local_variant_winner_is_selected_before_goal_sequencing() {
        let source = hungry(0.0).with_prices(crate::marketplace::Prices::new(1.0, 4.0).unwrap());
        let mut variants = meals(&source, 30.0);
        variants.extend(full_meals(&source));
        let best_score = variants
            .iter()
            .map(|v| v.average().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        let chosen = best_variant(&source, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(chosen.average().unwrap(), best_score);
        assert!(variants.iter().any(|v| v.actions == chosen.actions));
        let immediate = best_variant(&hungry(10.0), Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert_eq!(immediate.actions, [CitizenAction::Eat]);
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
    fn action_boundaries_do_not_add_samples_and_final_fraction_is_weighted() {
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
        let _provisional = middle.average().unwrap();
        assert_eq!(middle.score.sampled_ms, 0);
        let split = middle
            .perform(CitizenAction::Travel(Location::Forest), 0)
            .unwrap()
            .unwrap();
        assert!((direct.average().unwrap() - split.average().unwrap()).abs() < 1e-12);
        let rate_per_ms = 100.0 / (24.0 * 60.0 * 60_000.0);
        let expected = ((-10.0 - 60_000.0 * rate_per_ms) * 60_000.0
            + (-10.0 - 90_000.0 * rate_per_ms) * 30_000.0)
            / 90_000.0;
        assert!((direct.average().unwrap() - expected).abs() < 1e-12);
        assert_eq!(direct.score.sampled_ms, 60_000);
        assert_eq!(direct.score.pending_ms, 30_000);
    }

    #[test]
    fn goal_boundaries_preserve_the_plan_sampling_schedule() {
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
        assert_eq!(next.score.pending_ms, replay.score.pending_ms);
        assert_eq!(
            next.goal_score.sampled_ms + next.goal_score.pending_ms,
            next.elapsed_ms
        );
        assert_eq!(
            next.score.sampled_ms + next.score.pending_ms,
            prefix.elapsed_ms + next.elapsed_ms
        );
    }

    #[test]
    fn completion_effects_are_included_at_the_sample_endpoint() {
        let citizen = Citizen::with_needs(-50.0, -100.0).unwrap();
        let gathered = Prediction::new(&citizen, Cooldowns::default())
            .perform(CitizenAction::Forage, 0)
            .unwrap()
            .unwrap();
        // Only the thirtieth minute includes the newly gathered berries' wealth.
        assert!((gathered.average().unwrap() - 0.1 / 30.0).abs() < 1e-12);
    }

    #[test]
    fn scoring_handles_opposite_extremes_without_overflow() {
        for first in [-f64::MAX, f64::MAX] {
            let mut score = SampledWellbeing {
                pending_ms: 60_000,
                ..Default::default()
            };
            score.sample(first).unwrap();
            score.pending_ms = 60_000;
            assert_eq!(score.average(-first), Ok(0.0));
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
        assert_eq!(
            trade.score.sampled_ms + trade.score.pending_ms,
            trade.elapsed_ms
        );
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
            assert_eq!(chosen.actions, [expected]);
            assert_eq!(chosen.citizen.prices(), source.prices());
            if expected == CitizenAction::FindRocks {
                assert_eq!(chosen.citizen.pebbles_grams(), 5.0);
            } else {
                assert_eq!(chosen.citizen.berries_grams(), 10.0);
            }
        }
    }
}
