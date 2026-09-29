use imbl::HashMap;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(pub Uuid);

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub name: String,
    pub kind: AgentKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentKind {
    Citizen(Citizen),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Citizen {
    hunger: f64,
    hunger_per_hour: f64,
}

impl Citizen {
    pub fn new(hunger: f64, hunger_per_hour: f64) -> Result<Self, SimulationError> {
        if !hunger.is_finite() {
            return Err(SimulationError::InvalidHunger);
        }
        if !hunger_per_hour.is_finite() || hunger_per_hour < 0.0 {
            return Err(SimulationError::InvalidHungerRate);
        }
        Ok(Self {
            hunger,
            hunger_per_hour,
        })
    }

    pub fn hunger(&self) -> f64 {
        self.hunger
    }

    pub fn hunger_per_hour(&self) -> f64 {
        self.hunger_per_hour
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

        let elapsed_hours = elapsed_ms as f64 / 3_600_000.0;
        for (_, agent) in universe.agents.iter_mut() {
            match &mut agent.kind {
                AgentKind::Citizen(citizen) => {
                    let hunger = citizen.hunger + citizen.hunger_per_hour * elapsed_hours;
                    if !hunger.is_finite() {
                        return Err(SimulationError::HungerOverflow);
                    }
                    citizen.hunger = hunger;
                }
            }
        }
        universe.current_time_ms = current_time_ms;
        Ok(universe)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationError {
    InvalidHunger,
    InvalidHungerRate,
    TimeOverflow,
    HungerOverflow,
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidHunger => "hunger must be finite",
            Self::InvalidHungerRate => "hunger per hour must be finite and nonnegative",
            Self::TimeOverflow => "elapsed time exceeds the simulation clock's range",
            Self::HungerOverflow => "advancing time would produce nonfinite hunger",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SimulationError {}
