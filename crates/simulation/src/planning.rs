use crate::{Citizen, CitizenAction, SimulationError};
pub mod goals;
use goals::{Cooldowns, Effect, Prediction};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub const HORIZON_MS: u64 = 4 * 60 * 60 * 1000;
pub const COMMITMENT_MS: u64 = 2 * 60 * 60 * 1000;
pub const REPLAN_MIN_NUTRITION: f64 = 20.0;
pub const EAT_PLAN_COOLDOWN_MS: u64 = 4 * 60 * 60 * 1000;

#[derive(Clone, Debug, PartialEq)]
pub struct GoalForecast {
    pub actions: Vec<CitizenAction>,
    pub duration_ms: u64,
    pub average_wellbeing: f64,
    pub full_plan_wellbeing: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoalDecision {
    pub goal: Effect,
    pub forecast: Option<GoalForecast>,
}

impl GoalDecision {
    pub fn unavailable_reason(&self) -> Option<&'static str> {
        self.forecast.is_none().then_some(
            "No executable sequence within the four-hour prerequisite limit and action rules.",
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionInputDecision {
    pub good: crate::marketplace::Good,
    pub required_units: crate::Quantity,
    pub carried_units: crate::Quantity,
    pub supply_shortfall: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionDecision {
    pub recipe: crate::production::Recipe,
    pub profit_per_hour: Option<f64>,
    pub remaining_batches: Option<crate::Quantity>,
    pub inputs: Vec<ProductionInputDecision>,
    pub prefix_attempted: bool,
    pub preparation_limit_observed: bool,
    pub feasible_sequence_found: bool,
    pub selected: bool,
    pub best_competing_score: Option<f64>,
}

impl ProductionDecision {
    fn new(citizen: &Citizen, recipe: crate::production::Recipe) -> Self {
        Self {
            recipe,
            profit_per_hour: crate::production::recipe_profit(recipe, citizen)
                .zip(recipe.duration_ms(citizen).ok())
                .map(|(profit, duration)| profit * 3_600_000.0 / duration as f64),
            remaining_batches: citizen
                .production_targets()
                .map(|t| t.remaining_batches(recipe)),
            inputs: recipe
                .inputs()
                .iter()
                .map(|&(good, required_units)| {
                    let carried_units = citizen.units(good);
                    let obtainable = carried_units
                        .saturating_add(citizen.market().listed_units(citizen.id(), good))
                        .saturating_add(citizen.market().available_units(citizen.id(), good));
                    let can_gather = good == crate::marketplace::Good::Berries
                        || citizen
                            .available_recipes()
                            .any(|r| r.outputs().iter().any(|&(g, _)| g == good));
                    ProductionInputDecision {
                        good,
                        required_units,
                        carried_units,
                        supply_shortfall: obtainable < required_units && !can_gather,
                    }
                })
                .collect(),
            prefix_attempted: false,
            preparation_limit_observed: false,
            feasible_sequence_found: false,
            selected: false,
            best_competing_score: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanningDecision {
    pub candidates: Vec<GoalDecision>,
    pub selected_goal: Effect,
    pub production: Vec<ProductionDecision>,
    pub prices: crate::marketplace::Prices,
}

/// A chosen goal and its primitive actions, including prerequisites.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanGoal {
    pub goal: Effect,
    pub actions: std::ops::Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    actions: Vec<CitizenAction>,
    action_durations_ms: Vec<u64>,
    goals: Vec<PlanGoal>,
    average_wellbeing: f64,
    decision: Option<Arc<PlanningDecision>>,
}

impl Plan {
    /// Goal boundaries in execution order.
    pub fn goals(&self) -> &[PlanGoal] {
        &self.goals
    }

    /// Predicted durations in the same order as `actions()`.
    pub fn action_durations_ms(&self) -> &[u64] {
        &self.action_durations_ms
    }

    pub fn decision(&self) -> Option<&PlanningDecision> {
        self.decision.as_deref()
    }

    pub fn actions(&self) -> &[CitizenAction] {
        &self.actions
    }

    pub fn average_wellbeing(&self) -> f64 {
        self.average_wellbeing
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivePlan {
    plan: Plan,
    action_index: usize,
    elapsed_ms: u64,
}

impl ActivePlan {
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Zero-based index of the action being executed.
    pub fn action_index(&self) -> usize {
        self.action_index
    }

    /// Time spent executing this plan, including the current action.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    pub(crate) fn travelling_purchase(&self, remaining_ms: u64) -> Option<CitizenAction> {
        if self.elapsed_ms.saturating_add(remaining_ms) >= COMMITMENT_MS {
            return None;
        }
        let next = self.action_index + 1;
        let goal = self
            .plan
            .goals
            .iter()
            .find(|goal| goal.actions.contains(&self.action_index))?;
        (next < goal.actions.end).then(|| self.plan.actions[next])
    }

    pub(crate) fn advance(
        &self,
        source: &Citizen,
        mut elapsed_ms: u64,
        end_prices: Option<crate::marketplace::Prices>,
    ) -> Result<Citizen, SimulationError> {
        let mut citizen = source.clone();
        citizen.active_plan = None;
        let mut execution = self.clone();

        while elapsed_ms > 0 {
            let remaining_ms = citizen
                .active_action()
                .expect("a planned citizen has an active action")
                .remaining_ms();
            let step_ms = elapsed_ms.min(remaining_ms);
            citizen = citizen.advance(step_ms)?;
            elapsed_ms -= step_ms;
            execution.elapsed_ms = execution
                .elapsed_ms
                .checked_add(step_ms)
                .ok_or(SimulationError::TimeOverflow)?;

            if elapsed_ms == 0
                && let Some(prices) = end_prices
            {
                citizen.prices = prices;
                citizen.market.prices = prices;
            }
            if citizen.active_action().is_none() {
                execution.action_index += 1;
                citizen = execution.start_next_action(citizen)?;
            }
        }

        citizen.active_plan = Some(execution);
        Ok(citizen)
    }

    pub(crate) fn record_elapsed(&mut self, elapsed_ms: u64) -> Result<(), SimulationError> {
        self.elapsed_ms = self
            .elapsed_ms
            .checked_add(elapsed_ms)
            .ok_or(SimulationError::TimeOverflow)?;
        Ok(())
    }

    pub(crate) fn finish_action(&mut self) {
        self.action_index += 1;
    }

    pub(crate) fn resume(&mut self, citizen: Citizen) -> Result<Citizen, SimulationError> {
        self.start_next_action(citizen)
    }

    fn start_next_action(&mut self, mut citizen: Citizen) -> Result<Citizen, SimulationError> {
        loop {
            if self.elapsed_ms >= COMMITMENT_MS
                || self.action_index >= self.plan.actions.len()
                || replan_check(
                    &citizen,
                    self.plan.actions[self.action_index],
                    self.action_index,
                )
            {
                let shortage = self
                    .plan
                    .actions
                    .get(self.action_index)
                    .is_some_and(|action| {
                        matches!(action, CitizenAction::Produce(_))
                            && replan_check(&citizen, *action, self.action_index)
                    });
                citizen.refresh_production_targets(shortage)?;
                self.plan = plan(&citizen)?;
                self.action_index = 0;
                self.elapsed_ms = 0;
            }
            citizen = citizen.start_action(self.plan.actions[self.action_index])?;
            if citizen.active_action().is_some() {
                return Ok(citizen);
            }
            self.action_index += 1;
        }
    }
}

fn replan_check(citizen: &Citizen, action: CitizenAction, action_index: usize) -> bool {
    action_index > 0
        && match action {
            CitizenAction::Eat => citizen.food_nutrition() < REPLAN_MIN_NUTRITION,
            CitizenAction::EquipClothing => {
                citizen.units(crate::marketplace::Good::FlaxGarment) == 0
            }
            CitizenAction::Produce(recipe) => recipe
                .inputs()
                .iter()
                .any(|&(good, units)| citizen.units(good) < units),
            _ => false,
        }
}

pub fn plan(citizen: &Citizen) -> Result<Plan, SimulationError> {
    if citizen.active_action().is_some() || citizen.active_plan().is_some() {
        return Err(SimulationError::CitizenBusy);
    }
    let mut best = empty_plan();
    let mut candidates = Vec::new();
    let mut production: Vec<_> = citizen
        .available_recipes()
        .map(|recipe| ProductionDecision::new(citizen, recipe))
        .collect();
    let mut selected_goal = Effect::ReduceTiredness;
    let clothing_choice =
        citizen.units(crate::marketplace::Good::FlaxGarment) > 0 && citizen.clothing_need() > 0.0;
    // Compare wearing an acquired garment with selling it after a commitment expires.
    let standing_listing = !clothing_choice
        && citizen.production_targets().is_some()
        && citizen.hunger() < crate::production::URGENT_HUNGER
        && citizen.tiredness() < crate::production::URGENT_TIREDNESS
        && citizen.excess_value()? >= crate::production::MIN_LISTING_VALUE;
    let goals: &[Effect] = if standing_listing {
        &[Effect::ListExcess]
    } else if citizen.production_targets().is_some() {
        &[
            Effect::ReduceHunger,
            Effect::ReduceTiredness,
            Effect::IncreaseWealth,
            Effect::ReplenishReserves,
            Effect::ReduceClothingNeed,
            Effect::Production,
        ]
    } else {
        &[
            Effect::ReduceHunger,
            Effect::ReduceTiredness,
            Effect::IncreaseWealth,
            Effect::ReplenishReserves,
            Effect::ReduceClothingNeed,
        ]
    };
    let mut goals = goals.to_vec();
    if clothing_choice && citizen.production_targets().is_some() {
        goals.push(Effect::ListExcess);
    }
    let mut cache = Continuations::default();
    for goal in goals {
        let mut forecast = None;
        let mut goal_best = empty_plan();
        let initial = Prediction::new(citizen, Cooldowns::default());
        let variants = if goal == Effect::Production {
            goals::production_prefixes(initial, 0, &mut production)?
        } else {
            goals::variants_after(&initial, goal)?
        };
        for variant in variants {
            for diagnostic in &mut production {
                diagnostic.feasible_sequence_found |= variant
                    .actions
                    .contains(&CitizenAction::Produce(diagnostic.recipe));
            }
            let actions = variant.actions.clone();
            let duration_ms = variant.elapsed_ms;
            let average_wellbeing = variant.average()?;
            let mut continuation = empty_plan();
            let boundary = PlanGoal {
                goal,
                actions: 0..variant.actions.len(),
            };
            search_with_goals(
                variant,
                HORIZON_MS,
                &mut continuation,
                vec![boundary],
                &mut cache,
            )?;
            if continuation.average_wellbeing.is_finite() {
                for diagnostic in &mut production {
                    if continuation
                        .actions
                        .contains(&CitizenAction::Produce(diagnostic.recipe))
                    {
                        diagnostic.feasible_sequence_found = true;
                        diagnostic.best_competing_score = Some(
                            diagnostic
                                .best_competing_score
                                .map_or(continuation.average_wellbeing, |score| {
                                    score.max(continuation.average_wellbeing)
                                }),
                        );
                    }
                }
            }
            if continuation.average_wellbeing > goal_best.average_wellbeing {
                forecast = Some(GoalForecast {
                    actions,
                    duration_ms,
                    average_wellbeing,
                    full_plan_wellbeing: continuation.average_wellbeing,
                });
                goal_best = continuation;
            }
        }
        if goal_best.average_wellbeing > best.average_wellbeing {
            best = goal_best;
            selected_goal = goal;
        }
        candidates.push(GoalDecision { goal, forecast });
    }
    for diagnostic in &mut production {
        diagnostic.selected = best
            .actions
            .contains(&CitizenAction::Produce(diagnostic.recipe));
    }
    best.decision = Some(Arc::new(PlanningDecision {
        candidates,
        selected_goal,
        production,
        prices: citizen.prices(),
    }));
    Ok(best)
}

fn empty_plan() -> Plan {
    Plan {
        actions: Vec::new(),
        action_durations_ms: Vec::new(),
        goals: Vec::new(),
        average_wellbeing: f64::NEG_INFINITY,
        decision: None,
    }
}

#[cfg(test)]
fn search(state: Prediction, horizon_ms: u64, best: &mut Plan) -> Result<(), SimulationError> {
    search_with_goals(
        state,
        horizon_ms,
        best,
        Vec::new(),
        &mut Continuations {
            disabled: true,
            ..Default::default()
        },
    )
}

#[derive(Default)]
struct Continuations {
    plans: HashMap<u64, Vec<(Vec<CitizenAction>, u8, Plan)>>,
    #[cfg(test)]
    disabled: bool,
}

impl Continuations {
    fn key(actions: &[CitizenAction], used_goals: u8) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        used_goals.hash(&mut hash);
        for action in actions {
            std::mem::discriminant(action).hash(&mut hash);
            match action {
                CitizenAction::Travel(place) => place.hash(&mut hash),
                CitizenAction::BuyFood(good) => good.hash(&mut hash),
                CitizenAction::Produce(recipe) => recipe.hash(&mut hash),
                CitizenAction::List(good, units) | CitizenAction::Withdraw(good, units) => {
                    good.hash(&mut hash);
                    units.hash(&mut hash);
                }
                CitizenAction::Buy(list) => {
                    for (good, units) in list.items() {
                        good.hash(&mut hash);
                        units.hash(&mut hash);
                    }
                }
                _ => {}
            }
        }
        hash.finish()
    }

    fn get(&self, key: u64, actions: &[CitizenAction], used_goals: u8) -> Option<&Plan> {
        #[cfg(test)]
        if self.disabled {
            return None;
        }
        self.plans
            .get(&key)?
            .iter()
            .find(|(path, used, _)| path == actions && *used == used_goals)
            .map(|(_, _, plan)| plan)
    }

    fn insert(&mut self, key: u64, actions: Vec<CitizenAction>, used_goals: u8, plan: Plan) {
        #[cfg(test)]
        if self.disabled {
            return;
        }
        self.plans
            .entry(key)
            .or_default()
            .push((actions, used_goals, plan));
    }
}

fn goal_bit(goal: Effect) -> u8 {
    match goal {
        Effect::ReduceHunger => 1,
        Effect::ReduceTiredness => 2,
        Effect::IncreaseWealth => 4,
        Effect::ReplenishReserves => 8,
        Effect::Production => 16,
        Effect::ListExcess => 32,
        Effect::ReduceClothingNeed => 64,
        Effect::Food | Effect::Coins => unreachable!("resources are not planning goals"),
    }
}

fn search_with_goals(
    state: Prediction,
    horizon_ms: u64,
    best: &mut Plan,
    boundaries: Vec<PlanGoal>,
    cache: &mut Continuations,
) -> Result<(), SimulationError> {
    if state.elapsed_ms >= horizon_ms {
        let score = state.average()?;
        if score > best.average_wellbeing {
            *best = Plan {
                actions: state.actions,
                action_durations_ms: state.action_durations_ms,
                goals: boundaries,
                average_wellbeing: score,
                decision: None,
            };
        }
        return Ok(());
    }
    let used_goals = boundaries
        .iter()
        .fold(0, |used, boundary| used | goal_bit(boundary.goal));
    let key = Continuations::key(&state.actions, used_goals);
    if let Some(cached) = cache.get(key, &state.actions, used_goals) {
        if cached.average_wellbeing > best.average_wellbeing {
            let mut result = cached.clone();
            result
                .goals
                .retain(|goal| goal.actions.start >= state.actions.len());
            result.goals.splice(0..0, boundaries);
            *best = result;
        }
        return Ok(());
    }
    let mut continuation = empty_plan();
    for goal in [
        Effect::ReduceHunger,
        Effect::ReduceTiredness,
        Effect::IncreaseWealth,
        Effect::ReplenishReserves,
        Effect::ReduceClothingNeed,
        Effect::Production,
    ] {
        if used_goals & goal_bit(goal) != 0 {
            continue;
        }
        for variant in goals::variants_after(&state, goal)? {
            let mut next = variant;
            next.elapsed_ms += state.elapsed_ms;
            let mut actions = state.actions.clone();
            actions.append(&mut next.actions);
            let mut durations = state.action_durations_ms.clone();
            durations.append(&mut next.action_durations_ms);
            next.action_durations_ms = durations;
            let mut goals = boundaries.clone();
            goals.push(PlanGoal {
                goal,
                actions: state.actions.len()..actions.len(),
            });
            next.actions = actions;
            search_with_goals(next, horizon_ms, &mut continuation, goals, cache)?;
        }
    }
    // Goal availability affects continuation even when primitive prefixes are identical.
    cache.insert(key, state.actions, used_goals, continuation.clone());
    if continuation.average_wellbeing > best.average_wellbeing {
        *best = continuation;
    }
    Ok(())
}

pub(crate) fn start(citizen: &Citizen) -> Result<Citizen, SimulationError> {
    let plan = plan(citizen)?;
    let mut citizen = citizen.start_action(plan.actions[0])?;
    citizen.active_plan = Some(ActivePlan {
        plan,
        action_index: 0,
        elapsed_ms: 0,
    });
    Ok(citizen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::SmallRng};

    const MINUTE_MS: u64 = 60_000;

    fn executing(citizen: &Citizen, actions: Vec<CitizenAction>) -> Citizen {
        let mut citizen = citizen.start_action(actions[0]).unwrap();
        citizen.active_plan = Some(ActivePlan {
            plan: Plan {
                action_durations_ms: vec![0; actions.len()],
                goals: Vec::new(),
                actions,
                average_wellbeing: -1.0,
                decision: None,
            },
            action_index: 0,
            elapsed_ms: 0,
        });
        citizen
    }

    #[test]
    fn failed_garment_purchase_replans_before_equipping() {
        let citizen = Citizen::with_needs(-50.0, -100.0).unwrap();
        assert!(replan_check(&citizen, CitizenAction::EquipClothing, 1));
        let planned = executing(
            &citizen,
            vec![CitizenAction::Wait, CitizenAction::EquipClothing],
        );
        let next = planned.advance(crate::ACTION_DURATION_MS).unwrap();
        assert_eq!(next.active_plan().unwrap().elapsed_ms(), 0);
        assert_ne!(
            next.active_action().unwrap().action(),
            CitizenAction::EquipClothing
        );
    }

    #[test]
    fn commitment_compares_wearing_an_acquired_garment_with_listing_it() {
        use crate::{StartingRole, marketplace::Good};
        let mut citizen = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Tailor)
            .with_good(Good::FlaxGarment, 1)
            .unwrap();
        citizen.refresh_production_targets(true).unwrap();
        let mut planned = executing(
            &citizen,
            vec![CitizenAction::Wait, CitizenAction::EquipClothing],
        );
        planned.active_plan.as_mut().unwrap().elapsed_ms =
            COMMITMENT_MS - crate::ACTION_DURATION_MS;
        let replanned = planned.advance(crate::ACTION_DURATION_MS).unwrap();
        let decision = replanned.active_plan().unwrap().plan().decision().unwrap();
        assert!(decision.candidates.iter().any(|candidate| {
            candidate.goal == Effect::ReduceClothingNeed && candidate.forecast.is_some()
        }));
        assert!(
            decision
                .candidates
                .iter()
                .any(|candidate| candidate.goal == Effect::ListExcess)
        );
        assert_eq!(replanned.active_plan().unwrap().elapsed_ms(), 0);
    }

    #[test]
    fn cached_continuations_distinguish_prefix_goal_availability() {
        let citizen = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_berries(300)
            .unwrap();
        let prefix = goals::variants_after(
            &Prediction::new(&citizen, Cooldowns::default()),
            Effect::IncreaseWealth,
        )
        .unwrap()
        .remove(0);
        let boundary = |goal| {
            vec![PlanGoal {
                goal,
                actions: 0..prefix.actions.len(),
            }]
        };
        let mut cache = Continuations::default();
        let mut first = empty_plan();
        search_with_goals(
            prefix.clone(),
            HORIZON_MS,
            &mut first,
            boundary(Effect::IncreaseWealth),
            &mut cache,
        )
        .unwrap();
        assert!(
            cache
                .get(
                    Continuations::key(&prefix.actions, goal_bit(Effect::IncreaseWealth)),
                    &prefix.actions,
                    goal_bit(Effect::IncreaseWealth)
                )
                .is_some()
        );
        let mut reused = empty_plan();
        search_with_goals(
            prefix.clone(),
            HORIZON_MS,
            &mut reused,
            boundary(Effect::ReplenishReserves),
            &mut cache,
        )
        .unwrap();
        let mut reference = empty_plan();
        search_with_goals(
            prefix.clone(),
            HORIZON_MS,
            &mut reference,
            boundary(Effect::ReplenishReserves),
            &mut Continuations {
                disabled: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(reused, reference);

        assert_eq!(reused.goals()[0].goal, Effect::ReplenishReserves);
        assert!(
            reused.goals()[1..]
                .iter()
                .all(|goal| goal.goal != Effect::ReplenishReserves)
        );
        assert_eq!(reused.goals()[1].actions.start, prefix.actions.len());
    }

    #[test]
    fn continuation_cache_checks_used_goals_even_on_hash_collision() {
        let mut cache = Continuations::default();
        cache.insert(
            7,
            vec![CitizenAction::Forage],
            goal_bit(Effect::IncreaseWealth),
            empty_plan(),
        );
        assert!(
            cache
                .get(
                    7,
                    &[CitizenAction::Forage],
                    goal_bit(Effect::IncreaseWealth)
                )
                .is_some()
        );
        assert!(
            cache
                .get(
                    7,
                    &[CitizenAction::Forage],
                    goal_bit(Effect::ReplenishReserves)
                )
                .is_none()
        );
    }

    #[test]
    fn selected_plans_use_each_goal_once_and_reset_on_replanning() {
        let source = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_berries(300)
            .unwrap();
        for citizen in [&source, &source.with_coins(300).unwrap()] {
            let first = plan(citizen).unwrap();
            assert!(!first.actions().is_empty());
            let mut used = 0;
            for goal in first.goals() {
                assert_eq!(used & goal_bit(goal.goal), 0);
                used |= goal_bit(goal.goal);
            }
            assert_eq!(plan(citizen).unwrap(), first);
        }
    }

    #[test]
    fn food_requests_follow_only_the_executing_goal_and_clear_on_replanning() {
        use crate::{
            AgentKind, Universe,
            locations::{Location, Map, Position},
            marketplace::{Good, ShoppingList},
        };
        let map = Map::new(
            Position::default(),
            Position::default(),
            Position { x: 100.0, y: 0.0 },
        )
        .unwrap();
        let travel = CitizenAction::Travel(map.public_place(Location::Market));
        let purchase = CitizenAction::Buy(ShoppingList::single(Good::Bread, 60));
        let source = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap();
        let mut buyer = executing(&source, vec![travel, purchase]);
        let execution = buyer.active_plan.as_mut().unwrap();
        execution.plan.goals = vec![
            PlanGoal {
                goal: Effect::IncreaseWealth,
                actions: 0..1,
            },
            PlanGoal {
                goal: Effect::ReplenishReserves,
                actions: 1..2,
            },
        ];
        assert_eq!(execution.travelling_purchase(60_000), None);
        let (universe, id) = Universe::with_map(map)
            .with_citizen("Buyer", buyer.clone())
            .unwrap();
        assert_eq!(universe.market().requested(id), ShoppingList::default());

        let execution = buyer.active_plan.as_mut().unwrap();
        execution.plan.goals = vec![PlanGoal {
            goal: Effect::ReplenishReserves,
            actions: 0..2,
        }];
        let remaining_ms = buyer.active_action().unwrap().remaining_ms();
        let execution = buyer.active_plan.as_mut().unwrap();
        for (elapsed_ms, expected) in [
            (COMMITMENT_MS - remaining_ms - 1, Some(purchase)),
            (COMMITMENT_MS - remaining_ms, None),
            (COMMITMENT_MS - remaining_ms + 1, None),
        ] {
            execution.elapsed_ms = elapsed_ms;
            assert_eq!(execution.travelling_purchase(remaining_ms), expected);
        }
        let (universe, id) = Universe::with_map(buyer.map())
            .with_citizen("Buyer", buyer)
            .unwrap();
        assert_eq!(universe.market().requested(id), ShoppingList::default());
        let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
        let arrived = universe
            .advance(citizen.active_action().unwrap().remaining_ms())
            .unwrap();
        let AgentKind::Citizen(citizen) = &arrived.agents()[&id].kind;
        assert_eq!(citizen.active_plan().unwrap().elapsed_ms(), 0);
        assert_eq!(arrived.market().requested(id), ShoppingList::default());
    }

    #[test]
    fn goal_ranges_include_prerequisites_and_cover_repeated_primitives() {
        use crate::marketplace::Prices;
        let citizen = Citizen::with_needs(70.0, -100.0)
            .unwrap()
            .with_prices(Prices::new(200.0).unwrap());
        let selected = plan(&citizen).unwrap();
        let forecast = selected
            .decision()
            .unwrap()
            .candidates
            .iter()
            .find(|candidate| candidate.goal == selected.decision().unwrap().selected_goal)
            .unwrap()
            .forecast
            .as_ref()
            .unwrap();
        assert_eq!(selected.goals()[0].actions, 0..forecast.actions.len());
        assert_eq!(
            &selected.actions()[selected.goals()[0].actions.clone()],
            forecast.actions
        );
        assert_eq!(
            selected.action_durations_ms()[selected.goals()[0].actions.clone()]
                .iter()
                .sum::<u64>(),
            forecast.duration_ms
        );
        let mut covered = Vec::new();
        for boundary in selected.goals() {
            covered.extend(boundary.actions.clone());
            assert!(
                selected.actions()[boundary.actions.end - 1]
                    .effects()
                    .contains(&boundary.goal)
            );
        }
        assert_eq!(covered, (0..selected.actions().len()).collect::<Vec<_>>());
        assert!(selected.actions().windows(2).any(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn goal_boundaries_cover_actions_and_metadata_survives_execution() {
        let citizen = Citizen::with_needs(60.0, 100.0)
            .unwrap()
            .with_berries(310)
            .unwrap();
        let started = start(&citizen).unwrap();
        let plan = started.active_plan().unwrap().plan();
        assert_eq!(plan.goals()[0].goal, plan.decision().unwrap().selected_goal);
        assert_eq!(plan.action_durations_ms().len(), plan.actions().len());
        assert!(
            plan.action_durations_ms()
                .iter()
                .all(|&duration| duration > 0)
        );
        let mut next = 0;
        for boundary in plan.goals() {
            assert_eq!(boundary.actions.start, next);
            assert!(boundary.actions.end > next);
            next = boundary.actions.end;
        }
        assert_eq!(next, plan.actions().len());
        assert_eq!(plan.goals()[0].goal, Effect::ReduceHunger);
        assert_eq!(plan.goals()[0].actions, 0..1);
        assert_eq!(plan.goals()[1].goal, Effect::ReduceTiredness);
        let advanced = started.advance(155_000).unwrap();
        assert_eq!(advanced.active_plan().unwrap().action_index(), 1);
        assert_eq!(advanced.active_plan().unwrap().plan(), plan);
        assert_eq!(started.active_plan().unwrap().action_index(), 0);
        assert_eq!(
            advanced.active_action().unwrap().duration_ms(),
            plan.action_durations_ms()[1]
        );
    }

    #[test]
    fn gathering_order_execution_delivers_each_primitive_random_yield() {
        let citizen = Citizen::with_needs(-50.0, -100.0).unwrap();
        let mut planned = executing(&citizen, vec![CitizenAction::Forage; 4]);
        let mut replay = citizen.clone();
        for _ in 0..3 {
            replay = replay
                .start_action(CitizenAction::Forage)
                .unwrap()
                .advance(crate::ACTION_DURATION_MS)
                .unwrap();
            planned = planned.advance(crate::ACTION_DURATION_MS).unwrap();
            assert_eq!(planned.berries_units(), replay.berries_units());
            assert_eq!(planned.forage_rng, replay.forage_rng);
        }
    }

    #[test]
    fn commitment_replans_between_segments_without_truncating_the_forecast() {
        let citizen = Citizen::with_needs(-50.0, -100.0).unwrap();
        let chosen = plan(&citizen).unwrap();
        let wealth = chosen
            .decision()
            .unwrap()
            .candidates
            .iter()
            .find(|candidate| candidate.goal == Effect::IncreaseWealth)
            .unwrap()
            .forecast
            .as_ref()
            .unwrap();
        assert!(wealth.duration_ms > COMMITMENT_MS);
        let actions = wealth.actions.clone();
        let source = executing(&citizen, actions.clone());
        let before = source.advance(COMMITMENT_MS - 1).unwrap();
        assert_eq!(before.active_plan().unwrap().plan().actions(), actions);
        assert_eq!(
            before.active_plan().unwrap().elapsed_ms(),
            COMMITMENT_MS - 1
        );
        let replanned = before.advance(1).unwrap();
        assert_eq!(replanned.active_plan().unwrap().elapsed_ms(), 0);
        assert!(replanned.active_plan().unwrap().plan().decision().is_some());
        assert!(replanned.wealth().unwrap() > citizen.wealth().unwrap());
    }

    #[test]
    fn commitment_waits_for_travel_to_finish_before_replanning() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 300.0, y: 400.0 },
            Position::default(),
            Position::default(),
        )
        .unwrap();
        let source = Citizen::new(0.0).unwrap().with_map(map.clone()).unwrap();
        let mut planned = executing(
            &source,
            vec![
                CitizenAction::Travel(map.public_place(Location::Forest)),
                CitizenAction::Forage,
            ],
        );
        planned.active_plan.as_mut().unwrap().elapsed_ms = COMMITMENT_MS - 60_000;
        let crossed = planned.advance(60_000).unwrap();
        assert_eq!(
            crossed.active_plan().unwrap().plan(),
            planned.active_plan().unwrap().plan()
        );
        assert_eq!(
            crossed.active_action().unwrap().action(),
            CitizenAction::Travel(map.public_place(Location::Forest))
        );
        let arrived = crossed.advance(240_000).unwrap();
        assert_eq!(
            arrived.position(),
            map.position(map.public_place(Location::Forest))
        );
        assert_eq!(arrived.active_plan().unwrap().elapsed_ms(), 0);
        assert!(arrived.active_plan().unwrap().plan().decision().is_some());
    }

    #[test]
    fn decision_records_existing_candidates_and_preserves_search_result() {
        use crate::marketplace::Prices;
        for prices in [Prices::new(200.0).unwrap(), Prices::new(100.0).unwrap()] {
            let citizen = Citizen::with_needs(50.0, -100.0)
                .unwrap()
                .with_prices(prices)
                .with_berries(33)
                .unwrap();
            let snapshot = citizen.clone();
            let chosen = plan(&citizen).unwrap();
            let mut reference = empty_plan();
            search(
                Prediction::new(&citizen, Cooldowns::default()),
                HORIZON_MS,
                &mut reference,
            )
            .unwrap();
            assert_eq!(chosen.actions(), reference.actions());
            assert_eq!(chosen.average_wellbeing(), reference.average_wellbeing());
            let decision = chosen.decision().unwrap();
            assert_eq!(decision.candidates.len(), 5);
            assert_eq!(decision.prices, prices);
            for candidate in &decision.candidates {
                let expected = goals::variants_after(
                    &Prediction::new(&citizen, Cooldowns::default()),
                    candidate.goal,
                )
                .unwrap()
                .into_iter()
                .find(|variant| {
                    candidate
                        .forecast
                        .as_ref()
                        .is_some_and(|forecast| forecast.actions == variant.actions)
                });
                match (&candidate.forecast, expected) {
                    (Some(forecast), Some(expected)) => {
                        assert_eq!(forecast.actions, expected.actions);
                        assert_eq!(forecast.duration_ms, expected.elapsed_ms);
                        assert_eq!(forecast.average_wellbeing, expected.average().unwrap());
                        assert!(forecast.full_plan_wellbeing <= chosen.average_wellbeing());
                        if candidate.goal == decision.selected_goal {
                            assert!(chosen.actions().starts_with(&forecast.actions));
                            assert_eq!(forecast.full_plan_wellbeing, chosen.average_wellbeing());
                        }
                    }
                    (None, None) => assert!(
                        candidate
                            .unavailable_reason()
                            .unwrap()
                            .contains("four-hour")
                    ),
                    _ => panic!("report differs from goal search"),
                }
            }
            let hunger = &decision.candidates[0];
            assert!(hunger.forecast.is_some());
            assert_eq!(citizen, snapshot);
        }
    }

    #[test]
    fn full_continuations_can_outscore_the_local_goal_winner() {
        let citizen = Citizen::with_needs(0.0, -100.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(500.0).unwrap())
            .with_berries(120)
            .unwrap();
        let local = goals::best_variant(&citizen, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        let chosen = plan(&citizen).unwrap();
        let forecast = chosen.decision().unwrap().candidates[0]
            .forecast
            .as_ref()
            .unwrap();
        let mut local_plan = empty_plan();
        search(local.clone(), HORIZON_MS, &mut local_plan).unwrap();
        assert_ne!(forecast.actions, local.actions);
        assert!(forecast.average_wellbeing < local.average().unwrap());
        assert!(forecast.full_plan_wellbeing > local_plan.average_wellbeing);
        for variant in goals::variants_after(
            &Prediction::new(&citizen, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap()
        {
            let mut full = empty_plan();
            search(variant, HORIZON_MS, &mut full).unwrap();
            assert!(full.average_wellbeing <= forecast.full_plan_wellbeing);
        }
    }

    #[test]
    fn exported_hunger_state_compares_affordable_trading_continuations() {
        use crate::locations::{Location, Map, Position};
        use crate::marketplace::Prices;
        let map = Map::new(
            Position {
                x: 386.3952040173808,
                y: -841.1173238559137,
            },
            Position {
                x: -724.4895163019701,
                y: -755.0057040246737,
            },
            Position {
                x: -501.65675592959025,
                y: 468.19843490627386,
            },
        )
        .unwrap();
        let growth = 73533.0 * 100.0 / 86400000.0;
        let citizen =
            Citizen::with_needs(-19.624550026806507 - growth, -14.518873842590954 - growth)
                .unwrap()
                .with_map(map.clone())
                .unwrap()
                .with_position(map.position(map.public_place(Location::River)))
                .unwrap()
                .with_prices(Prices::new(189.75551078215392).unwrap())
                .with_berries(25)
                .unwrap()
                .with_coins(100)
                .unwrap();
        let mut market = citizen.market().clone();
        market
            .list(
                crate::AgentId(uuid::Uuid::new_v4()),
                crate::marketplace::Good::Berries,
                1000,
            )
            .unwrap();
        let citizen = citizen.with_market(market);
        let variants = goals::variants_after(
            &Prediction::new(&citizen, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap();
        let trade = variants
            .iter()
            .find(|variant| {
                variant.actions
                    == [
                        CitizenAction::Travel(map.public_place(Location::Market)),
                        CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                        CitizenAction::Eat,
                    ]
            })
            .unwrap();
        let mut trade_plan = empty_plan();
        search(trade.clone(), HORIZON_MS, &mut trade_plan).unwrap();
        let local = goals::best_variant(&citizen, Effect::ReduceHunger, Cooldowns::default(), 0)
            .unwrap()
            .unwrap();
        let mut local_plan = empty_plan();
        search(local, HORIZON_MS, &mut local_plan).unwrap();
        assert!(trade_plan.average_wellbeing.is_finite());
        let chosen = plan(&citizen).unwrap();
        let hunger = chosen.decision().unwrap().candidates[0]
            .forecast
            .as_ref()
            .unwrap();
        assert!(hunger.full_plan_wellbeing >= trade_plan.average_wellbeing);
        assert!(hunger.full_plan_wellbeing >= local_plan.average_wellbeing);
        for variant in variants {
            let mut full = empty_plan();
            search(variant, HORIZON_MS, &mut full).unwrap();
            assert!(hunger.full_plan_wellbeing >= full.average_wellbeing);
        }
    }

    #[test]
    fn report_remains_a_snapshot_until_replanning() {
        use crate::marketplace::Prices;
        let source = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .start_planning()
            .unwrap();
        let report = source
            .active_plan()
            .unwrap()
            .plan()
            .decision
            .clone()
            .unwrap();
        let repriced = source.with_prices(Prices::new(200.0).unwrap());
        let partial = repriced.advance(1).unwrap();
        assert!(Arc::ptr_eq(
            &report,
            partial
                .active_plan()
                .unwrap()
                .plan()
                .decision
                .as_ref()
                .unwrap()
        ));
        let mut next = partial;
        for _ in 0..20 {
            let duration = next.active_action().unwrap().remaining_ms();
            next = next.advance(duration).unwrap();
            let current = next
                .active_plan()
                .unwrap()
                .plan()
                .decision
                .as_ref()
                .unwrap();
            if !Arc::ptr_eq(&report, current) {
                assert_eq!(current.prices, Prices::new(200.0).unwrap());
                return;
            }
        }
        panic!("expected a replan");
    }

    #[test]
    fn action_starting_at_the_daily_boundary_uses_new_prices() {
        use crate::{
            AgentKind, Universe,
            marketplace::{Good, Prices, UPDATE_TIME_MS},
        };
        let citizen = Citizen::new(0.0).unwrap().with_coins(100).unwrap();
        let mut market = citizen.market().clone();
        let seller = Citizen::new(0.0).unwrap().with_map(citizen.map()).unwrap();
        market.list(seller.id(), Good::Berries, 1000).unwrap();
        let citizen = citizen.with_market(market.clone());
        let planned = executing(
            &citizen,
            vec![
                CitizenAction::Wait,
                CitizenAction::BuyFood(crate::marketplace::Good::Berries),
                CitizenAction::Eat,
                CitizenAction::Sleep,
            ],
        );
        let universe = Universe::with_map(seller.map())
            .with_prices(Prices::default())
            .advance(UPDATE_TIME_MS - crate::ACTION_DURATION_MS)
            .unwrap();
        let (mut universe, _) = universe.with_citizen("Seller", seller).unwrap();
        universe.market = market;
        let planned = {
            let mut p = planned;
            p.map = universe.map.clone();
            p
        };
        let (universe, id) = universe.with_citizen("Ada", planned).unwrap();
        let boundary = universe.advance(crate::ACTION_DURATION_MS).unwrap();
        let AgentKind::Citizen(buyer) = &boundary.agents()[&id].kind;
        assert_eq!(
            buyer.active_action().unwrap().action(),
            CitizenAction::BuyFood(crate::marketplace::Good::Berries)
        );
        assert_eq!(
            buyer.active_action().unwrap().remaining_ms(),
            crate::TRADE_DURATION_MS
        );
        let after = boundary.advance(crate::TRADE_DURATION_MS).unwrap();
        let AgentKind::Citizen(buyer) = &after.agents()[&id].kind;
        assert_eq!(
            buyer.coins(),
            100 - boundary
                .market()
                .purchase_cost(id, Good::Berries, 155)
                .unwrap()
        );
        let combined = universe
            .advance(crate::ACTION_DURATION_MS + crate::TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(after, combined);
    }

    #[test]
    fn replan_check_is_strictly_below_twenty_nutrition_and_only_before_later_meals() {
        for units in [0, 61, 62, 77] {
            let citizen = Citizen::with_needs(100.0, -100.0)
                .unwrap()
                .with_berries(units)
                .unwrap();
            assert!(!replan_check(&citizen, CitizenAction::Eat, 0));
            assert!(!replan_check(&citizen, CitizenAction::Forage, 1));
            assert_eq!(replan_check(&citizen, CitizenAction::Eat, 1), units < 62);
            let planned = executing(
                &citizen,
                vec![
                    CitizenAction::Wait,
                    CitizenAction::Eat,
                    CitizenAction::Sleep,
                ],
            );
            let next = planned.advance(30 * MINUTE_MS).unwrap();
            if units < 62 {
                assert_eq!(next.active_plan().unwrap().action_index(), 0);
                assert_eq!(next.active_plan().unwrap().elapsed_ms(), 0);
            } else {
                assert_eq!(
                    next.active_plan().unwrap().plan(),
                    planned.active_plan().unwrap().plan()
                );
                assert_eq!(next.active_plan().unwrap().action_index(), 1);
                assert_eq!(next.active_plan().unwrap().elapsed_ms(), 30 * MINUTE_MS);
                assert_eq!(next.active_action().unwrap().remaining_ms(), units * 1000);
                let partial = next.advance(20_000).unwrap();
                assert!(partial.berries_units() < 62);
                assert_eq!(partial.active_plan().unwrap().action_index(), 1);
                assert_eq!(
                    partial.active_action().unwrap().action(),
                    CitizenAction::Eat
                );
            }
            assert_eq!(planned.berries_units(), units);
        }
    }

    #[test]
    fn a_low_actual_forage_yield_replans_from_actual_inventory() {
        let mut citizen = Citizen::with_needs(100.0, -100.0)
            .unwrap()
            .with_berries(30)
            .unwrap();
        let (source, actual) = (0..100)
            .find_map(|seed| {
                citizen.forage_rng = SmallRng::seed_from_u64(seed);
                let actual = citizen
                    .start_action(CitizenAction::Forage)
                    .unwrap()
                    .advance(30 * MINUTE_MS)
                    .unwrap();
                (actual.berries_units() < 40).then(|| (citizen.clone(), actual))
            })
            .unwrap();
        let planned = executing(
            &source,
            vec![
                CitizenAction::Forage,
                CitizenAction::Eat,
                CitizenAction::Sleep,
            ],
        );
        let actual_plan = plan(&actual).unwrap();
        let result = planned.advance(30 * MINUTE_MS).unwrap();
        assert_eq!(result.active_plan().unwrap().plan(), &actual_plan);
        assert_eq!(result.active_plan().unwrap().action_index(), 0);
        assert_eq!(result.active_plan().unwrap().elapsed_ms(), 0);
        assert_eq!(
            result.active_action().unwrap().action(),
            actual_plan.actions()[0]
        );
        assert_eq!(
            result.berries_units(),
            actual
                .start_action(actual_plan.actions()[0])
                .unwrap()
                .berries_units()
        );
        assert_eq!(result.forage_rng, actual.forage_rng);
        assert_eq!(source.berries_units(), 30);
    }

    #[test]
    fn an_empty_first_meal_is_skipped_and_an_exhausted_plan_is_replaced() {
        let citizen = Citizen::new(0.0).unwrap();
        let mut active = ActivePlan {
            plan: Plan {
                actions: vec![CitizenAction::Eat, CitizenAction::Wait],
                action_durations_ms: vec![0; 2],
                goals: Vec::new(),
                average_wellbeing: 0.0,
                decision: None,
            },
            action_index: 0,
            elapsed_ms: 0,
        };
        let next = active.start_next_action(citizen.clone()).unwrap();
        assert_eq!(next.active_action().unwrap().action(), CitizenAction::Wait);
        assert_eq!(active.action_index(), 1);
        assert_eq!(active.elapsed_ms(), 0);
        active.action_index = active.plan.actions.len();
        let next = active.start_next_action(citizen).unwrap();
        assert!(next.active_action().unwrap().remaining_ms() > 0);
        assert_eq!(active.action_index(), 0);
    }

    #[test]
    fn stocked_hungry_citizens_plan_at_most_one_meal_in_the_current_horizon() {
        for (hunger, units) in [(50.0, 200), (150.0, 1000), (500.0, 1000), (500.0, 10_000)] {
            let citizen = Citizen::new(hunger).unwrap().with_berries(units).unwrap();
            let chosen = plan(&citizen).unwrap();
            assert_eq!(
                chosen
                    .actions()
                    .iter()
                    .filter(|action| **action == CitizenAction::Eat)
                    .count(),
                1
            );
        }
    }
}
