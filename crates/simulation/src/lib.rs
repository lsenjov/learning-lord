use imbl::HashMap;
use std::fmt;
use uuid::Uuid;

pub const HUNGER_PER_HOUR: f64 = 100.0 / 24.0;
pub const MEAL_NOURISHMENT: f64 = 50.0;
pub const ACTION_DURATION_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(pub Uuid);

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub name: String,
    pub kind: AgentKind,
}

impl Agent {
    pub fn start_action(&self, action: CitizenAction) -> Result<Self, SimulationError> {
        let kind = match &self.kind {
            AgentKind::Citizen(citizen) => AgentKind::Citizen(citizen.start_action(action)?),
        };
        Ok(Self {
            name: self.name.clone(),
            kind,
        })
    }

    pub fn personal_wellbeing(&self) -> Result<f64, SimulationError> {
        match &self.kind {
            AgentKind::Citizen(citizen) => citizen.personal_wellbeing(),
        }
    }

    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        let kind = match &self.kind {
            AgentKind::Citizen(citizen) => AgentKind::Citizen(citizen.advance(elapsed_ms)?),
        };
        Ok(Self {
            name: self.name.clone(),
            kind,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentKind {
    Citizen(Citizen),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CitizenAction {
    Eat,
    Wait,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveAction {
    action: CitizenAction,
    remaining_ms: u64,
}

impl ActiveAction {
    pub fn action(&self) -> CitizenAction {
        self.action
    }

    pub fn remaining_ms(&self) -> u64 {
        self.remaining_ms
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Citizen {
    hunger: f64,
    hunger_per_hour: f64,
    active_action: Option<ActiveAction>,
}

impl Citizen {
    pub fn new(hunger: f64) -> Result<Self, SimulationError> {
        Self::with_hunger_rate(hunger, HUNGER_PER_HOUR)
    }

    pub fn with_hunger_rate(hunger: f64, hunger_per_hour: f64) -> Result<Self, SimulationError> {
        if !hunger.is_finite() {
            return Err(SimulationError::InvalidHunger);
        }
        if !hunger_per_hour.is_finite() || hunger_per_hour < 0.0 {
            return Err(SimulationError::InvalidHungerRate);
        }
        Ok(Self {
            hunger,
            hunger_per_hour,
            active_action: None,
        })
    }

    pub fn hunger(&self) -> f64 {
        self.hunger
    }

    pub fn hunger_per_hour(&self) -> f64 {
        self.hunger_per_hour
    }

    pub fn active_action(&self) -> Option<ActiveAction> {
        self.active_action
    }

    pub fn start_action(&self, action: CitizenAction) -> Result<Self, SimulationError> {
        if self.active_action.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.active_action = Some(ActiveAction {
            action,
            remaining_ms: ACTION_DURATION_MS,
        });
        Ok(citizen)
    }

    pub fn personal_wellbeing(&self) -> Result<f64, SimulationError> {
        let wellbeing = -self.hunger.max(0.0)
            - 4.0 * (self.hunger - 100.0).max(0.0)
            - (-self.hunger - 100.0).max(0.0);
        if !wellbeing.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        Ok(wellbeing)
    }

    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        let mut citizen = self.clone();
        if elapsed_ms == 0 {
            return Ok(citizen);
        }

        if let Some(mut active) = citizen.active_action {
            let action_elapsed_ms = elapsed_ms.min(active.remaining_ms);
            citizen.advance_hunger(action_elapsed_ms)?;
            active.remaining_ms -= action_elapsed_ms;

            if active.remaining_ms == 0 {
                if active.action == CitizenAction::Eat {
                    citizen.hunger -= MEAL_NOURISHMENT;
                }
                citizen.active_action = None;
            } else {
                citizen.active_action = Some(active);
            }
            citizen.advance_hunger(elapsed_ms - action_elapsed_ms)?;
        } else {
            citizen.advance_hunger(elapsed_ms)?;
        }
        Ok(citizen)
    }

    fn advance_hunger(&mut self, elapsed_ms: u64) -> Result<(), SimulationError> {
        let elapsed_hours = elapsed_ms as f64 / 3_600_000.0;
        let hunger = self.hunger + self.hunger_per_hour * elapsed_hours;
        if !hunger.is_finite() {
            return Err(SimulationError::HungerOverflow);
        }
        self.hunger = hunger;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Universe {
    current_time_ms: u64,
    agents: HashMap<AgentId, Agent>,
}

impl Universe {
    pub fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }

    pub fn agents(&self) -> &HashMap<AgentId, Agent> {
        &self.agents
    }

    pub fn with_citizen(&self, name: impl Into<String>, citizen: Citizen) -> (Self, AgentId) {
        let mut id = AgentId(Uuid::new_v4());
        while self.agents.contains_key(&id) {
            id = AgentId(Uuid::new_v4());
        }

        let mut universe = self.clone();
        universe.agents.insert(
            id,
            Agent {
                name: name.into(),
                kind: AgentKind::Citizen(citizen),
            },
        );
        (universe, id)
    }

    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        let current_time_ms = self
            .current_time_ms
            .checked_add(elapsed_ms)
            .ok_or(SimulationError::TimeOverflow)?;
        let mut universe = self.clone();
        if elapsed_ms == 0 {
            return Ok(universe);
        }

        for (_, agent) in universe.agents.iter_mut() {
            *agent = agent.advance(elapsed_ms)?;
        }
        universe.current_time_ms = current_time_ms;
        Ok(universe)
    }

    pub fn start_action(
        &self,
        id: AgentId,
        action: CitizenAction,
    ) -> Result<Self, SimulationError> {
        let agent = self.agents.get(&id).ok_or(SimulationError::AgentNotFound)?;
        let updated = agent.start_action(action)?;
        let mut universe = self.clone();
        universe.agents.insert(id, updated);
        Ok(universe)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationError {
    InvalidHunger,
    InvalidHungerRate,
    TimeOverflow,
    HungerOverflow,
    WellbeingOverflow,
    CitizenBusy,
    AgentNotFound,
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidHunger => "hunger must be finite",
            Self::InvalidHungerRate => "hunger per hour must be finite and nonnegative",
            Self::TimeOverflow => "elapsed time exceeds the simulation clock's range",
            Self::HungerOverflow => "advancing time would produce nonfinite hunger",
            Self::WellbeingOverflow => "personal wellbeing exceeds the finite score range",
            Self::CitizenBusy => "citizen is already performing an action",
            Self::AgentNotFound => "agent does not exist in this universe",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SimulationError {}
