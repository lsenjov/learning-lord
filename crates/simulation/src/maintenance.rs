use crate::{AgentId, planning::goals::Effect};

pub const LISTING_INTERVAL_MS: u64 = 12 * 60 * 60 * 1000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaintenanceTask {
    pub claimant: Option<AgentId>,
    pub completed_at_ms: Option<u64>,
    pub performed_work: bool,
}

impl MaintenanceTask {
    pub fn remaining_ms(&self, goal: Effect, now_ms: u64) -> u64 {
        if goal != Effect::ListExcess {
            return 0;
        }
        self.completed_at_ms.map_or(0, |completed| {
            completed
                .saturating_add(LISTING_INTERVAL_MS)
                .saturating_sub(now_ms)
        })
    }

    pub fn available(&self, goal: Effect, now_ms: u64) -> bool {
        self.claimant.is_none() && self.remaining_ms(goal, now_ms) == 0
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaintenanceState {
    pub listing: MaintenanceTask,
    pub replenishment: MaintenanceTask,
}

impl MaintenanceState {
    pub fn task(&self, goal: Effect) -> Option<&MaintenanceTask> {
        match goal {
            Effect::ListExcess => Some(&self.listing),
            Effect::ReplenishReserves => Some(&self.replenishment),
            _ => None,
        }
    }

    fn task_mut(&mut self, goal: Effect) -> Option<&mut MaintenanceTask> {
        match goal {
            Effect::ListExcess => Some(&mut self.listing),
            Effect::ReplenishReserves => Some(&mut self.replenishment),
            _ => None,
        }
    }

    pub fn available(&self, goal: Effect, now_ms: u64) -> bool {
        self.task(goal).is_none_or(|task| {
            task.claimant.is_none()
                && (goal == Effect::ReplenishReserves || task.remaining_ms(goal, now_ms) == 0)
        })
    }

    pub fn claim(&mut self, goal: Effect, worker: AgentId, now_ms: u64) -> bool {
        if !self.available(goal, now_ms) {
            return false;
        }
        let Some(task) = self.task_mut(goal) else {
            return true;
        };
        if task.claimant.is_some() {
            return false;
        }
        task.claimant = Some(worker);
        task.performed_work = false;
        true
    }

    pub(crate) fn claim_urgent(
        &mut self,
        goal: Effect,
        worker: AgentId,
        now_ms: u64,
        urgent: bool,
    ) -> bool {
        if urgent && goal == Effect::ReplenishReserves {
            if self.replenishment.claimant.is_some() {
                return false;
            }
            self.replenishment.claimant = Some(worker);
            self.replenishment.performed_work = false;
            return true;
        }
        self.claim(goal, worker, now_ms)
    }
    pub fn finish(&mut self, goal: Effect, worker: AgentId, now_ms: u64, successful: bool) {
        let Some(task) = self.task_mut(goal) else {
            return;
        };
        if task.claimant != Some(worker) {
            return;
        }
        task.claimant = None;
        if successful && task.performed_work {
            task.completed_at_ms = Some(now_ms);
        }
        task.performed_work = false;
    }

    pub fn record_work(&mut self, goal: Effect, worker: AgentId) {
        if let Some(task) = self.task_mut(goal)
            && task.claimant == Some(worker)
        {
            task.performed_work = true;
        }
    }

    pub fn cancel(&mut self, worker: AgentId) {
        for task in [&mut self.listing, &mut self.replenishment] {
            if task.claimant == Some(worker) {
                task.claimant = None;
                task.performed_work = false;
            }
        }
    }
}

impl crate::Citizen {
    pub(crate) fn urgent_provisioning(&self) -> bool {
        let members = self.supplied_members() as f64;
        self.home_food_nutrition() < 100.0 * members
            || self.home_drink_units() < 400 * self.supplied_members()
    }
    pub(crate) fn maintenance_available(&self, goal: Effect) -> bool {
        self.maintenance.available(goal, self.current_time_ms())
            || goal == Effect::ReplenishReserves
                && self.urgent_provisioning()
                && self.maintenance.replenishment.claimant.is_none()
    }
    pub(crate) fn claim_personal_maintenance(&mut self, goal: Effect) -> bool {
        let urgent = self.urgent_provisioning();
        self.maintenance
            .claim_urgent(goal, self.id, self.current_time_ms(), urgent)
    }
    pub(crate) fn supplied_members(&self) -> u64 {
        self.household
            .as_ref()
            .map_or(1, |household| household.members.len() as u64)
    }
}

impl crate::Universe {
    pub(crate) fn claim_maintenance(
        &mut self,
        worker: AgentId,
    ) -> Result<(), crate::SimulationError> {
        let crate::AgentKind::Citizen(citizen) = &self
            .agents
            .get(&worker)
            .ok_or(crate::SimulationError::AgentNotFound)?
            .kind;
        let Some(goal) = citizen
            .active_plan()
            .and_then(|plan| plan.maintenance_goal())
        else {
            return Ok(());
        };
        let owner = citizen.economic_id();
        let now = citizen.current_time_ms();
        let urgent = citizen.urgent_provisioning();
        let state = if let Some(household) = self.households.get_mut(&owner) {
            &mut household.maintenance
        } else {
            let crate::AgentKind::Citizen(citizen) =
                &mut self.agents.get_mut(&worker).unwrap().kind;
            &mut citizen.maintenance
        };
        if state
            .task(goal)
            .is_some_and(|task| task.claimant == Some(worker))
        {
            return Ok(());
        }
        if state.claim_urgent(goal, worker, now, urgent) {
            Ok(())
        } else {
            Err(crate::SimulationError::MissingInputs)
        }
    }

    pub(crate) fn finish_maintenance(
        &mut self,
        worker: AgentId,
        goal: Effect,
        now: u64,
        successful: bool,
    ) -> Result<(), crate::SimulationError> {
        let owner = self.economic_id(worker)?;
        let state = if let Some(household) = self.households.get_mut(&owner) {
            &mut household.maintenance
        } else {
            let crate::AgentKind::Citizen(citizen) = &mut self
                .agents
                .get_mut(&worker)
                .ok_or(crate::SimulationError::AgentNotFound)?
                .kind;
            &mut citizen.maintenance
        };
        state.finish(goal, worker, now, successful);
        Ok(())
    }

    pub(crate) fn record_maintenance_work(
        &mut self,
        worker: AgentId,
        goal: Effect,
    ) -> Result<(), crate::SimulationError> {
        let owner = self.economic_id(worker)?;
        let state = if let Some(household) = self.households.get_mut(&owner) {
            &mut household.maintenance
        } else {
            let crate::AgentKind::Citizen(citizen) = &mut self
                .agents
                .get_mut(&worker)
                .ok_or(crate::SimulationError::AgentNotFound)?
                .kind;
            &mut citizen.maintenance
        };
        state.record_work(goal, worker);
        Ok(())
    }

    pub(crate) fn cancel_maintenance(
        &mut self,
        worker: AgentId,
    ) -> Result<(), crate::SimulationError> {
        let owner = self.economic_id(worker)?;
        let state = if let Some(household) = self.households.get_mut(&owner) {
            &mut household.maintenance
        } else {
            let crate::AgentKind::Citizen(citizen) = &mut self
                .agents
                .get_mut(&worker)
                .ok_or(crate::SimulationError::AgentNotFound)?
                .kind;
            &mut citizen.maintenance
        };
        state.cancel(worker);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_waits_twelve_hours_but_replenishment_has_no_timer() {
        let worker = AgentId(uuid::Uuid::new_v4());
        let mut state = MaintenanceState::default();
        for goal in [Effect::ListExcess, Effect::ReplenishReserves] {
            assert!(state.claim(goal, worker, 0));
            state.record_work(goal, worker);
            state.finish(goal, worker, 100, true);
        }
        assert!(state.available(Effect::ReplenishReserves, 100));
        assert!(!state.available(Effect::ListExcess, 100 + 12 * 60 * 60 * 1000 - 1));
        assert!(state.available(Effect::ListExcess, 100 + 12 * 60 * 60 * 1000));
    }

    #[test]
    fn committed_claims_are_shared_and_cooldowns_are_independent() {
        let first = AgentId(uuid::Uuid::new_v4());
        let second = AgentId(uuid::Uuid::new_v4());
        let mut state = MaintenanceState::default();
        assert!(state.claim(Effect::ListExcess, first, 100));
        assert!(!state.claim(Effect::ListExcess, second, 100));
        assert!(state.claim(Effect::ReplenishReserves, second, 100));
        state.record_work(Effect::ListExcess, first);
        state.finish(Effect::ListExcess, first, 500, true);
        assert!(!state.available(Effect::ListExcess, 500 + LISTING_INTERVAL_MS - 1));
        assert!(state.available(Effect::ListExcess, 500 + LISTING_INTERVAL_MS));
        assert_eq!(state.replenishment.claimant, Some(second));
    }

    #[test]
    fn cancellation_and_noops_do_not_start_cooldowns() {
        let worker = AgentId(uuid::Uuid::new_v4());
        let mut state = MaintenanceState::default();
        assert!(state.claim(Effect::ReplenishReserves, worker, 0));
        state.finish(Effect::ReplenishReserves, worker, 100, false);
        assert!(state.available(Effect::ReplenishReserves, 100));
        assert!(state.claim(Effect::ReplenishReserves, worker, 100));
        state.cancel(worker);
        assert!(state.available(Effect::ReplenishReserves, 100));
        assert_eq!(state.replenishment.completed_at_ms, None);
    }
}
