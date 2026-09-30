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
            Self::Wait => &[],
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
    pub scores: Vec<f64>,
    pub cooldowns: Cooldowns,
}

impl Prediction {
    pub fn new(citizen: &Citizen, cooldowns: Cooldowns) -> Self {
        Self {
            citizen: citizen.clone(),
            actions: Vec::new(),
            elapsed_ms: 0,
            scores: Vec::new(),
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
        let citizen = started.advance_predicted(duration)?;
        let mut next = self.clone();
        next.scores.push(citizen.personal_wellbeing()?);
        next.citizen = citizen;
        next.actions.push(action);
        next.elapsed_ms += duration;
        next.cooldowns = self.cooldowns.after(action, duration);
        Ok(Some(next))
    }

    pub fn average(&self) -> Result<f64, SimulationError> {
        average(&self.scores)
    }
}

pub(super) fn average(scores: &[f64]) -> Result<f64, SimulationError> {
    let mut mean: f64 = 0.0;
    for (index, &score) in scores.iter().enumerate() {
        let count = (index + 1) as f64;
        // Opposite signs can overflow subtraction even when both scores are finite.
        mean = if mean.is_sign_positive() == score.is_sign_positive() {
            mean + (score - mean) / count
        } else {
            mean * ((count - 1.0) / count) + score / count
        };
        if !mean.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
    }
    Ok(mean)
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
            reserved_ms,
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
    match action.input_for(&state.citizen, amount) {
        Some(requirement) => satisfy(
            state,
            requirement,
            reserved_ms + action.predicted_duration(),
            prior_actions,
        ),
        None => Ok(vec![state.clone()]),
    }
}

pub(super) fn best_variant(
    citizen: &Citizen,
    goal: Effect,
    cooldowns: Cooldowns,
    prior_actions: usize,
) -> Result<Option<Prediction>, SimulationError> {
    let initial = Prediction::new(citizen, cooldowns);
    let mut best: Option<Prediction> = None;
    let mut consider = |candidate: Prediction| -> Result<(), SimulationError> {
        let candidate_score = candidate.average()?;
        if best
            .as_ref()
            .map(|old| old.average())
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
        for ready in prepare(&initial, action, MEAL_NOURISHMENT, 0, prior_actions)? {
            if let Some(candidate) = ready.perform(action, prior_actions)? {
                consider(candidate)?;
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
        prepare(
            &Prediction::new(citizen, Cooldowns::default()),
            CitizenAction::Eat,
            MEAL_NOURISHMENT,
            0,
            0,
        )
        .unwrap()
        .into_iter()
        .filter_map(|ready| ready.perform(CitizenAction::Eat, 0).unwrap())
        .collect()
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
        let variants = full_meals(&source);
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
        let chosen = best_variant(&short, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        assert!(
            chosen.actions.contains(&CitizenAction::FindRocks)
                || chosen.actions.contains(&CitizenAction::Forage)
        );
    }

    #[test]
    fn backward_limit_allows_one_crossing_action_but_not_missing_prerequisites() {
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
        prefix.elapsed_ms = 7 * ACTION_DURATION_MS;
        let goal = best_variant(
            &prefix.citizen,
            Effect::ReduceHunger,
            prefix.cooldowns,
            prefix.actions.len(),
        )
        .unwrap()
        .unwrap();
        assert!(goal.elapsed_ms > ACTION_DURATION_MS);
        let mut best = super::super::Plan {
            actions: Vec::new(),
            average_wellbeing: f64::NEG_INFINITY,
        };
        super::super::search(prefix.clone(), super::super::HORIZON_MS, &mut best).unwrap();
        let mut expected = prefix.actions;
        expected.extend(goal.actions);
        assert_eq!(best.actions, expected);
        let mut scores = prefix.scores;
        scores.extend(goal.scores);
        assert_eq!(best.average_wellbeing, average(&scores).unwrap());
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
    fn scoring_handles_opposite_extremes_without_overflow() {
        assert_eq!(average(&[-f64::MAX, f64::MAX]), Ok(0.0));
        assert_eq!(average(&[f64::MAX, -f64::MAX]), Ok(0.0));
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
