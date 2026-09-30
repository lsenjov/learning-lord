use crate::{BERRY_NUTRITION_PER_GRAM, Citizen, CitizenAction, SimulationError};

pub const HORIZON_MS: u64 = 4 * 60 * 60 * 1000;
pub const COMMITMENT_MS: u64 = 2 * 60 * 60 * 1000;
pub const REPLAN_MIN_NUTRITION: f64 = 20.0;
pub const EAT_PLAN_COOLDOWN_MS: u64 = 4 * 60 * 60 * 1000;

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    actions: Vec<CitizenAction>,
    average_wellbeing: f64,
}

impl Plan {
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

    pub(crate) fn advance(
        &self,
        source: &Citizen,
        mut elapsed_ms: u64,
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

            if citizen.active_action().is_none() {
                execution.action_index += 1;
                citizen = execution.start_next_action(citizen)?;
            }
        }

        citizen.active_plan = Some(execution);
        Ok(citizen)
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
    action == CitizenAction::Eat
        && action_index > 0
        && citizen.berries_grams() * BERRY_NUTRITION_PER_GRAM < REPLAN_MIN_NUTRITION
}

pub fn plan(citizen: &Citizen) -> Result<Plan, SimulationError> {
    if citizen.active_action().is_some() || citizen.active_plan().is_some() {
        return Err(SimulationError::CitizenBusy);
    }
    let mut best = Plan {
        actions: Vec::new(),
        average_wellbeing: f64::NEG_INFINITY,
    };
    search(citizen, &mut Vec::new(), HORIZON_MS, 0.0, &mut best, 0)?;
    Ok(best)
}

