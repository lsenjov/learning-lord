use imbl::HashMap;
use rand::{RngExt, rngs::SmallRng};
use std::fmt;
use uuid::Uuid;

pub mod marketplace;
pub mod planning;

pub const WEALTH_WELLBEING_PER_COIN: f64 = 10.0;

pub const HUNGER_PER_HOUR: f64 = 100.0 / 24.0;
pub const TIREDNESS_PER_HOUR: f64 = 100.0 / 24.0;
pub const MEAL_NOURISHMENT: f64 = 50.0;
pub const ACTION_DURATION_MS: u64 = 30 * 60 * 1000;
pub const TRADE_DURATION_MS: u64 = 5 * 60 * 1000;
pub const SLEEP_DURATION_MS: u64 = 8 * 60 * 60 * 1000;
pub const SLEEP_RECOVERY: f64 = 100.0;
pub const BERRY_NUTRITION_PER_GRAM: f64 = 0.5;
pub const BERRY_EATING_MS_PER_GRAM: f64 = 1000.0;
pub const FORAGE_MIN_GRAMS: f64 = 5.0;
pub const FORAGE_MAX_GRAMS: f64 = 15.0;
pub const FORAGE_AVERAGE_GRAMS: f64 = (FORAGE_MIN_GRAMS + FORAGE_MAX_GRAMS) / 2.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(pub Uuid);

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub name: String,
    pub kind: AgentKind,
}

impl Agent {
    pub fn start_planning(&self) -> Result<Self, SimulationError> {
        let kind = match &self.kind {
            AgentKind::Citizen(citizen) => AgentKind::Citizen(citizen.start_planning()?),
        };
        Ok(Self {
            name: self.name.clone(),
            kind,
        })
    }

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
    Sleep,
    Forage,
    FindRocks,
    BuyBerries,
    SellPebbles,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveAction {
    action: CitizenAction,
    remaining_ms: u64,
    duration_ms: u64,
    meal_grams: f64,
    berries_after_meal: f64,
    trade_grams: f64,
    trade_coins: f64,
}

impl ActiveAction {
    pub fn action(&self) -> CitizenAction {
        self.action
    }

    pub fn remaining_ms(&self) -> u64 {
        self.remaining_ms
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Citizen {
    hunger: f64,
    hunger_per_hour: f64,
    tiredness: f64,
    berries_grams: f64,
    pebbles_grams: f64,
    coins: f64,
    forage_rng: SmallRng,
    active_action: Option<ActiveAction>,
    active_plan: Option<planning::ActivePlan>,
}

impl Citizen {
    pub fn new(hunger: f64) -> Result<Self, SimulationError> {
        Self::with_hunger_rate(hunger, HUNGER_PER_HOUR)
    }

    pub fn with_needs(hunger: f64, tiredness: f64) -> Result<Self, SimulationError> {
        if !tiredness.is_finite() {
            return Err(SimulationError::InvalidTiredness);
        }
        let mut citizen = Self::new(hunger)?;
        citizen.tiredness = tiredness.max(-100.0);
        Ok(citizen)
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
            tiredness: 0.0,
            berries_grams: 0.0,
            pebbles_grams: 0.0,
            coins: 0.0,
            forage_rng: rand::make_rng(),
            active_action: None,
            active_plan: None,
        })
    }

    pub fn hunger(&self) -> f64 {
        self.hunger
    }

    pub fn hunger_per_hour(&self) -> f64 {
        self.hunger_per_hour
    }

    pub fn tiredness(&self) -> f64 {
        self.tiredness
    }

    pub fn berries_grams(&self) -> f64 {
        self.berries_grams
    }

