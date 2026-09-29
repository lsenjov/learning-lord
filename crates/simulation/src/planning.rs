use crate::{Citizen, CitizenAction, SimulationError};

pub const HORIZON_MS: u64 = 4 * 60 * 60 * 1000;
pub const COMMITMENT_MS: u64 = 2 * 60 * 60 * 1000;

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
                if execution.elapsed_ms >= COMMITMENT_MS {
                    execution.plan = plan(&citizen)?;
                    execution.action_index = 0;
                    execution.elapsed_ms = 0;
                }
                citizen = citizen.start_action(execution.plan.actions[execution.action_index])?;
            }
        }

        citizen.active_plan = Some(execution);
        Ok(citizen)
    }
}

pub fn plan(citizen: &Citizen) -> Result<Plan, SimulationError> {
    if citizen.active_action().is_some() || citizen.active_plan().is_some() {
        return Err(SimulationError::CitizenBusy);
    }
    let mut best = Plan {
        actions: Vec::new(),
        average_wellbeing: f64::NEG_INFINITY,
    };
    search(citizen, &mut Vec::new(), HORIZON_MS, 0.0, &mut best)?;
    Ok(best)
}

fn search(
    citizen: &Citizen,
    actions: &mut Vec<CitizenAction>,
    remaining_ms: u64,
    average_wellbeing: f64,
    best: &mut Plan,
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

    for action in [CitizenAction::Wait, CitizenAction::Eat] {
        let started = citizen.start_action(action)?;
        let duration_ms = started
            .active_action()
            .expect("starting an action creates an active action")
            .remaining_ms();
        let next = started.advance(duration_ms)?;
        let wellbeing = next.personal_wellbeing()?;
        // Incremental averaging avoids overflowing a sum and supports variable plan lengths.
        // Wellbeing is nonpositive, so the difference between two scores is finite.
        let score =
            average_wellbeing + (wellbeing - average_wellbeing) / (actions.len() + 1) as f64;
        if !score.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        actions.push(action);
        search(
            &next,
            actions,
            remaining_ms.saturating_sub(duration_ms),
            score,
            best,
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

    const MINUTE_MS: u64 = 60_000;

    #[test]
    fn search_completes_the_action_crossing_the_horizon_and_averages_all_completions() {
        let citizen = Citizen::with_hunger_rate(150.0, 0.0).unwrap();
        for (horizon_ms, expected_duration_ms, expected_score) in [
            (15 * MINUTE_MS, 30 * MINUTE_MS, -100.0),
            (75 * MINUTE_MS, 90 * MINUTE_MS, -50.0),
            (HORIZON_MS, HORIZON_MS, -18.75),
        ] {
            let mut best = Plan {
                actions: Vec::new(),
                average_wellbeing: f64::NEG_INFINITY,
            };
            search(&citizen, &mut Vec::new(), horizon_ms, 0.0, &mut best).unwrap();

            let mut predicted = citizen.clone();
            let mut elapsed_ms = 0;
            let mut sum = 0.0;
            for action in best.actions() {
                assert!(elapsed_ms < horizon_ms);
                let started = predicted.start_action(*action).unwrap();
                let duration_ms = started.active_action().unwrap().remaining_ms();
                predicted = started.advance(duration_ms).unwrap();
                elapsed_ms += duration_ms;
                sum += predicted.personal_wellbeing().unwrap();
            }
            assert_eq!(elapsed_ms, expected_duration_ms);
            assert!((best.average_wellbeing() - expected_score).abs() < 1e-10);
            assert_eq!(sum / best.actions().len() as f64, expected_score);
        }
    }

    #[test]
    fn replanning_waits_for_the_action_crossing_the_commitment_boundary() {
        let mut source = Citizen::with_hunger_rate(170.0, 0.0)
            .unwrap()
            .start_planning()
            .unwrap()
            .advance(115 * MINUTE_MS)
            .unwrap();
        // Model a delayed action without changing the game's fixed meal/wait durations.
        source.active_action.as_mut().unwrap().remaining_ms = 20 * MINUTE_MS;
        let snapshot = source.clone();
        let initial_plan = source.active_plan().unwrap().plan();

        let at_boundary = source.advance(5 * MINUTE_MS).unwrap();
        assert_eq!(
            at_boundary.active_plan().unwrap().elapsed_ms(),
            COMMITMENT_MS
        );
        assert_eq!(at_boundary.active_plan().unwrap().plan(), initial_plan);
        assert_eq!(
            at_boundary.active_action().unwrap().remaining_ms(),
            15 * MINUTE_MS
        );
        assert_eq!(at_boundary.hunger(), 20.0);

        let almost_done = source.advance(20 * MINUTE_MS - 1).unwrap();
        assert_eq!(almost_done.active_plan().unwrap().plan(), initial_plan);
        assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
        let finished = almost_done.advance(1).unwrap();
        assert_eq!(finished.hunger(), -30.0);
        assert_eq!(finished.active_plan().unwrap().action_index(), 0);
        assert_eq!(finished.active_plan().unwrap().elapsed_ms(), 0);
        assert_eq!(
            finished.active_plan().unwrap().plan().average_wellbeing(),
            0.0
        );

        let crossed = source.advance(25 * MINUTE_MS).unwrap();
        assert_eq!(crossed, finished.advance(5 * MINUTE_MS).unwrap());
        assert_eq!(crossed.active_plan().unwrap().elapsed_ms(), 5 * MINUTE_MS);
        assert_eq!(source, snapshot);
    }
}
