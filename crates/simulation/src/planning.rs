use crate::{ACTION_DURATION_MS, Citizen, CitizenAction, SimulationError};

pub const HORIZON_ACTIONS: usize = 8;
pub const COMMITTED_ACTIONS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    actions: [CitizenAction; HORIZON_ACTIONS],
    average_wellbeing: f64,
}

impl Plan {
    pub fn actions(&self) -> &[CitizenAction; HORIZON_ACTIONS] {
        &self.actions
    }

    pub fn average_wellbeing(&self) -> f64 {
        self.average_wellbeing
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanExecution {
    plan: Plan,
    action_index: usize,
}

impl PlanExecution {
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Zero-based index within the committed four-action batch.
    pub fn action_index(&self) -> usize {
        self.action_index
    }

    pub(crate) fn advance(
        &self,
        source: &Citizen,
        mut elapsed_ms: u64,
    ) -> Result<Citizen, SimulationError> {
        let mut citizen = source.clone();
        citizen.plan_execution = None;
        let mut execution = self.clone();

        while elapsed_ms > 0 {
            let remaining_ms = citizen
                .active_action()
                .expect("a planned citizen has an active action")
                .remaining_ms();
            let step_ms = elapsed_ms.min(remaining_ms);
            citizen = citizen.advance(step_ms)?;
            elapsed_ms -= step_ms;

            if citizen.active_action().is_none() {
                execution.action_index += 1;
                if execution.action_index == COMMITTED_ACTIONS {
                    execution.plan = plan(&citizen)?;
                    execution.action_index = 0;
                }
                citizen = citizen.start_action(execution.plan.actions[execution.action_index])?;
            }
        }

        citizen.plan_execution = Some(execution);
        Ok(citizen)
    }
}

pub fn plan(citizen: &Citizen) -> Result<Plan, SimulationError> {
    if citizen.active_action().is_some() || citizen.plan_execution().is_some() {
        return Err(SimulationError::CitizenBusy);
    }
    let mut best = Plan {
        actions: [CitizenAction::Wait; HORIZON_ACTIONS],
        average_wellbeing: f64::NEG_INFINITY,
    };
    search(
        citizen,
        [CitizenAction::Wait; HORIZON_ACTIONS],
        0,
        0.0,
        &mut best,
    )?;
    Ok(best)
}

fn search(
    citizen: &Citizen,
    mut actions: [CitizenAction; HORIZON_ACTIONS],
    depth: usize,
    average_wellbeing: f64,
    best: &mut Plan,
) -> Result<(), SimulationError> {
    if depth == HORIZON_ACTIONS {
        if average_wellbeing > best.average_wellbeing {
            *best = Plan {
                actions,
                average_wellbeing,
            };
        }
        return Ok(());
    }

    for action in [CitizenAction::Wait, CitizenAction::Eat] {
        let next = citizen.start_action(action)?.advance(ACTION_DURATION_MS)?;
        // Divide before accumulating so finite scores do not overflow their sum.
        let score = average_wellbeing + next.personal_wellbeing()? / HORIZON_ACTIONS as f64;
        if !score.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        actions[depth] = action;
        search(&next, actions, depth + 1, score, best)?;
    }
    Ok(())
}

pub(crate) fn start(citizen: &Citizen) -> Result<Citizen, SimulationError> {
    let plan = plan(citizen)?;
    let mut citizen = citizen.start_action(plan.actions[0])?;
    citizen.plan_execution = Some(PlanExecution {
        plan,
        action_index: 0,
    });
    Ok(citizen)
}