fn search(
    citizen: &Citizen,
    actions: &mut Vec<CitizenAction>,
    remaining_ms: u64,
    average_wellbeing: f64,
    best: &mut Plan,
    eat_cooldown_ms: u64,
) -> Result<(), SimulationError> {
    if remaining_ms == 0 {
        if average_wellbeing > best.average_wellbeing {
            *best = Plan {
                actions: actions.clone(),
                average_wellbeing,
            };
        }
        return Ok(());
    }

    for action in [
        CitizenAction::Wait,
        CitizenAction::Eat,
        CitizenAction::Sleep,
        CitizenAction::Forage,
    ] {
        if action == CitizenAction::Eat && eat_cooldown_ms > 0
            || replan_check(citizen, action, actions.len())
        {
            continue;
        }
        let started = citizen.start_action(action)?;
        let Some(active) = started.active_action() else {
            continue;
        };
        let duration_ms = active.remaining_ms();
        let next = started.advance_predicted(duration_ms)?;
        let wellbeing = next.personal_wellbeing()?;
        // Incremental averaging avoids overflowing a sum and supports variable plan lengths.
        // Wellbeing is nonpositive, so the difference between two scores is finite.
        let score =
            average_wellbeing + (wellbeing - average_wellbeing) / (actions.len() + 1) as f64;
        if !score.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        let next_eat_cooldown_ms = if action == CitizenAction::Eat {
            EAT_PLAN_COOLDOWN_MS
        } else {
            eat_cooldown_ms.saturating_sub(duration_ms)
        };
        actions.push(action);
        search(
            &next,
            actions,
            remaining_ms.saturating_sub(duration_ms),
            score,
            best,
            next_eat_cooldown_ms,
        )?;
        actions.pop();
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
                actions,
                average_wellbeing: -1.0,
            },
            action_index: 0,
            elapsed_ms: 0,
        });
        citizen
    }

    #[test]
    fn replan_check_is_strictly_below_twenty_nutrition_and_only_before_later_meals() {
        for grams in [0.0, 39.999, 40.0, 50.0] {
            let citizen = Citizen::with_needs(100.0, -100.0)
                .unwrap()
                .with_berries(grams)
                .unwrap();
            assert!(!replan_check(&citizen, CitizenAction::Eat, 0));
            assert!(!replan_check(&citizen, CitizenAction::Forage, 1));
            assert_eq!(replan_check(&citizen, CitizenAction::Eat, 1), grams < 40.0);
            let planned = executing(
                &citizen,
                vec![
                    CitizenAction::Wait,
                    CitizenAction::Eat,
                    CitizenAction::Sleep,
                ],
            );
            let next = planned.advance(30 * MINUTE_MS).unwrap();
            if grams < 40.0 {
                assert_eq!(next.active_plan().unwrap().action_index(), 0);
                assert_eq!(next.active_plan().unwrap().elapsed_ms(), 0);
            } else {
                assert_eq!(
                    next.active_plan().unwrap().plan(),
                    planned.active_plan().unwrap().plan()
                );
                assert_eq!(next.active_plan().unwrap().action_index(), 1);
                assert_eq!(next.active_plan().unwrap().elapsed_ms(), 30 * MINUTE_MS);
                assert_eq!(
                    next.active_action().unwrap().remaining_ms(),
                    (grams * 1000.0) as u64
                );
                let partial = next.advance(20_000).unwrap();
                assert!(partial.berries_grams() < 40.0);
                assert_eq!(partial.active_plan().unwrap().action_index(), 1);
                assert_eq!(
                    partial.active_action().unwrap().action(),
                    CitizenAction::Eat
                );
            }
            assert_eq!(planned.berries_grams(), grams);
        }
    }

    #[test]
    fn a_low_actual_forage_yield_replans_from_actual_inventory() {
        let mut citizen = Citizen::with_needs(100.0, -100.0)
            .unwrap()
            .with_berries(30.0)
            .unwrap();
        let (source, actual) = (0..100)
            .find_map(|seed| {
                citizen.forage_rng = SmallRng::seed_from_u64(seed);
                let actual = citizen
                    .start_action(CitizenAction::Forage)
                    .unwrap()
                    .advance(30 * MINUTE_MS)
                    .unwrap();
                (actual.berries_grams() < 40.0).then(|| (citizen.clone(), actual))
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
        assert_eq!(result.berries_grams(), actual.berries_grams());
        assert_eq!(result.forage_rng, actual.forage_rng);
        assert_eq!(source.berries_grams(), 30.0);
    }

    #[test]
    fn an_empty_first_meal_is_skipped_and_an_exhausted_plan_is_replaced() {
        let citizen = Citizen::new(0.0).unwrap();
        let mut active = ActivePlan {
            plan: Plan {
                actions: vec![CitizenAction::Eat, CitizenAction::Wait],
                average_wellbeing: 0.0,
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
        for (hunger, grams) in [
            (50.0, 200.0),
            (150.0, 1000.0),
            (500.0, 1000.0),
            (500.0, 10_000.0),
        ] {
            let citizen = Citizen::new(hunger).unwrap().with_berries(grams).unwrap();
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

    #[test]
    fn longer_predictions_allow_another_meal_four_hours_after_completion() {
        let citizen = Citizen::with_needs(200.0, -100.0)
            .unwrap()
            .with_berries(200.0)
            .unwrap();
        let mut best = Plan {
            actions: Vec::new(),
            average_wellbeing: f64::NEG_INFINITY,
        };
        search(
            &citizen,
            &mut Vec::new(),
            EAT_PLAN_COOLDOWN_MS + 200_000,
            0.0,
            &mut best,
            0,
        )
        .unwrap();
        let mut predicted = citizen;
        let mut elapsed_ms = 0;
        let mut last_meal_completion = None;
        let mut meals = 0;
        for &action in best.actions() {
            let duration_ms = predicted.action_duration_ms(action);
            if action == CitizenAction::Eat {
                if let Some(completed_ms) = last_meal_completion {
                    assert!(elapsed_ms - completed_ms >= EAT_PLAN_COOLDOWN_MS);
                }
                meals += 1;
                last_meal_completion = Some(elapsed_ms + duration_ms);
            }
            predicted = predicted
                .start_action(action)
                .unwrap()
                .advance_predicted(duration_ms)
                .unwrap();
            elapsed_ms += duration_ms;
        }
        assert_eq!(meals, 2);
    }

    #[test]
    fn meal_eligibility_opens_at_the_cooldown_boundary_and_resets_for_a_new_plan() {
        let citizen = Citizen::with_needs(200.0, -100.0)
            .unwrap()
            .with_berries(200.0)
            .unwrap();
        for cooldown_ms in [0, 1] {
            let mut best = Plan {
                actions: Vec::new(),
                average_wellbeing: f64::NEG_INFINITY,
            };
            search(&citizen, &mut Vec::new(), 1, 0.0, &mut best, cooldown_ms).unwrap();
            assert_eq!(best.actions()[0] == CitizenAction::Eat, cooldown_ms == 0);
        }
        let just_eaten = citizen
            .start_action(CitizenAction::Eat)
            .unwrap()
            .advance(100_000)
            .unwrap();
        let replanned = just_eaten.start_planning().unwrap();
        assert_eq!(
            replanned.active_action().unwrap().action(),
            CitizenAction::Eat
        );
        let small_meal = Citizen::with_needs(200.0, -100.0)
            .unwrap()
            .with_berries(10.0)
            .unwrap();
        let mut best = Plan {
            actions: Vec::new(),
            average_wellbeing: f64::NEG_INFINITY,
        };
        search(&small_meal, &mut Vec::new(), 1, 0.0, &mut best, 0).unwrap();
        assert_eq!(best.actions(), &[CitizenAction::Eat]);
    }

    #[test]
    fn search_completes_the_action_crossing_the_horizon_and_averages_all_completions() {
        let citizen = Citizen::with_hunger_rate(150.0, 0.0).unwrap();
        for horizon_ms in [15 * MINUTE_MS, 75 * MINUTE_MS, HORIZON_MS] {
            let mut best = Plan {
                actions: Vec::new(),
                average_wellbeing: f64::NEG_INFINITY,
            };
            search(&citizen, &mut Vec::new(), horizon_ms, 0.0, &mut best, 0).unwrap();

            let mut predicted = citizen.clone();
            let mut elapsed_ms = 0;
            let mut sum = 0.0;
            for action in best.actions() {
                assert!(elapsed_ms < horizon_ms);
                let started = predicted.start_action(*action).unwrap();
                let duration_ms = started.active_action().unwrap().remaining_ms();
                predicted = started.advance_predicted(duration_ms).unwrap();
                elapsed_ms += duration_ms;
                sum += predicted.personal_wellbeing().unwrap();
            }
            assert!(elapsed_ms >= horizon_ms);
            assert!((sum / best.actions().len() as f64 - best.average_wellbeing()).abs() < 1e-10);
        }
    }
}
