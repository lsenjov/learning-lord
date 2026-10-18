use crate::{Citizen, CitizenAction, SimulationError};
pub mod goals;
use goals::{Cooldowns, Effect, Prediction};
mod scheduler;
use std::sync::{Arc, OnceLock};

pub const REPLAN_MIN_NUTRITION: f64 = 20.0;
pub const EAT_PLAN_COOLDOWN_MS: u64 = 4 * 60 * 60 * 1000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct GoalForecast {
    pub actions: Vec<CitizenAction>,
    pub duration_ms: u64,
    pub average_wellbeing: f64,
    pub full_plan_wellbeing: f64,
    pub future_food_benefit: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionLocationForecast {
    pub place: crate::locations::PlaceId,
    pub location_name: String,
    pub forecast: Option<GoalForecast>,
    pub best_for_goal: bool,
    pub selected: bool,
    #[serde(default)]
    pub projection: Option<ProductionProjection>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct GoalDecision {
    pub goal: Effect,
    pub forecast: Option<GoalForecast>,
    pub production_locations: Vec<ProductionLocationForecast>,
}

impl GoalDecision {
    pub fn unavailable_reason(&self) -> Option<&'static str> {
        self.forecast.is_none().then_some(
            "No executable sequence within the four-hour prerequisite limit and action rules.",
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionInputDecision {
    pub good: crate::marketplace::Good,
    pub required_units: crate::Quantity,
    pub carried_units: crate::Quantity,
    pub supply_shortfall: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionDecision {
    pub recipe: crate::production::Recipe,
    pub profit_per_hour: Option<f64>,
    #[serde(default)]
    pub routes: crate::production::ProductionRoutes,
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
        let routes = crate::production::production_routes(recipe, citizen, None);
        Self {
            recipe,
            routes,
            profit_per_hour: crate::production::recipe_profit(recipe, citizen)
                .zip(recipe.duration_ms(citizen).ok())
                .map(|(profit, duration)| profit * 3_600_000.0 / duration as f64),
            remaining_batches: citizen.production_targets().and_then(|t| {
                crate::production::crop_kind(recipe)
                    .is_none()
                    .then(|| t.remaining_batches(recipe))
            }),
            inputs: recipe
                .inputs()
                .iter()
                .map(|&(good, required_units)| {
                    let carried_units = citizen.available_units(good);
                    let obtainable = carried_units
                        .saturating_add(citizen.market().listed_units(citizen.market_party(), good))
                        .saturating_add(
                            citizen
                                .market()
                                .available_units(citizen.market_party(), good),
                        );
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PlanningDecision {
    pub candidates: Vec<GoalDecision>,
    pub selected_goal: Effect,
    pub production: Vec<ProductionDecision>,
    pub prices: crate::marketplace::Prices,
    pub blocked_requests: crate::marketplace::ShoppingList,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionProjection {
    pub recipes: Vec<crate::production::Recipe>,
    pub purchases: crate::marketplace::ShoppingList,
}

impl ProductionProjection {
    fn from_targets(
        citizen: &Citizen,
        targets: &crate::production::ProductionTargets,
    ) -> Result<Self, SimulationError> {
        let requirements = crate::production::recipe_requirements_at(
            citizen,
            targets.site,
            &targets.projected_order,
        )?;
        Ok(Self {
            recipes: targets.projected_order.clone(),
            purchases: crate::marketplace::ShoppingList::new(requirements.items().map(
                |(good, units)| (good, units.saturating_sub(citizen.usable_owned_units(good))),
            ))?,
        })
    }
}

/// A chosen goal and its primitive actions, including prerequisites.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PlanGoal {
    pub goal: Effect,
    pub actions: std::ops::Range<usize>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Plan {
    actions: Vec<CitizenAction>,
    commercial_actions: Vec<usize>,
    action_durations_ms: Vec<u64>,
    goals: Vec<PlanGoal>,
    average_wellbeing: f64,
    decision: Option<Arc<PlanningDecision>>,
}

impl Plan {
    fn claim_commercial_action(
        &self,
        citizen: &mut Citizen,
        index: usize,
    ) -> Result<(), SimulationError> {
        if !self.commercial_actions.contains(&index) {
            return Ok(());
        }
        let action = self.actions[index];
        let recipe = match action {
            CitizenAction::Plant { crop, .. } => Some(crop.recipe()),
            action => action.production_recipe(),
        };
        if let Some(recipe) = recipe {
            let site = action.required_place(citizen);
            let Some(profit) = crate::production::recipe_profit_for_site(recipe, citizen, site)
            else {
                return Err(SimulationError::MissingInputs);
            };
            if profit <= 0.0 && !citizen.eligible_production_trial(recipe, site) {
                return Err(SimulationError::MissingInputs);
            }
            citizen.claim_production_trial(recipe, site)?;
            citizen.start_trial_production(action)?;
        }
        Ok(())
    }

    fn start_action(&self, citizen: &Citizen, index: usize) -> Result<Citizen, SimulationError> {
        let action = self
            .actions
            .get(index)
            .copied()
            .ok_or(SimulationError::PlanningFailed)?;
        let mut candidate = citizen.clone();
        self.claim_commercial_action(&mut candidate, index)?;
        candidate.start_action(action)
    }

    pub(crate) fn maintenance_goal(&self) -> Option<Effect> {
        self.goals
            .first()
            .map(|goal| goal.goal)
            .filter(|goal| matches!(goal, Effect::ListExcess | Effect::ReplenishReserves))
    }

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

    pub(crate) fn production_site(&self) -> Option<crate::locations::PlaceId> {
        let goal = self
            .goals
            .first()
            .filter(|goal| goal.goal == Effect::Production)?;
        productive_site(&self.actions[goal.actions.clone()])
    }

    pub fn actions(&self) -> &[CitizenAction] {
        &self.actions
    }

    pub fn average_wellbeing(&self) -> f64 {
        self.average_wellbeing
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ActivePlan {
    plan: Plan,
    action_index: usize,
    elapsed_ms: u64,
}

impl ActivePlan {
    pub(crate) fn maintenance_goal(&self) -> Option<Effect> {
        self.plan.maintenance_goal()
    }

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

    pub(crate) fn shopping_request(
        &self,
        _remaining_ms: u64,
    ) -> Result<Option<crate::marketplace::ShoppingList>, SimulationError> {
        let Some(goal) = self
            .plan
            .goals
            .iter()
            .find(|goal| goal.actions.contains(&self.action_index))
        else {
            return Ok(None);
        };
        let mut items = Vec::new();
        for index in self.action_index..goal.actions.end {
            match self.plan.actions[index] {
                CitizenAction::Travel(_) => {}
                CitizenAction::BuyAt { list, .. } => items.extend(list.items()),
                _ => break,
            }
        }
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(crate::marketplace::ShoppingList::new(items)?))
        }
    }

    pub(crate) fn travelling_purchase(&self, _remaining_ms: u64) -> Option<CitizenAction> {
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
            let previous = citizen.clone();
            citizen = citizen.advance(step_ms)?;
            if citizen.household.is_none()
                && let Some(goal) = execution.maintenance_goal()
            {
                let worked = match goal {
                    Effect::ListExcess => crate::marketplace::Good::ALL.into_iter().any(|good| {
                        citizen.market.listed_units(citizen.market_party(), good)
                            > previous.market.listed_units(previous.market_party(), good)
                    }),
                    Effect::ReplenishReserves => crate::marketplace::Good::FOOD
                        .into_iter()
                        .chain([
                            crate::marketplace::Good::Water,
                            crate::marketplace::Good::Beer,
                        ])
                        .any(|good| {
                            citizen.stored_available_units(citizen.home(), good)
                                > previous.stored_available_units(previous.home(), good)
                        }),
                    _ => false,
                };
                if worked {
                    citizen.maintenance.record_work(goal, citizen.id);
                }
                if citizen.active_action().is_none() && execution.finishes_committed_goal() {
                    citizen
                        .maintenance
                        .finish(goal, citizen.id, citizen.current_time_ms(), true);
                }
            }
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

    pub(crate) fn abandon_committed_goal(&mut self) {
        self.action_index = self.committed_actions_end();
    }

    pub(crate) fn resume(&mut self, citizen: Citizen) -> Result<Citizen, SimulationError> {
        self.start_next_action(citizen)
    }

    pub(crate) fn committed_actions_end(&self) -> usize {
        self.plan
            .goals
            .first()
            .map_or(self.plan.actions.len(), |goal| goal.actions.end)
    }

    pub(crate) fn finishes_committed_goal(&self) -> bool {
        self.action_index + 1 >= self.committed_actions_end()
    }

    pub(crate) fn needs_replan(&self, citizen: &Citizen) -> bool {
        self.action_index >= self.committed_actions_end()
            || replan_check(
                citizen,
                self.plan.actions[self.action_index],
                self.action_index,
            )
    }

    pub(crate) fn prepare_replan(&self, citizen: &mut Citizen) -> Result<(), SimulationError> {
        citizen.refresh_production_targets(true)
    }

    pub(crate) fn resume_committed(
        &mut self,
        mut citizen: Citizen,
    ) -> Result<Citizen, SimulationError> {
        while !self.needs_replan(&citizen) {
            let started = self.plan.start_action(&citizen, self.action_index);
            citizen = match started {
                Ok(started) => started,
                Err(SimulationError::MissingInputs) => {
                    self.action_index = self.committed_actions_end();
                    return Ok(citizen);
                }
                Err(error) => return Err(error),
            };
            if citizen.active_action().is_some() {
                return Ok(citizen);
            }
            self.action_index += 1;
        }
        Ok(citizen)
    }

    pub(crate) fn first_action_valid(citizen: &Citizen, plan: &Plan) -> bool {
        if plan
            .maintenance_goal()
            .is_some_and(|goal| !citizen.maintenance_available(goal))
        {
            return false;
        }
        plan.actions.first().is_some_and(|action| {
            !replan_check(citizen, *action, 0) && citizen.start_action(*action).is_ok()
        })
    }

    pub(crate) fn adopt(
        &mut self,
        mut citizen: Citizen,
        plan: Plan,
    ) -> Result<Citizen, SimulationError> {
        if !Self::first_action_valid(&citizen, &plan) {
            return Err(SimulationError::MissingInputs);
        }
        set_production_site(&mut citizen, &plan)?;
        let Some(_) = plan.actions.first() else {
            return Err(SimulationError::PlanningFailed);
        };
        let started = plan.start_action(&citizen, 0)?;
        if started.active_action().is_none() {
            return Err(SimulationError::MissingInputs);
        }
        let mut started = started;
        if started.household.is_none()
            && let Some(goal) = plan.maintenance_goal()
        {
            started.maintenance.cancel(started.id);
            started.claim_personal_maintenance(goal);
        }
        self.plan = plan;
        self.action_index = 0;
        self.elapsed_ms = 0;
        Ok(started)
    }

    fn start_next_action(&mut self, mut citizen: Citizen) -> Result<Citizen, SimulationError> {
        let mut freshly_planned = false;
        loop {
            if self.needs_replan(&citizen) {
                if citizen.household.is_none() {
                    citizen.maintenance.cancel(citizen.id);
                }
                self.prepare_replan(&mut citizen)?;
                self.plan = plan(&citizen)?;
                freshly_planned = true;
                set_production_site(&mut citizen, &self.plan)?;
                self.action_index = 0;
                self.elapsed_ms = 0;
                if citizen.household.is_none()
                    && let Some(goal) = self.plan.maintenance_goal()
                {
                    citizen.claim_personal_maintenance(goal);
                }
            }
            let started = self.plan.start_action(&citizen, self.action_index);
            citizen = match started {
                Ok(started) => started,
                Err(SimulationError::MissingInputs) if !freshly_planned => {
                    self.action_index = self.committed_actions_end();
                    continue;
                }
                Err(error) => return Err(error),
            };
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
            CitizenAction::Eat => citizen.home_meal_nutrition() < REPLAN_MIN_NUTRITION,
            CitizenAction::Drink => {
                citizen.position() != citizen.map().position(citizen.home())
                    || citizen.home_drink_units() == 0
            }
            CitizenAction::EquipClothing => {
                citizen.available_units(crate::marketplace::Good::FlaxGarment) == 0
            }
            CitizenAction::Produce(recipe) | CitizenAction::ProduceAt { recipe, .. } => recipe
                .inputs()
                .iter()
                .any(|&(good, units)| citizen.available_units(good) < units),
            action if action.crop_plot().is_some() => citizen.farming_duration_ms(action).is_err(),
            _ => false,
        }
}

fn worker_count(available: Option<usize>) -> usize {
    available.map_or(2, |count| count.saturating_sub(4).max(2))
}

fn planning_pool() -> Result<&'static scheduler::Scheduler, SimulationError> {
    static POOL: OnceLock<Result<scheduler::Scheduler, SimulationError>> = OnceLock::new();
    POOL.get_or_init(|| {
        scheduler::Scheduler::new(worker_count(
            std::thread::available_parallelism().ok().map(usize::from),
        ))
    })
    .as_ref()
    .map_err(|error| *error)
}

type GoalEvaluation = (
    Plan,
    GoalDecision,
    Vec<ProductionDecision>,
    crate::marketplace::ShoppingList,
);

pub(crate) struct PlanningRequest {
    roots: scheduler::Request<GoalEvaluation>,
    citizen: Arc<Citizen>,
    production: Vec<ProductionDecision>,
    priority_listing: bool,
    prior_elapsed: std::time::Duration,
}

impl PlanningRequest {
    pub(crate) fn cancel(&self) {
        self.roots.cancel();
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.prior_elapsed + self.roots.elapsed()
    }

    pub(crate) fn try_result(&mut self) -> Option<Result<Plan, SimulationError>> {
        let results = self.roots.try_result()?;
        let result = reduce_results(&self.citizen, std::mem::take(&mut self.production), results);
        if self.priority_listing && result.as_ref().is_ok_and(|plan| plan.actions.is_empty()) {
            let elapsed = self.elapsed();
            match request_with_priority_listing(&self.citizen, false) {
                Ok(mut fallback) => {
                    fallback.prior_elapsed = elapsed;
                    *self = fallback;
                    None
                }
                Err(error) => Some(Err(error)),
            }
        } else {
            Some(result)
        }
    }

    fn wait(self) -> Result<Plan, SimulationError> {
        let results = self.roots.wait()?;
        let plan = reduce_results(&self.citizen, self.production, results)?;
        if self.priority_listing && plan.actions.is_empty() {
            request_with_priority_listing(&self.citizen, false)?.wait()
        } else {
            Ok(plan)
        }
    }
}

pub fn plan(citizen: &Citizen) -> Result<Plan, SimulationError> {
    request(citizen)?.wait()
}

pub(crate) fn request(citizen: &Citizen) -> Result<PlanningRequest, SimulationError> {
    request_with_priority_listing(citizen, true)
}

fn request_with_priority_listing(
    citizen: &Citizen,
    allow_priority_listing: bool,
) -> Result<PlanningRequest, SimulationError> {
    if citizen.active_action().is_some() || citizen.active_plan().is_some() {
        return Err(SimulationError::CitizenBusy);
    }
    let production: Vec<_> = citizen
        .available_production_recipes()
        .map(|recipe| ProductionDecision::new(citizen, recipe))
        .collect();
    let goals = root_goals_with_priority(citizen, allow_priority_listing)?;
    let priority_listing = goals == [Effect::ListExcess];
    let citizen = Arc::new(citizen.clone());
    let jobs: Vec<scheduler::Job<GoalEvaluation>> = goals
        .into_iter()
        .map(|goal| {
            let citizen = Arc::clone(&citizen);
            let production = production.clone();
            Box::new(move || evaluate_goal(&citizen, goal, production))
                as scheduler::Job<GoalEvaluation>
        })
        .collect();
    let roots = match planning_pool() {
        Ok(pool) => pool.submit(jobs),
        Err(_) => scheduler::Request::completed(
            jobs.into_iter()
                .map(|job| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                        .unwrap_or(Err(SimulationError::PlanningFailed))
                })
                .collect(),
        ),
    };
    Ok(PlanningRequest {
        roots,
        citizen,
        production,
        priority_listing,
        prior_elapsed: std::time::Duration::ZERO,
    })
}

fn root_goals_with_priority(
    citizen: &Citizen,
    allow_priority_listing: bool,
) -> Result<Vec<Effect>, SimulationError> {
    let clothing_choice = citizen.available_units(crate::marketplace::Good::FlaxGarment) > 0
        && citizen.clothing_need() > 0.0;
    let standing_listing = allow_priority_listing
        && citizen
            .maintenance
            .available(Effect::ListExcess, citizen.current_time_ms())
        && !clothing_choice
        && citizen.production_targets().is_some()
        && citizen.thirst() < crate::production::URGENT_HUNGER
        && citizen.hunger() < crate::production::URGENT_HUNGER
        && citizen.tiredness() < crate::production::URGENT_TIREDNESS
        && citizen.excess_value()? >= crate::production::MIN_LISTING_VALUE;
    let goals: &[Effect] = if standing_listing {
        &[Effect::ListExcess]
    } else {
        &[
            Effect::ReduceHunger,
            Effect::ReduceThirst,
            Effect::ReduceTiredness,
            Effect::ReplenishReserves,
            Effect::ReduceClothingNeed,
            Effect::Production,
        ]
    };
    let mut goals = goals.to_vec();
    if clothing_choice && citizen.production_targets().is_some() {
        goals.push(Effect::ListExcess);
    }
    goals.retain(|goal| citizen.maintenance_available(*goal));
    Ok(goals)
}

fn reduce_results(
    citizen: &Citizen,
    mut production: Vec<ProductionDecision>,
    results: Vec<Result<GoalEvaluation, SimulationError>>,
) -> Result<Plan, SimulationError> {
    let mut best = empty_plan();
    let mut candidates = Vec::new();
    let mut selected_goal = Effect::ReduceTiredness;
    let mut blocked_requests = crate::marketplace::ShoppingList::default();
    for result in results {
        let (goal_best, candidate, diagnostics, blocked) = result?;
        blocked_requests = crate::marketplace::ShoppingList::new(
            crate::marketplace::Good::ALL
                .into_iter()
                .map(|good| (good, blocked_requests.units(good).max(blocked.units(good)))),
        )?;
        for (combined, diagnostic) in production.iter_mut().zip(diagnostics) {
            combined.prefix_attempted |= diagnostic.prefix_attempted;
            combined.preparation_limit_observed |= diagnostic.preparation_limit_observed;
            combined.feasible_sequence_found |= diagnostic.feasible_sequence_found;
            if let Some(score) = diagnostic.best_competing_score {
                combined.best_competing_score = Some(
                    combined
                        .best_competing_score
                        .map_or(score, |old| old.max(score)),
                );
            }
        }
        if goal_best.average_wellbeing > best.average_wellbeing {
            best = goal_best;
            selected_goal = candidate.goal;
        }
        candidates.push(candidate);
    }
    let selected_site = best.production_site();
    for candidate in &mut candidates {
        for location in &mut candidate.production_locations {
            location.selected =
                selected_goal == Effect::Production && selected_site == Some(location.place);
        }
    }
    for diagnostic in &mut production {
        diagnostic.selected = contains_production(citizen, &best.actions, diagnostic.recipe);
    }
    best.decision = Some(Arc::new(PlanningDecision {
        candidates,
        selected_goal,
        production,
        prices: citizen.prices(),
        blocked_requests,
    }));
    Ok(best)
}

fn productive_site(actions: &[CitizenAction]) -> Option<crate::locations::PlaceId> {
    actions.iter().find_map(|action| match *action {
        CitizenAction::ProduceAt { place, .. } => Some(place),
        action => action.crop_plot().map(|(field, _)| field),
    })
}

fn set_production_site(citizen: &mut Citizen, plan: &Plan) -> Result<(), SimulationError> {
    citizen.cancel_preparing_trials();
    if let Some(site) = plan.production_site() {
        citizen.production_targets = Some(crate::production::ProductionTargets::calculate_at(
            citizen,
            citizen.current_time_ms(),
            Some(site),
        )?);
    }
    for &index in &plan.commercial_actions {
        if index
            >= plan
                .goals
                .first()
                .map_or(plan.actions.len(), |goal| goal.actions.end)
        {
            continue;
        }
        let action = plan.actions[index];
        let recipe = match action {
            CitizenAction::Plant { crop, .. } => Some(crop.recipe()),
            action => action.production_recipe(),
        };
        if let Some(recipe) = recipe
            && citizen.eligible_production_trial(recipe, action.required_place(citizen))
        {
            citizen.claim_production_trial(recipe, action.required_place(citizen))?;
        }
    }
    Ok(())
}

fn contains_production(
    citizen: &Citizen,
    actions: &[CitizenAction],
    recipe: crate::production::Recipe,
) -> bool {
    actions.iter().any(|&action| match action {
        CitizenAction::Produce(produced)
        | CitizenAction::ProduceAt {
            recipe: produced, ..
        } => produced == recipe,
        CitizenAction::Plant { crop, .. } => crop.recipe() == recipe,
        CitizenAction::Tend { field, plot, .. } | CitizenAction::Harvest { field, plot } => citizen
            .crop(field, plot)
            .ok()
            .flatten()
            .is_some_and(|crop| crop.kind.recipe() == recipe),
        _ => false,
    })
}

fn evaluate_goal(
    citizen: &Citizen,
    goal: Effect,
    mut production: Vec<ProductionDecision>,
) -> Result<GoalEvaluation, SimulationError> {
    let mut forecast = None;
    let mut production_locations = Vec::new();
    if goal == Effect::Production {
        for place in citizen.production_sites() {
            production_locations.push(ProductionLocationForecast {
                place,
                location_name: citizen.map().place(place)?.name.clone(),
                forecast: None,
                best_for_goal: false,
                selected: false,
                projection: None,
            });
        }
    }
    let mut goal_best = empty_plan();
    let initial = Prediction::new(citizen, Cooldowns::default());
    let blocked_requests = initial.blocked_requests.clone();
    let variants = if goal == Effect::Production {
        goals::production_prefixes_with_locations(
            initial,
            0,
            &mut production,
            &mut production_locations,
        )?
    } else {
        goals::variants_after(&initial, goal)?
    };
    for variant in variants {
        for diagnostic in &mut production {
            diagnostic.feasible_sequence_found |=
                contains_production(citizen, &variant.actions, diagnostic.recipe);
        }
        let actions = variant.actions.clone();
        let duration_ms = variant.elapsed_ms;
        let average_wellbeing = variant.average()?;
        let future_food_benefit = variant.citizen.food_forecast(6).benefit;
        let mut continuation = empty_plan();
        let boundary = PlanGoal {
            goal,
            actions: 0..variant.actions.len(),
        };
        search_with_goals(variant, &mut continuation, vec![boundary])?;
        if continuation.average_wellbeing.is_finite() {
            for diagnostic in &mut production {
                if contains_production(citizen, &continuation.actions, diagnostic.recipe) {
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
        let candidate_forecast = GoalForecast {
            actions,
            duration_ms,
            average_wellbeing,
            full_plan_wellbeing: continuation.average_wellbeing,
            future_food_benefit,
        };
        if continuation.average_wellbeing.is_finite()
            && let Some(site) = productive_site(&candidate_forecast.actions)
            && let Some(location) = production_locations
                .iter_mut()
                .find(|location| location.place == site)
        {
            location.forecast = Some(candidate_forecast.clone());
        }
        if continuation.average_wellbeing > goal_best.average_wellbeing {
            forecast = Some(candidate_forecast);
            goal_best = continuation;
        }
    }
    if let Some(site) = goal_best.production_site()
        && let Some(location) = production_locations
            .iter_mut()
            .find(|location| location.place == site)
    {
        location.best_for_goal = true;
    }
    let blocked_requests = *blocked_requests.borrow();
    Ok((
        goal_best,
        GoalDecision {
            goal,
            forecast,
            production_locations,
        },
        production,
        blocked_requests,
    ))
}

impl ActivePlan {
    pub(crate) fn validate_save(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        if self.action_index >= self.plan.actions.len()
            || self.plan.actions.len() != self.plan.action_durations_ms.len()
            || self.plan.commercial_actions.iter().any(|index| {
                self.plan.actions.get(*index).is_none_or(|action| {
                    action.production_recipe().is_none()
                        && !matches!(action, CitizenAction::Plant { .. })
                })
            })
            || !self.plan.average_wellbeing.is_finite()
            || self.plan.goals.iter().any(|goal| {
                goal.actions.start >= goal.actions.end || goal.actions.end > self.plan.actions.len()
            })
        {
            return Err(SaveError::InvalidState("invalid active plan"));
        }
        Ok(())
    }
}

#[cfg(test)]
fn sequential_reference(citizen: &Citizen) -> Result<Plan, SimulationError> {
    let production = citizen
        .available_production_recipes()
        .map(|recipe| ProductionDecision::new(citizen, recipe))
        .collect::<Vec<_>>();
    let goals = root_goals_with_priority(citizen, true)?;
    let priority_listing = goals == [Effect::ListExcess];
    let results = goals
        .into_iter()
        .map(|goal| evaluate_goal(citizen, goal, production.clone()))
        .collect();
    let plan = reduce_results(citizen, production.clone(), results)?;
    if priority_listing && plan.actions.is_empty() {
        let results = root_goals_with_priority(citizen, false)?
            .into_iter()
            .map(|goal| evaluate_goal(citizen, goal, production.clone()))
            .collect();
        reduce_results(citizen, production, results)
    } else {
        Ok(plan)
    }
}

fn ordinary_goals() -> [Effect; 5] {
    [
        Effect::ReduceHunger,
        Effect::ReduceThirst,
        Effect::ReduceTiredness,
        Effect::ReduceClothingNeed,
        Effect::Production,
    ]
}

fn empty_plan() -> Plan {
    Plan {
        actions: Vec::new(),
        action_durations_ms: Vec::new(),
        commercial_actions: Vec::new(),
        goals: Vec::new(),
        average_wellbeing: f64::NEG_INFINITY,
        decision: None,
    }
}

#[cfg(test)]
fn search(state: Prediction, goal: Effect, best: &mut Plan) -> Result<(), SimulationError> {
    let end = state.actions.len();
    search_with_goals(
        state,
        best,
        vec![PlanGoal {
            goal,
            actions: 0..end,
        }],
    )
}

fn search_with_goals(
    state: Prediction,
    best: &mut Plan,
    boundaries: Vec<PlanGoal>,
) -> Result<(), SimulationError> {
    let score = state.average()?;
    if !state.actions.is_empty() && score > best.average_wellbeing {
        *best = Plan {
            actions: state.actions.clone(),
            action_durations_ms: state.action_durations_ms.clone(),
            commercial_actions: state.commercial_actions.clone(),
            goals: boundaries.clone(),
            average_wellbeing: score,
            decision: None,
        };
    }
    if boundaries.len() >= 2 {
        return Ok(());
    }
    for goal in ordinary_goals() {
        if boundaries.iter().any(|boundary| boundary.goal == goal) {
            continue;
        }
        for mut next in goals::variants_after(&state, goal)? {
            next.elapsed_ms += state.elapsed_ms;
            let mut actions = state.actions.clone();
            actions.append(&mut next.actions);
            let mut durations = state.action_durations_ms.clone();
            durations.append(&mut next.action_durations_ms);
            next.action_durations_ms = durations;
            let mut next_boundaries = boundaries.clone();
            next_boundaries.push(PlanGoal {
                goal,
                actions: state.actions.len()..actions.len(),
            });
            next.commercial_actions = state
                .commercial_actions
                .iter()
                .copied()
                .chain(
                    next.commercial_actions
                        .iter()
                        .map(|index| index + state.actions.len()),
                )
                .collect();
            next.actions = actions;
            search_with_goals(next, best, next_boundaries)?;
        }
    }
    Ok(())
}

pub(crate) fn start(citizen: &Citizen) -> Result<Citizen, SimulationError> {
    let plan = plan(citizen)?;
    let mut citizen = citizen.clone();
    set_production_site(&mut citizen, &plan)?;
    let mut citizen = plan.start_action(&citizen, 0)?;
    if citizen.household.is_none()
        && let Some(goal) = plan.maintenance_goal()
    {
        citizen.claim_personal_maintenance(goal);
    }
    citizen.active_plan = Some(ActivePlan {
        plan,
        action_index: 0,
        elapsed_ms: 0,
    });
    Ok(citizen)
}

#[cfg(test)]
pub(crate) fn gated_request_fixture(
    citizen: &Citizen,
    started: std::sync::mpsc::Sender<()>,
    gate: std::sync::mpsc::Receiver<()>,
) -> PlanningRequest {
    let citizen = Arc::new(citizen.clone());
    let evaluated = Arc::clone(&citizen);
    let roots = planning_pool().unwrap().submit(vec![Box::new(move || {
        started.send(()).unwrap();
        gate.recv().unwrap();
        evaluate_goal(&evaluated, Effect::ReduceTiredness, Vec::new())
    })]);
    PlanningRequest {
        roots,
        citizen,
        production: Vec::new(),
        priority_listing: false,
        prior_elapsed: std::time::Duration::ZERO,
    }
}

#[cfg(test)]
pub(crate) fn executing_fixture(citizen: &Citizen, actions: Vec<CitizenAction>) -> Citizen {
    let mut citizen = citizen.start_action(actions[0]).unwrap();
    citizen.active_plan = Some(ActivePlan {
        plan: Plan {
            action_durations_ms: vec![0; actions.len()],
            commercial_actions: Vec::new(),
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

#[cfg(test)]
pub(crate) fn adopt_commercial_fixture(
    citizen: &Citizen,
    actions: Vec<CitizenAction>,
    commercial_actions: Vec<usize>,
) -> Result<Citizen, SimulationError> {
    let plan = Plan {
        action_durations_ms: vec![0; actions.len()],
        commercial_actions,
        goals: vec![PlanGoal {
            goal: Effect::Production,
            actions: 0..actions.len(),
        }],
        actions,
        average_wellbeing: -1.0,
        decision: None,
    };
    let mut citizen = citizen.clone();
    set_production_site(&mut citizen, &plan)?;
    plan.start_action(&citizen, 0)
}

#[cfg(test)]
pub(crate) fn resume_commercial_fixture(
    citizen: Citizen,
    action: CitizenAction,
) -> Result<Citizen, SimulationError> {
    let mut execution = ActivePlan {
        plan: Plan {
            actions: vec![action],
            action_durations_ms: vec![0],
            commercial_actions: vec![0],
            goals: vec![PlanGoal {
                goal: Effect::Production,
                actions: 0..1,
            }],
            average_wellbeing: -1.0,
            decision: None,
        },
        action_index: 0,
        elapsed_ms: 0,
    };
    execution.resume_committed(citizen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::Xoshiro256PlusPlus};

    const MINUTE_MS: u64 = 60_000;
    const TWO_HOURS_MS: u64 = 2 * 60 * MINUTE_MS;

    fn blocked_priority_listing() -> Citizen {
        use crate::{
            StartingRole,
            locations::{Location, Map, ObjectKind, Position},
            marketplace::Good,
        };
        let mut citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Tailor)
            .with_good(Good::Berries, 20_000)
            .unwrap();
        let mut map = Map::default();
        let market = map.public_place(Location::Market);
        map.places_mut()
            .get_mut(&market)
            .unwrap()
            .outdoor_objects
            .retain(|object| object.kind != ObjectKind::PantryShelving);
        let (map, _) = map
            .with_place(
                Location::Tailory,
                Position::default(),
                Some(citizen.id()),
                "Tailory",
            )
            .unwrap();
        citizen = citizen
            .with_map(map)
            .unwrap()
            .with_home_good(Good::BreadRoll, 5)
            .unwrap();
        citizen.refresh_production_targets(true).unwrap();
        citizen
    }

    #[test]
    fn infeasible_priority_listing_uses_normal_roots_and_missing_actions_return_an_error() {
        let citizen = blocked_priority_listing();
        assert_eq!(
            root_goals_with_priority(&citizen, true).unwrap(),
            vec![Effect::ListExcess]
        );
        assert!(
            goals::variants_after(
                &Prediction::new(&citizen, Cooldowns::default()),
                Effect::ListExcess
            )
            .unwrap()
            .is_empty()
        );
        let result = plan(&citizen).unwrap();
        assert!(!result.actions().is_empty());
        assert_ne!(result.goals()[0].goal, Effect::ListExcess);
        assert!(citizen.start_planning().is_ok());
        assert_eq!(
            empty_plan().start_action(&citizen, 0),
            Err(SimulationError::PlanningFailed)
        );
    }

    #[test]
    fn polled_listing_fallback_keeps_elapsed_time_and_cancels_its_current_phase() {
        let citizen = blocked_priority_listing();
        let mut pending = request(&citizen).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut elapsed = pending.elapsed();
        while pending.priority_listing {
            assert!(pending.try_result().is_none());
            let next = pending.elapsed();
            assert!(next >= elapsed);
            elapsed = next;
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        pending.cancel();
        assert!(pending.try_result().is_none());
        let mut pending = request(&citizen).unwrap();
        loop {
            if let Some(result) = pending.try_result() {
                assert!(!result.unwrap().actions().is_empty());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_legacy_away_drink_plan_replans_before_consumption() {
        use crate::{
            locations::{Location, Map, Position},
            marketplace::Good,
        };
        let map = Map::new(
            Position::default(),
            Position { x: 100.0, y: 0.0 },
            Position::default(),
        )
        .unwrap();
        let source = Citizen::new(0.0)
            .unwrap()
            .with_thirst(80.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap()
            .with_home_good(Good::Water, 400)
            .unwrap()
            .with_position(map.position(map.public_place(Location::River)))
            .unwrap();
        let legacy = executing(
            &source,
            vec![
                CitizenAction::Wait,
                CitizenAction::Drink,
                CitizenAction::Sleep,
            ],
        );
        let next = legacy.advance(crate::ACTION_DURATION_MS).unwrap();
        assert_eq!(next.active_plan().unwrap().action_index(), 0);
        assert_eq!(
            next.active_action().unwrap().action(),
            CitizenAction::Travel(source.home())
        );
    }

    #[test]
    fn synchronous_replan_installs_the_new_selected_workplace_targets() {
        use crate::{
            StartingRole,
            locations::{Location, Position},
            marketplace::Good,
            production::{ProductionTargets, Recipe},
        };
        let mut worker = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Miller)
            .with_good(Good::Wheat, 600)
            .unwrap()
            .with_good(Good::Berries, 240)
            .unwrap()
            .with_good(Good::Water, 400)
            .unwrap();
        worker = worker.with_prices(
            worker
                .prices()
                .with_price(Good::Wheat, 1.0)
                .unwrap()
                .with_price(Good::Flour, 10000.0)
                .unwrap(),
        );
        let (map, mill) = worker
            .map()
            .with_place(
                Location::Mill,
                Position::default(),
                Some(worker.id()),
                "Mill",
            )
            .unwrap();
        worker = worker.with_map(map).unwrap();
        let forest = worker.map().public_place(Location::Forest);
        worker.production_targets =
            Some(ProductionTargets::calculate_at(&worker, 0, Some(forest)).unwrap());
        let mut execution = ActivePlan {
            plan: empty_plan(),
            action_index: 0,
            elapsed_ms: 0,
        };
        let started = execution.start_next_action(worker).unwrap();
        assert_eq!(execution.plan.production_site(), Some(mill));
        assert_eq!(started.production_targets().unwrap().site, Some(mill));
        assert_eq!(
            started
                .production_targets()
                .unwrap()
                .remaining_batches(Recipe::Forage),
            0
        );
    }

    #[test]
    fn production_diagnostics_report_each_location_and_keep_one_goal() {
        use crate::{
            StartingRole,
            locations::{Location, Position},
            marketplace::Good,
            production::Recipe,
        };
        let worker = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Miller)
            .with_good(Good::Wheat, 600)
            .unwrap();
        let (map, first) = worker
            .map()
            .with_place(
                Location::Mill,
                Position::default(),
                Some(worker.id()),
                "First mill",
            )
            .unwrap();
        let (map, second) = map
            .with_place(
                Location::Mill,
                Position::default(),
                Some(worker.id()),
                "Second mill",
            )
            .unwrap();
        let worker = worker.with_map(map).unwrap();
        let (_, decision, _, _) = evaluate_goal(&worker, Effect::Production, Vec::new()).unwrap();
        assert_eq!(decision.goal, Effect::Production);
        assert_eq!(
            decision
                .production_locations
                .iter()
                .filter(|location| location.best_for_goal)
                .count(),
            1
        );
        for site in [first, second] {
            let location = decision
                .production_locations
                .iter()
                .find(|location| location.place == site)
                .unwrap();
            let forecast = location.forecast.as_ref().unwrap();
            assert!(forecast.actions.contains(&CitizenAction::ProduceAt {
                place: site,
                recipe: Recipe::MillFlour
            }));
            assert!(forecast.duration_ms > 0);
            assert!(forecast.average_wellbeing.is_finite());
            assert!(forecast.full_plan_wellbeing.is_finite());
        }
        let plan = plan(&worker).unwrap();
        assert_eq!(
            plan.decision()
                .unwrap()
                .candidates
                .iter()
                .filter(|candidate| candidate.goal == Effect::Production)
                .count(),
            1
        );
        let selected = plan
            .decision()
            .unwrap()
            .candidates
            .iter()
            .flat_map(|candidate| &candidate.production_locations)
            .filter(|location| location.selected)
            .count();
        assert_eq!(
            selected,
            usize::from(plan.decision().unwrap().selected_goal == Effect::Production)
        );
    }

    #[test]
    fn replanning_refreshes_same_day_ingredient_budgets_after_output_stock_changes() {
        use crate::{StartingRole, locations::Location, marketplace::Good, production::Recipe};
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_good(Good::Flour, 1200)
            .unwrap()
            .with_good(Good::Wood, 300)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap();
        let citizen = citizen.with_prices(citizen.prices().with_price(Good::Flour, 300.0).unwrap());
        let (map, _) = citizen
            .map()
            .with_place(
                Location::Bakery,
                citizen.position(),
                Some(citizen.id()),
                "Bakery",
            )
            .unwrap();
        let mut citizen = citizen.with_map(map).unwrap();
        citizen.refresh_production_targets(true).unwrap();
        assert!(
            citizen
                .production_targets()
                .unwrap()
                .remaining_batches(Recipe::BakeRolls)
                > 0
        );
        citizen = citizen.with_good(Good::BreadRoll, 120).unwrap();
        let execution = ActivePlan {
            plan: empty_plan(),
            action_index: 0,
            elapsed_ms: 0,
        };
        execution.prepare_replan(&mut citizen).unwrap();
        assert_eq!(
            citizen
                .production_targets()
                .unwrap()
                .remaining_batches(Recipe::BakeRolls),
            0
        );
    }

    #[test]
    fn worker_count_reserves_four_threads_with_a_minimum_of_two() {
        for (available, expected) in [
            (None, 2),
            (Some(1), 2),
            (Some(4), 2),
            (Some(5), 2),
            (Some(6), 2),
            (Some(7), 3),
            (Some(16), 12),
        ] {
            assert_eq!(worker_count(available), expected);
        }
        assert_eq!(
            planning_pool().unwrap().worker_count(),
            worker_count(std::thread::available_parallelism().ok().map(usize::from))
        );
    }

    #[test]
    fn parallel_plans_match_sequential_evaluation_and_preserve_snapshots() {
        use crate::{
            StartingRole,
            marketplace::{Good, Market},
        };
        let hungry = Citizen::with_needs(70.0, 10.0)
            .unwrap()
            .with_berries(33)
            .unwrap();
        let seller = Citizen::new(0.0).unwrap();
        let mut market = Market::default();
        market
            .list(
                seller.id(),
                hungry
                    .map()
                    .public_place(crate::locations::Location::Market),
                Good::BreadRoll,
                8,
            )
            .unwrap();
        market
            .list(
                seller.id(),
                hungry
                    .map()
                    .public_place(crate::locations::Location::Market),
                Good::FlaxGarment,
                2,
            )
            .unwrap();
        let trader = hungry.with_coins(100).unwrap().with_market(market);
        let mut tailor = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Tailor)
            .with_good(Good::FlaxGarment, 1)
            .unwrap();
        tailor.refresh_production_targets(true).unwrap();
        let mut producer = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Tailor)
            .with_good(Good::Cloth, 800)
            .unwrap()
            .with_good(Good::Thread, 800)
            .unwrap()
            .with_good(Good::FlaxGarment, 1)
            .unwrap();
        producer.refresh_production_targets(true).unwrap();
        for citizen in [
            Citizen::new(0.0).unwrap(),
            hungry,
            trader,
            tailor,
            producer,
            Citizen::with_needs(0.0, f64::MAX).unwrap(),
        ] {
            let snapshot = citizen.clone();
            let expected = sequential_reference(&citizen).unwrap();
            for _ in 0..2 {
                assert_eq!(plan(&citizen).unwrap(), expected);
                assert_eq!(citizen, snapshot);
            }
        }
    }

    #[test]
    fn parallel_planning_preserves_errors() {
        let busy = Citizen::new(0.0)
            .unwrap()
            .start_action(CitizenAction::Wait)
            .unwrap();
        let overflow = Citizen::with_hunger_rate(0.0, f64::MAX).unwrap();
        for citizen in [busy, overflow] {
            let expected = sequential_reference(&citizen).unwrap_err();
            assert_eq!(plan(&citizen).unwrap_err(), expected);
        }
    }

    #[test]
    fn shopping_requests_cover_remaining_stops_within_the_current_goal() {
        use crate::marketplace::{Good, ShoppingList};
        let citizen = Citizen::new(0.0).unwrap();
        let place = citizen.selling_place();
        let mut execution = ActivePlan {
            plan: Plan {
                actions: vec![
                    CitizenAction::Travel(place),
                    CitizenAction::BuyAt {
                        place,
                        list: ShoppingList::single(Good::BreadRoll, 1),
                    },
                    CitizenAction::Travel(place),
                    CitizenAction::BuyAt {
                        place,
                        list: ShoppingList::single(Good::BerryPie, 1),
                    },
                    CitizenAction::BuyAt {
                        place,
                        list: ShoppingList::single(Good::BreadRoll, 10),
                    },
                ],
                commercial_actions: Vec::new(),
                action_durations_ms: vec![
                    60_000,
                    crate::TRADE_DURATION_MS,
                    60_000,
                    crate::TRADE_DURATION_MS,
                    crate::TRADE_DURATION_MS,
                ],
                goals: vec![
                    PlanGoal {
                        goal: Effect::ReplenishReserves,
                        actions: 0..4,
                    },
                    PlanGoal {
                        goal: Effect::ReduceHunger,
                        actions: 4..5,
                    },
                ],
                average_wellbeing: 0.0,
                decision: None,
            },
            action_index: 0,
            elapsed_ms: 0,
        };
        let request = execution.shopping_request(1).unwrap().unwrap();
        assert_eq!(request.units(Good::BreadRoll), 1);
        assert_eq!(request.units(Good::BerryPie), 1);
        execution.action_index = 2;
        assert_eq!(
            execution
                .shopping_request(1)
                .unwrap()
                .unwrap()
                .units(Good::BreadRoll),
            0
        );
        execution.elapsed_ms = TWO_HOURS_MS + 1;
        assert_eq!(
            execution
                .shopping_request(1)
                .unwrap()
                .unwrap()
                .units(Good::BerryPie),
            1
        );
        execution.action_index = 3;
        assert_eq!(
            execution
                .shopping_request(1)
                .unwrap()
                .unwrap()
                .units(Good::BerryPie),
            1
        );

        execution.action_index = 1;
        let request = execution.shopping_request(1).unwrap().unwrap();
        assert_eq!(request.units(Good::BreadRoll), 1);
        assert_eq!(request.units(Good::BerryPie), 1);
        execution.action_index = 0;
        execution.elapsed_ms = TWO_HOURS_MS - crate::TRADE_DURATION_MS - 2;
        let request = execution.shopping_request(1).unwrap().unwrap();
        assert_eq!(request.units(Good::BreadRoll), 1);
        assert_eq!(request.units(Good::BerryPie), 1);
    }

    fn executing(citizen: &Citizen, actions: Vec<CitizenAction>) -> Citizen {
        let mut citizen = citizen.start_action(actions[0]).unwrap();
        citizen.active_plan = Some(ActivePlan {
            plan: Plan {
                action_durations_ms: vec![0; actions.len()],
                commercial_actions: Vec::new(),
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
    fn goal_completion_compares_wearing_an_acquired_garment_with_listing_it() {
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
        planned.active_plan.as_mut().unwrap().plan.goals = vec![
            PlanGoal {
                goal: Effect::Production,
                actions: 0..1,
            },
            PlanGoal {
                goal: Effect::ReduceClothingNeed,
                actions: 1..2,
            },
        ];
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
    fn selected_plans_use_each_goal_once_and_reset_on_replanning() {
        let source = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_berries(300)
            .unwrap();
        for citizen in [&source, &source.with_coins(300).unwrap()] {
            let first = plan(citizen).unwrap();
            assert!(!first.actions().is_empty());
            assert!(first.goals().len() <= 2);
            for (index, goal) in first.goals().iter().enumerate() {
                assert!(
                    !first.goals()[..index]
                        .iter()
                        .any(|old| old.goal == goal.goal)
                );
            }
            assert_eq!(plan(citizen).unwrap(), first);
        }
    }

    #[test]
    fn accepted_blocked_requests_merge_by_maximum_and_replace_on_replanning() {
        use crate::{
            AgentKind, Universe,
            marketplace::{Good, ShoppingList},
        };
        let source = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_coins(1000)
            .unwrap();
        let forecast = plan(&source).unwrap();
        assert_eq!(
            forecast
                .decision()
                .unwrap()
                .blocked_requests
                .units(Good::FlaxGarment),
            1
        );
        assert_eq!(
            source.market().requested(source.id()),
            ShoppingList::default()
        );
        let mut buyer = executing(
            &source,
            vec![CitizenAction::Buy(ShoppingList::single(Good::BreadRoll, 2))],
        );
        let mut decision = forecast.decision().unwrap().clone();
        decision.blocked_requests =
            ShoppingList::new([(Good::BreadRoll, 3), (Good::FlaxGarment, 1)]).unwrap();
        buyer.active_plan.as_mut().unwrap().plan.decision = Some(Arc::new(decision));
        let (mut universe, id) = Universe::with_map(buyer.map())
            .with_citizen("Buyer", buyer)
            .unwrap();
        assert_eq!(universe.market().requested(id).units(Good::BreadRoll), 3);
        assert_eq!(universe.market().requested(id).units(Good::FlaxGarment), 1);
        let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
        citizen.active_plan.as_mut().unwrap().plan.decision = None;
        universe.register_action_request(id).unwrap();
        assert_eq!(universe.market().requested(id).units(Good::BreadRoll), 2);
        assert_eq!(universe.market().requested(id).units(Good::FlaxGarment), 0);
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
        let purchase = CitizenAction::Buy(ShoppingList::single(Good::BreadRoll, 60));
        let source = Citizen::with_needs(-50.0, -100.0)
            .unwrap()
            .with_map(map.clone())
            .unwrap();
        let mut buyer = executing(&source, vec![travel, purchase]);
        let execution = buyer.active_plan.as_mut().unwrap();
        execution.plan.goals = vec![
            PlanGoal {
                goal: Effect::Production,
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
            (TWO_HOURS_MS - remaining_ms - 1, Some(purchase)),
            (TWO_HOURS_MS - remaining_ms, Some(purchase)),
            (TWO_HOURS_MS - remaining_ms + 1, Some(purchase)),
        ] {
            execution.elapsed_ms = elapsed_ms;
            assert_eq!(execution.travelling_purchase(remaining_ms), expected);
        }
        let (universe, id) = Universe::with_map(buyer.map())
            .with_citizen("Buyer", buyer)
            .unwrap();
        assert_eq!(
            universe.market().requested(id),
            ShoppingList::single(Good::BreadRoll, 60)
        );
        let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
        let arrived = universe
            .advance(citizen.active_action().unwrap().remaining_ms())
            .unwrap();
        let AgentKind::Citizen(citizen) = &arrived.agents()[&id].kind;
        assert!(citizen.active_plan().unwrap().elapsed_ms() > TWO_HOURS_MS);
        assert_eq!(
            arrived.market().requested(id),
            ShoppingList::single(Good::BreadRoll, 60)
        );
        let finished = arrived.advance(crate::TRADE_DURATION_MS).unwrap();
        assert_ne!(
            match &finished.agents()[&id].kind {
                AgentKind::Citizen(citizen) => citizen.active_plan().unwrap().plan(),
            },
            citizen.active_plan().unwrap().plan()
        );
    }

    #[test]
    fn goal_ranges_include_prerequisites_and_cover_repeated_primitives() {
        use crate::marketplace::Prices;
        let citizen = Citizen::with_needs(70.0, -100.0)
            .unwrap()
            .with_thirst(-100.0)
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
            .with_home_good(crate::marketplace::Good::Berries, 310)
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
        assert_eq!(advanced.active_plan().unwrap().action_index(), 0);
        assert_eq!(advanced.active_plan().unwrap().elapsed_ms(), 0);
        assert_ne!(advanced.active_plan().unwrap().plan(), plan);
        assert_eq!(started.active_plan().unwrap().action_index(), 0);
        assert!(advanced.active_action().unwrap().duration_ms() > 0);
    }

    #[test]
    fn gathering_order_execution_delivers_each_primitive_random_yield() {
        let citizen = Citizen::with_needs(-50.0, -100.0).unwrap();
        let mut planned = executing(
            &citizen,
            vec![CitizenAction::Produce(crate::production::Recipe::Forage); 4],
        );
        let mut replay = citizen.clone();
        for _ in 0..3 {
            replay = replay
                .start_action(CitizenAction::Produce(crate::production::Recipe::Forage))
                .unwrap()
                .advance(crate::ACTION_DURATION_MS)
                .unwrap();
            planned = planned.advance(crate::ACTION_DURATION_MS).unwrap();
            assert_eq!(planned.berries_units(), replay.berries_units());
            assert_eq!(planned.forage_rng, replay.forage_rng);
        }
    }

    #[test]
    fn a_long_compound_goal_finishes_before_replanning() {
        let source = Citizen::with_needs(-50.0, -100.0).unwrap();
        let actions = vec![CitizenAction::Produce(crate::production::Recipe::Forage); 6];
        let planned = executing(&source, actions.clone());
        let crossed = planned.advance(TWO_HOURS_MS).unwrap();
        assert_eq!(crossed.active_plan().unwrap().plan().actions(), actions);
        assert_eq!(crossed.active_plan().unwrap().action_index(), 4);
        let finished = crossed.advance(60 * MINUTE_MS).unwrap();
        assert_eq!(finished.active_plan().unwrap().elapsed_ms(), 0);
        assert!(finished.active_plan().unwrap().plan().decision().is_some());
    }

    #[test]
    fn single_goals_are_eligible_and_long_pairs_have_no_outer_horizon() {
        let citizen = Citizen::with_needs(-50.0, 100.0).unwrap();
        let initial = Prediction::new(&citizen, Cooldowns::default());
        let sleep = goals::variants_after(&initial, Effect::ReduceTiredness)
            .unwrap()
            .remove(0);
        let mut best = empty_plan();
        search(sleep.clone(), Effect::ReduceTiredness, &mut best).unwrap();
        assert!(best.average_wellbeing >= sleep.average().unwrap());
        assert!(best.action_durations_ms.iter().sum::<u64>() > 4 * 60 * MINUTE_MS);
        assert!((1..=2).contains(&best.goals.len()));
    }

    #[test]
    fn decision_records_existing_candidates_and_preserves_search_result() {
        use crate::marketplace::Prices;
        for prices in [Prices::new(200.0).unwrap(), Prices::new(100.0).unwrap()] {
            let citizen = Citizen::with_needs(50.0, -100.0)
                .unwrap()
                .with_prices(prices)
                .with_home_good(crate::marketplace::Good::Berries, 33)
                .unwrap();
            let snapshot = citizen.clone();
            let chosen = plan(&citizen).unwrap();
            let reference = sequential_reference(&citizen).unwrap();
            assert_eq!(chosen.actions(), reference.actions());
            assert_eq!(chosen.average_wellbeing(), reference.average_wellbeing());
            let decision = chosen.decision().unwrap();
            assert_eq!(decision.candidates.len(), 6);
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
    fn pair_evaluation_compares_all_first_goal_variants() {
        let citizen = Citizen::with_needs(0.0, -100.0)
            .unwrap()
            .with_prices(crate::marketplace::Prices::new(500.0).unwrap())
            .with_home_good(crate::marketplace::Good::Berries, 120)
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
        search(local.clone(), Effect::ReduceHunger, &mut local_plan).unwrap();
        assert!(forecast.full_plan_wellbeing >= local_plan.average_wellbeing);
        for variant in goals::variants_after(
            &Prediction::new(&citizen, Cooldowns::default()),
            Effect::ReduceHunger,
        )
        .unwrap()
        {
            let mut full = empty_plan();
            search(variant, Effect::ReduceHunger, &mut full).unwrap();
            assert!(full.average_wellbeing <= forecast.full_plan_wellbeing);
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
    fn action_starting_at_the_daily_boundary_uses_live_seller_quotes() {
        use crate::{
            AgentKind, Universe,
            marketplace::{Good, Prices, UPDATE_TIME_MS},
        };
        let citizen = Citizen::new(0.0).unwrap().with_coins(200).unwrap();
        let mut market = citizen.market().clone();
        let seller = Citizen::new(0.0).unwrap().with_map(citizen.map()).unwrap();
        market
            .list(
                seller.id(),
                citizen
                    .map()
                    .public_place(crate::locations::Location::Market),
                Good::Berries,
                1000,
            )
            .unwrap();
        let initial_quote = market.orders().next().unwrap().quoted_price;
        market.prices = market.prices.with_price(Good::Berries, 1000.0).unwrap();
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
        assert_eq!(
            boundary.market().orders().next().unwrap().quoted_price,
            (initial_quote + Prices::default().price(Good::Berries).unwrap()) / 2.0
        );
        assert_eq!(buyer.prices(), boundary.prices());
        let after = boundary.advance(crate::TRADE_DURATION_MS).unwrap();
        let AgentKind::Citizen(buyer) = &after.agents()[&id].kind;
        assert_eq!(
            buyer.coins(),
            200 - boundary
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
    fn fresh_plan_can_start_with_a_partial_meal_below_the_replan_threshold() {
        let citizen = Citizen::with_needs(100.0, -100.0)
            .unwrap()
            .with_home_good(crate::marketplace::Good::Berries, 39)
            .unwrap();
        let mut execution = executing(&citizen, vec![CitizenAction::Wait])
            .active_plan
            .unwrap();
        let partial_meal = Plan {
            actions: vec![CitizenAction::Eat],
            action_durations_ms: vec![39_000],
            commercial_actions: Vec::new(),
            goals: vec![],
            average_wellbeing: 0.0,
            decision: None,
        };
        assert!(ActivePlan::first_action_valid(&citizen, &partial_meal));
        let started = execution.adopt(citizen, partial_meal).unwrap();
        assert_eq!(
            started.active_action().unwrap().action(),
            CitizenAction::Eat
        );
    }

    #[test]
    fn replan_check_is_strictly_below_twenty_nutrition_and_only_before_later_meals() {
        for units in [0, 61, 62, 77] {
            let citizen = Citizen::with_needs(100.0, -100.0)
                .unwrap()
                .with_home_good(crate::marketplace::Good::Berries, units)
                .unwrap();
            assert!(!replan_check(&citizen, CitizenAction::Eat, 0));
            assert!(!replan_check(
                &citizen,
                CitizenAction::Produce(crate::production::Recipe::Forage),
                1
            ));
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
            assert_eq!(planned.home_units(crate::marketplace::Good::Berries), units);
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
                citizen.forage_rng = Xoshiro256PlusPlus::seed_from_u64(seed);
                let actual = citizen
                    .start_action(CitizenAction::Produce(crate::production::Recipe::Forage))
                    .unwrap()
                    .advance(30 * MINUTE_MS)
                    .unwrap();
                (actual.berries_units() < 40).then(|| (citizen.clone(), actual))
            })
            .unwrap();
        let planned = executing(
            &source,
            vec![
                CitizenAction::Produce(crate::production::Recipe::Forage),
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
                commercial_actions: Vec::new(),
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
    fn stocked_hungry_citizens_forecast_at_most_one_meal() {
        for (hunger, units) in [(50.0, 200), (150.0, 1000), (500.0, 1000), (500.0, 10_000)] {
            let citizen = Citizen::new(hunger)
                .unwrap()
                .with_home_good(crate::marketplace::Good::Berries, units)
                .unwrap();
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