    pub fn with_berries(&self, grams: f64) -> Result<Self, SimulationError> {
        if !grams.is_finite() || grams < 0.0 {
            return Err(SimulationError::InvalidBerries);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.berries_grams = grams;
        Ok(citizen)
    }

    pub fn pebbles_grams(&self) -> f64 {
        self.pebbles_grams
    }

    pub fn coins(&self) -> f64 {
        self.coins
    }

    pub fn with_pebbles(&self, grams: f64) -> Result<Self, SimulationError> {
        if !grams.is_finite() || grams < 0.0 {
            return Err(SimulationError::InvalidPebbles);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.pebbles_grams = grams;
        Ok(citizen)
    }

    pub fn with_coins(&self, coins: f64) -> Result<Self, SimulationError> {
        if !coins.is_finite() {
            return Err(SimulationError::InvalidCoins);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.coins = coins;
        Ok(citizen)
    }

    pub fn wealth(&self) -> Result<f64, SimulationError> {
        use marketplace::{Good, value};
        let wealth = self.coins
            + value(Good::Berries, self.berries_grams)
            + value(Good::Pebbles, self.pebbles_grams);
        if !wealth.is_finite() {
            return Err(SimulationError::WealthOverflow);
        }
        Ok(wealth)
    }

    pub fn action_duration_ms(&self, action: CitizenAction) -> u64 {
        match action {
            CitizenAction::Eat => (self.meal_grams() * BERRY_EATING_MS_PER_GRAM).ceil() as u64,
            CitizenAction::Wait | CitizenAction::Forage | CitizenAction::FindRocks => {
                ACTION_DURATION_MS
            }
            CitizenAction::Sleep => SLEEP_DURATION_MS,
            CitizenAction::BuyBerries | CitizenAction::SellPebbles => {
                if self.trade_amounts(action).0 > 0.0 {
                    TRADE_DURATION_MS
                } else {
                    0
                }
            }
        }
    }

    fn trade_amounts(&self, action: CitizenAction) -> (f64, f64) {
        use marketplace::{Good, coins_per_kg, value};
        match action {
            CitizenAction::BuyBerries => {
                let missing =
                    (MEAL_NOURISHMENT / BERRY_NUTRITION_PER_GRAM - self.berries_grams).max(0.0);
                let cost = value(Good::Berries, missing).min(self.coins.max(0.0));
                let grams = (cost / coins_per_kg(Good::Berries) * 1000.0).min(missing);
                if cost <= 0.0 || self.berries_grams + grams == self.berries_grams {
                    (0.0, 0.0)
                } else {
                    (grams, cost)
                }
            }
            CitizenAction::SellPebbles => {
                (self.pebbles_grams, value(Good::Pebbles, self.pebbles_grams))
            }
            _ => (0.0, 0.0),
        }
    }

    fn meal_grams(&self) -> f64 {
        self.berries_grams
            .min(MEAL_NOURISHMENT / BERRY_NUTRITION_PER_GRAM)
    }

    pub fn active_action(&self) -> Option<ActiveAction> {
        self.active_action
    }

    pub fn active_plan(&self) -> Option<&planning::ActivePlan> {
        self.active_plan.as_ref()
    }

    pub fn start_planning(&self) -> Result<Self, SimulationError> {
        planning::start(self)
    }

    pub fn start_action(&self, action: CitizenAction) -> Result<Self, SimulationError> {
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        let duration_ms = self.action_duration_ms(action);
        if duration_ms == 0 {
            return Ok(citizen);
        }
        let meal_grams = if action == CitizenAction::Eat {
            self.meal_grams()
        } else {
            0.0
        };
        let (trade_grams, trade_coins) = self.trade_amounts(action);
        citizen.active_action = Some(ActiveAction {
            action,
            remaining_ms: duration_ms,
            duration_ms,
            meal_grams,
            berries_after_meal: self.berries_grams - meal_grams,
            trade_grams,
            trade_coins,
        });
        Ok(citizen)
    }

    pub fn personal_wellbeing(&self) -> Result<f64, SimulationError> {
        let wellbeing = -self.hunger.max(0.0)
            - 4.0 * (self.hunger - 100.0).max(0.0)
            - (-self.hunger - 100.0).max(0.0)
            - self.tiredness.max(0.0)
            + self.wealth()? * WEALTH_WELLBEING_PER_COIN;
        if !wellbeing.is_finite() {
            return Err(SimulationError::WellbeingOverflow);
        }
        Ok(wellbeing)
    }

    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        if let Some(execution) = &self.active_plan {
            return execution.advance(self, elapsed_ms);
        }
        self.advance_action(elapsed_ms, false)
    }

    pub(crate) fn advance_predicted(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        self.advance_action(elapsed_ms, true)
    }

    fn advance_action(&self, elapsed_ms: u64, prediction: bool) -> Result<Self, SimulationError> {
        let mut citizen = self.clone();
        if elapsed_ms == 0 {
            return Ok(citizen);
        }

        if let Some(mut active) = citizen.active_action {
            let action_elapsed_ms = elapsed_ms.min(active.remaining_ms);
            citizen.advance_needs(action_elapsed_ms, Some(active))?;
            active.remaining_ms -= action_elapsed_ms;

            if active.action == CitizenAction::Eat {
                citizen.berries_grams = active.berries_after_meal
                    + active.meal_grams * (active.remaining_ms as f64 / active.duration_ms as f64);
            }

            if active.remaining_ms == 0 {
                match active.action {
                    CitizenAction::BuyBerries => {
                        citizen.berries_grams += active.trade_grams;
                        citizen.coins -= active.trade_coins;
                    }
                    CitizenAction::SellPebbles => {
                        citizen.pebbles_grams = 0.0;
                        citizen.coins += active.trade_coins;
                        if !citizen.coins.is_finite() {
                            return Err(SimulationError::WealthOverflow);
                        }
                    }
                    _ => {}
                }
                if matches!(
                    active.action,
                    CitizenAction::Forage | CitizenAction::FindRocks
                ) {
                    let yield_grams = if prediction {
                        FORAGE_AVERAGE_GRAMS
                    } else {
                        citizen
                            .forage_rng
                            .random_range(FORAGE_MIN_GRAMS..=FORAGE_MAX_GRAMS)
                    };
                    let (stock, error) = if active.action == CitizenAction::Forage {
                        (&mut citizen.berries_grams, SimulationError::BerriesOverflow)
                    } else {
                        (&mut citizen.pebbles_grams, SimulationError::PebblesOverflow)
                    };
                    *stock += yield_grams;
                    if !stock.is_finite() {
                        return Err(error);
                    }
                }
                citizen.active_action = None;
            } else {
                citizen.active_action = Some(active);
            }
            citizen.advance_needs(elapsed_ms - action_elapsed_ms, None)?;
        } else {
            citizen.advance_needs(elapsed_ms, None)?;
        }
        Ok(citizen)
    }

    fn advance_needs(
        &mut self,
        elapsed_ms: u64,
        active: Option<ActiveAction>,
    ) -> Result<(), SimulationError> {
        let elapsed_hours = elapsed_ms as f64 / 3_600_000.0;
        let nourishment_per_hour = active.map_or(0.0, |action| {
            action.meal_grams * BERRY_NUTRITION_PER_GRAM * 3_600_000.0 / action.duration_ms as f64
        });
        let recovery_per_hour =
            if active.is_some_and(|action| action.action == CitizenAction::Sleep) {
                SLEEP_RECOVERY * 3_600_000.0 / SLEEP_DURATION_MS as f64
            } else {
                0.0
            };
        let hunger = self.hunger + (self.hunger_per_hour - nourishment_per_hour) * elapsed_hours;
        if !hunger.is_finite() {
            return Err(SimulationError::HungerOverflow);
        }
        let tiredness = self.tiredness + (TIREDNESS_PER_HOUR - recovery_per_hour) * elapsed_hours;
        if !tiredness.is_finite() {
            return Err(SimulationError::TirednessOverflow);
        }
        self.hunger = hunger;
        self.tiredness = tiredness.max(-100.0);
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

    pub fn start_planning(&self, id: AgentId) -> Result<Self, SimulationError> {
        let agent = self.agents.get(&id).ok_or(SimulationError::AgentNotFound)?;
        let updated = agent.start_planning()?;
        let mut universe = self.clone();
        universe.agents.insert(id, updated);
        Ok(universe)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationError {
    InvalidHunger,
    InvalidHungerRate,
    InvalidTiredness,
    InvalidBerries,
    InvalidPebbles,
    InvalidCoins,
    WealthOverflow,
    PebblesOverflow,
    TimeOverflow,
    HungerOverflow,
    TirednessOverflow,
    BerriesOverflow,
    WellbeingOverflow,
    CitizenBusy,
    AgentNotFound,
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidHunger => "hunger must be finite",
            Self::InvalidHungerRate => "hunger per hour must be finite and nonnegative",
            Self::InvalidTiredness => "tiredness must be finite",
            Self::InvalidBerries => "berry grams must be finite and nonnegative",
            Self::InvalidPebbles => "pebble grams must be finite and nonnegative",
            Self::InvalidCoins => "coins must be finite",
            Self::WealthOverflow => "wealth exceeds the finite range",
            Self::PebblesOverflow => "finding rocks would produce nonfinite pebble grams",
            Self::TimeOverflow => "elapsed time exceeds the simulation clock's range",
            Self::HungerOverflow => "advancing time would produce nonfinite hunger",
            Self::TirednessOverflow => "advancing time would produce nonfinite tiredness",
            Self::BerriesOverflow => "foraging would produce nonfinite berry grams",
            Self::WellbeingOverflow => "personal wellbeing exceeds the finite score range",
            Self::CitizenBusy => "citizen is already performing an action",
            Self::AgentNotFound => "agent does not exist in this universe",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SimulationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn prediction_uses_average_yield_without_advancing_the_random_stream() {
        let mut citizen = Citizen::new(0.0).unwrap();
        citizen.forage_rng = SmallRng::seed_from_u64(42);
        let started = citizen.start_action(CitizenAction::Forage).unwrap();
        let predicted = started.advance_predicted(ACTION_DURATION_MS).unwrap();
        assert_eq!(predicted.berries_grams(), 10.0);
        assert_eq!(predicted.forage_rng, citizen.forage_rng);
        let actual = started.advance(ACTION_DURATION_MS).unwrap();
        assert_ne!(actual.forage_rng, citizen.forage_rng);
        assert!((5.0..=15.0).contains(&actual.berries_grams()));

        let mut total = 0.0;
        for _ in 0..2000 {
            let next = citizen
                .start_action(CitizenAction::Forage)
                .unwrap()
                .advance(ACTION_DURATION_MS)
                .unwrap();
            let yield_grams = next.berries_grams() - citizen.berries_grams();
            assert!((5.0..=15.0).contains(&yield_grams));
            total += yield_grams;
            citizen = next;
        }
        assert!((total / 2000.0 - 10.0).abs() < 0.2);
    }
}
