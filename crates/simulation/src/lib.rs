use imbl::HashMap;
use marketplace::{Good, Market, Prices};
use rand::{RngExt, rngs::SmallRng};
use std::fmt;
use uuid::Uuid;

pub mod locations;
pub mod marketplace;
use locations::{Location, Map, PlaceId, Position, WALK_MS_PER_METRE};
pub mod planning;
pub mod production;

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

    /// Advances this isolated snapshot; use Universe for shared trade settlement.
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CitizenAction {
    Eat,
    Wait,
    Sleep,
    Forage,
    BuyFood(Good),
    Produce(production::Recipe),
    List(Good, f64),
    Buy(marketplace::ShoppingList),
    Withdraw(Good, f64),
    Travel(PlaceId),
}

impl CitizenAction {
    pub fn required_place(self, citizen: &Citizen) -> Option<PlaceId> {
        match self {
            Self::Sleep => Some(citizen.home),
            Self::Forage => Some(citizen.map.public_place(Location::Forest)),
            Self::Produce(recipe) => citizen.production_place(recipe).ok(),
            Self::BuyFood(_) | Self::List(..) | Self::Buy(..) | Self::Withdraw(..) => {
                Some(citizen.map.public_place(Location::Market))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveAction {
    action: CitizenAction,
    remaining_ms: u64,
    duration_ms: u64,
    meal_nutrition: f64,
    meal_grams: [f64; Good::COUNT],
    inventory_after_meal: [f64; Good::COUNT],
    travel_origin: Position,
    travel_destination: Option<Position>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartingRole {
    Farmer,
    Miller,
    Woodcutter,
    Baker,
}

impl StartingRole {
    pub fn name(self) -> &'static str {
        match self {
            Self::Farmer => "Farmer",
            Self::Miller => "Miller",
            Self::Woodcutter => "Woodcutter",
            Self::Baker => "Baker",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Citizen {
    hunger: f64,
    hunger_per_hour: f64,
    tiredness: f64,
    inventory: HashMap<Good, f64>,
    id: AgentId,
    home: PlaceId,
    starting_role: Option<StartingRole>,
    skills: [f64; production::Skill::COUNT],
    coins: f64,
    forage_rng: SmallRng,
    prices: Prices,
    market: Market,
    market_time_ms: u64,
    map: Map,
    position: Position,
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
        let id = AgentId(Uuid::new_v4());
        let (map, home) =
            Map::default().with_place(Location::Home, Position::default(), Some(id), "Home")?;
        Ok(Self {
            hunger,
            hunger_per_hour,
            tiredness: 0.0,
            inventory: Good::ALL.into_iter().map(|good| (good, 0.0)).collect(),
            id,
            home,
            starting_role: None,
            skills: [0.0; production::Skill::COUNT],
            coins: 0.0,
            forage_rng: rand::make_rng(),
            prices: Prices::default(),
            market: Market::default(),
            market_time_ms: 0,
            map,
            position: Position::default(),
            active_action: None,
            active_plan: None,
        })
    }

    pub fn position(&self) -> Position {
        self.position
    }
    pub fn map(&self) -> Map {
        self.map.clone()
    }

    pub fn with_map(&self, map: Map) -> Result<Self, SimulationError> {
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        if !map.places().contains_key(&self.home) {
            let (map, home) =
                map.with_place(Location::Home, Position::default(), Some(self.id), "Home")?;
            citizen.map = map;
            citizen.home = home;
        } else {
            citizen.map = map;
        }
        Ok(citizen)
    }

    pub fn with_position(&self, position: Position) -> Result<Self, SimulationError> {
        if !position.valid() {
            return Err(SimulationError::InvalidPosition);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.position = position;
        Ok(citizen)
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

    pub fn starting_role(&self) -> Option<StartingRole> {
        self.starting_role
    }
    pub fn with_starting_role(&self, role: StartingRole) -> Self {
        let mut citizen = self.clone();
        citizen.starting_role = Some(role);
        let skill = match role {
            StartingRole::Farmer => production::Skill::Farming,
            StartingRole::Miller => production::Skill::Milling,
            StartingRole::Woodcutter => production::Skill::Woodcutting,
            StartingRole::Baker => production::Skill::Baking,
        };
        citizen.skills[skill as usize] = 1.0;
        citizen
    }

    pub fn id(&self) -> AgentId {
        self.id
    }
    pub fn home(&self) -> PlaceId {
        self.home
    }
    pub fn owned_properties(&self) -> impl Iterator<Item = &locations::Place> {
        self.map
            .places()
            .values()
            .filter(|place| place.owner == Some(self.id))
    }
    pub fn grams(&self, good: Good) -> f64 {
        self.inventory.get(&good).copied().unwrap_or(0.0)
    }
    pub fn with_good(&self, good: Good, grams: f64) -> Result<Self, SimulationError> {
        if !grams.is_finite() || grams < 0.0 {
            return Err(SimulationError::InvalidInventory);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.inventory.insert(good, grams);
        Ok(citizen)
    }
    pub fn berries_grams(&self) -> f64 {
        self.grams(Good::Berries)
    }
    pub fn with_berries(&self, grams: f64) -> Result<Self, SimulationError> {
        self.with_good(Good::Berries, grams).map_err(|error| {
            if error == SimulationError::InvalidInventory {
                SimulationError::InvalidBerries
            } else {
                error
            }
        })
    }
    pub fn coins(&self) -> f64 {
        self.coins
    }

    pub fn market(&self) -> &Market {
        &self.market
    }

    pub fn with_market(&self, market: Market) -> Self {
        let mut citizen = self.clone();
        citizen.prices = market.prices;
        citizen.market = market;
        citizen
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

    pub fn prices(&self) -> Prices {
        self.prices
    }

    pub fn with_prices(&self, prices: Prices) -> Self {
        let mut citizen = self.clone();
        citizen.prices = prices;
        citizen.market.prices = prices;
        citizen
    }

    pub fn wealth(&self) -> Result<f64, SimulationError> {
        let wealth = self.coins
            + Good::ALL
                .into_iter()
                .map(|good| {
                    self.prices
                        .value(
                            good,
                            self.grams(good) + self.market.listed_grams(self.id, good),
                        )
                        .unwrap_or(0.0)
                })
                .sum::<f64>();
        if !wealth.is_finite() {
            return Err(SimulationError::WealthOverflow);
        }
        Ok(wealth)
    }

    fn accessible_place(&self, id: PlaceId) -> Result<&locations::Place, SimulationError> {
        let place = self.map.place(id)?;
        if place.owner.is_some_and(|owner| owner != self.id) {
            return Err(SimulationError::PrivateProperty);
        }
        Ok(place)
    }

    pub fn action_duration_ms(&self, action: CitizenAction) -> Result<u64, SimulationError> {
        Ok(match action {
            CitizenAction::Travel(destination) => (self
                .position
                .distance(self.accessible_place(destination)?.position)
                * WALK_MS_PER_METRE)
                .ceil() as u64,
            CitizenAction::Eat => self
                .meal()
                .0
                .iter()
                .enumerate()
                .map(|(index, grams)| grams * Good::ALL[index].eating_ms_per_gram().unwrap_or(0.0))
                .sum::<f64>()
                .ceil() as u64,
            CitizenAction::Produce(recipe) => {
                self.production_place(recipe)?;
                recipe.duration_ms(self)?
            }
            CitizenAction::Wait | CitizenAction::Forage => ACTION_DURATION_MS,
            CitizenAction::Sleep => SLEEP_DURATION_MS,
            CitizenAction::BuyFood(_)
            | CitizenAction::List(..)
            | CitizenAction::Buy(..)
            | CitizenAction::Withdraw(..) => TRADE_DURATION_MS,
        })
    }

    pub fn food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| self.grams(good) * good.nutrition_per_gram().unwrap())
            .sum()
    }

    fn meal(&self) -> ([f64; Good::COUNT], f64) {
        let mut grams = [0.0; Good::COUNT];
        let mut remaining = MEAL_NOURISHMENT;
        let mut total = 0.0;
        for good in Good::FOOD {
            let nutrition = good.nutrition_per_gram().unwrap();
            let portion = self.grams(good).min(remaining / nutrition);
            grams[good as usize] = portion;
            total += portion * nutrition;
            remaining = (remaining - portion * nutrition).max(0.0);
        }
        (grams, total)
    }

    pub fn skill_level(&self, skill: production::Skill) -> f64 {
        self.skills[skill as usize]
    }

    pub fn with_skill(
        &self,
        skill: production::Skill,
        level: f64,
    ) -> Result<Self, SimulationError> {
        if !level.is_finite() || level < 0.0 {
            return Err(SimulationError::InvalidSkill);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.skills[skill as usize] = level;
        Ok(citizen)
    }

    pub fn production_place(&self, recipe: production::Recipe) -> Result<PlaceId, SimulationError> {
        let kind = recipe.location();
        if kind.is_public() {
            return Ok(self.map.public_place(kind));
        }
        self.owned_properties()
            .filter(|place| place.kind == kind)
            .map(|place| place.id)
            .min_by_key(|id| id.0)
            .ok_or(SimulationError::MissingProperty)
    }

    pub fn available_recipes(&self) -> impl Iterator<Item = production::Recipe> + '_ {
        production::Recipe::ALL.into_iter().filter(|recipe| {
            recipe.duration_ms(self).is_ok() && self.production_place(*recipe).is_ok()
        })
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
        if action
            .required_place(self)
            .is_some_and(|location| self.position != self.map.position(location))
        {
            return Err(SimulationError::WrongLocation);
        }
        if let CitizenAction::Travel(id) = action {
            self.accessible_place(id)?;
        }
        if let CitizenAction::List(_, grams) | CitizenAction::Withdraw(_, grams) = action {
            marketplace::validate_quantity(grams)?;
        }
        if let CitizenAction::Buy(list) = action {
            list.validate()?;
        }
        let mut citizen = self.clone();
        let duration_ms = self.action_duration_ms(action)?;
        if duration_ms == 0 {
            return Ok(citizen);
        }
        if let CitizenAction::Produce(recipe) = action
            && recipe
                .inputs()
                .iter()
                .any(|&(good, grams)| self.grams(good) < grams)
        {
            return Err(SimulationError::MissingInputs);
        }
        if let CitizenAction::BuyFood(good) = action
            && good.nutrition_per_gram().is_none()
        {
            return Err(SimulationError::NotFood);
        }
        let (meal_grams, meal_nutrition) = if action == CitizenAction::Eat {
            self.meal()
        } else {
            ([0.0; Good::COUNT], 0.0)
        };
        citizen.active_action = Some(ActiveAction {
            action,
            remaining_ms: duration_ms,
            duration_ms,
            meal_nutrition,
            meal_grams,
            inventory_after_meal: std::array::from_fn(|index| {
                self.grams(Good::ALL[index]) - meal_grams[index]
            }),
            travel_origin: self.position,
            travel_destination: if let CitizenAction::Travel(location) = action {
                Some(self.map.position(location))
            } else {
                None
            },
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

    /// Advances this isolated snapshot; use Universe for shared trade settlement.
    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        if let Some(execution) = &self.active_plan {
            return execution.advance(self, elapsed_ms, None);
        }
        self.advance_action(elapsed_ms, false)
    }

    pub(crate) fn advance_predicted(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        self.advance_action(elapsed_ms, true)
    }

    fn advance_action(&self, elapsed_ms: u64, prediction: bool) -> Result<Self, SimulationError> {
        Ok(self
            .advance_action_with_settlement(elapsed_ms, prediction, false)?
            .0)
    }

    fn advance_action_with_settlement(
        &self,
        elapsed_ms: u64,
        prediction: bool,
        record_trades: bool,
    ) -> Result<(Self, Vec<(AgentId, f64)>), SimulationError> {
        let mut citizen = self.clone();
        let mut payments = Vec::new();
        if elapsed_ms == 0 {
            return Ok((citizen, payments));
        }

        if let Some(mut active) = citizen.active_action {
            let action_elapsed_ms = elapsed_ms.min(active.remaining_ms);
            citizen.advance_needs(action_elapsed_ms, Some(active))?;
            active.remaining_ms -= action_elapsed_ms;
            if let Some(destination) = active.travel_destination {
                let fraction = 1.0 - active.remaining_ms as f64 / active.duration_ms as f64;
                citizen.position = if active.remaining_ms == 0 {
                    destination
                } else {
                    Position {
                        x: active.travel_origin.x
                            + (destination.x - active.travel_origin.x) * fraction,
                        y: active.travel_origin.y
                            + (destination.y - active.travel_origin.y) * fraction,
                    }
                };
            }

            if active.action == CitizenAction::Eat {
                for good in Good::FOOD {
                    let index = good as usize;
                    citizen.inventory.insert(
                        good,
                        active.inventory_after_meal[index]
                            + active.meal_grams[index]
                                * (active.remaining_ms as f64 / active.duration_ms as f64),
                    );
                }
            }

            if active.remaining_ms == 0 {
                match active.action {
                    CitizenAction::BuyFood(good) => {
                        let grams = (MEAL_NOURISHMENT - citizen.food_nutrition()).max(0.0)
                            / good.nutrition_per_gram().ok_or(SimulationError::NotFood)?;
                        payments.extend(citizen.complete_purchase(
                            good,
                            grams,
                            record_trades,
                            action_elapsed_ms,
                        )?);
                    }
                    CitizenAction::Produce(recipe) => citizen.complete_production(recipe)?,
                    CitizenAction::Buy(list) => {
                        for (good, grams) in list.items() {
                            payments.extend(citizen.complete_purchase(
                                good,
                                grams,
                                record_trades,
                                action_elapsed_ms,
                            )?);
                        }
                    }
                    CitizenAction::List(good, requested) => {
                        let grams = requested.min(citizen.grams(good));
                        if grams > 0.0 && citizen.grams(good) - grams == citizen.grams(good) {
                            return Err(SimulationError::WealthOverflow);
                        }
                        citizen.market.list(citizen.id, good, grams)?;
                        citizen.inventory.insert(good, citizen.grams(good) - grams);
                    }
                    CitizenAction::Withdraw(good, requested) => {
                        let grams = citizen.market.withdraw(citizen.id, good, requested)?;
                        let stock = citizen.grams(good) + grams;
                        if !stock.is_finite() || grams > 0.0 && stock == citizen.grams(good) {
                            return Err(SimulationError::WealthOverflow);
                        }
                        citizen.inventory.insert(good, stock);
                    }
                    CitizenAction::Forage => {
                        let grams = if prediction {
                            FORAGE_AVERAGE_GRAMS
                        } else {
                            citizen
                                .forage_rng
                                .random_range(FORAGE_MIN_GRAMS..=FORAGE_MAX_GRAMS)
                        };
                        let stock = citizen.berries_grams() + grams;
                        if !stock.is_finite() {
                            return Err(SimulationError::BerriesOverflow);
                        }
                        citizen.inventory.insert(Good::Berries, stock);
                    }
                    _ => {}
                }
                citizen.active_action = None;
            } else {
                citizen.active_action = Some(active);
            }
            citizen.advance_needs(elapsed_ms - action_elapsed_ms, None)?;
        } else {
            citizen.advance_needs(elapsed_ms, None)?;
        }
        citizen.market_time_ms = citizen
            .market_time_ms
            .checked_add(elapsed_ms)
            .ok_or(SimulationError::TimeOverflow)?;
        Ok((citizen, payments))
    }

    fn complete_production(&mut self, recipe: production::Recipe) -> Result<(), SimulationError> {
        for &(good, grams) in recipe.inputs() {
            let stock = self.grams(good);
            if stock < grams {
                return Err(SimulationError::MissingInputs);
            }
            if stock - grams == stock {
                return Err(SimulationError::InventoryOverflow);
            }
            self.inventory.insert(good, stock - grams);
        }
        for &(good, grams) in recipe.outputs() {
            let stock = self.grams(good);
            let next = stock + grams;
            if !next.is_finite() || next == stock {
                return Err(SimulationError::InventoryOverflow);
            }
            self.inventory.insert(good, next);
        }
        if let Some(skill) = recipe.skill() {
            self.skills[skill as usize] += production::SKILL_GAIN_PER_BATCH;
            if !self.skill_level(skill).is_finite() {
                return Err(SimulationError::InvalidSkill);
            }
        }
        Ok(())
    }

    fn complete_purchase(
        &mut self,
        good: Good,
        grams: f64,
        record_trades: bool,
        elapsed_ms: u64,
    ) -> Result<Vec<(AgentId, f64)>, SimulationError> {
        let purchase = self.market.purchase(
            self.id,
            good,
            grams,
            self.coins,
            self.market_time_ms
                .checked_add(elapsed_ms)
                .ok_or(SimulationError::TimeOverflow)?,
            record_trades,
        )?;
        let stock = self.grams(good) + purchase.grams;
        if !stock.is_finite() || purchase.grams > 0.0 && stock == self.grams(good) {
            return Err(SimulationError::WealthOverflow);
        }
        self.inventory.insert(good, stock);
        self.coins -= purchase.coins;
        Ok(purchase.payments)
    }

    fn advance_needs(
        &mut self,
        elapsed_ms: u64,
        active: Option<ActiveAction>,
    ) -> Result<(), SimulationError> {
        let elapsed_hours = elapsed_ms as f64 / 3_600_000.0;
        let nourishment_per_hour = active.map_or(0.0, |action| {
            action.meal_nutrition * 3_600_000.0 / action.duration_ms as f64
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

#[derive(Clone, Debug, PartialEq)]
pub struct Universe {
    current_time_ms: u64,
    market: Market,
    map: Map,
    agents: HashMap<AgentId, Agent>,
}

impl Default for Universe {
    fn default() -> Self {
        Self {
            current_time_ms: 0,
            market: Market::default(),
            map: Map::random(),
            agents: HashMap::new(),
        }
    }
}

impl Universe {
    pub fn map(&self) -> Map {
        self.map.clone()
    }

    pub fn with_map(map: Map) -> Self {
        Self {
            map,
            ..Self::default()
        }
    }

    pub fn prices(&self) -> Prices {
        self.market.prices
    }

    pub fn with_prices(&self, prices: Prices) -> Self {
        let mut universe = self.clone();
        universe.market.prices = prices;
        for (_, agent) in universe.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.prices = prices;
            citizen.market.prices = prices;
        }
        universe
    }

    pub fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }

    pub fn agents(&self) -> &HashMap<AgentId, Agent> {
        &self.agents
    }

    pub fn with_citizen(
        &self,
        name: impl Into<String>,
        citizen: Citizen,
    ) -> Result<(Self, AgentId), SimulationError> {
        let name = name.into();
        let id = citizen.id;
        if self.agents.contains_key(&id) {
            return Err(SimulationError::AgentAlreadyExists);
        }
        let mut universe = self.clone();
        let mut citizen = citizen.with_prices(self.prices());
        if citizen.map != self.map {
            let (map, home) = universe.map.with_place(
                Location::Home,
                universe.map.next_position(),
                Some(id),
                format!("{name}'s home"),
            )?;
            universe.map = map;
            citizen.home = home;
            citizen.position = universe.map.position(home);
            citizen.active_action = None;
            citizen.active_plan = None;
        }
        universe.map = universe
            .map
            .with_place_name(citizen.home, format!("{name}'s home"))?;
        citizen.map = universe.map.clone();
        citizen.market = universe.market.clone();
        citizen.market_time_ms = universe.current_time_ms;
        for (_, agent) in universe.agents.iter_mut() {
            let AgentKind::Citizen(existing) = &mut agent.kind;
            existing.map = universe.map.clone();
        }
        universe.agents.insert(
            id,
            Agent {
                name,
                kind: AgentKind::Citizen(citizen),
            },
        );
        Ok((universe, id))
    }

    pub fn with_property(
        &self,
        owner: AgentId,
        kind: Location,
    ) -> Result<(Self, PlaceId), SimulationError> {
        let agent = self
            .agents
            .get(&owner)
            .ok_or(SimulationError::AgentNotFound)?;
        let (map, id) = self.map.with_place(
            kind,
            self.map.next_position(),
            Some(owner),
            format!("{}'s {}", agent.name, kind.name()),
        )?;
        let mut universe = self.clone();
        universe.map = map;
        for (_, agent) in universe.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.map = universe.map.clone();
        }
        Ok((universe, id))
    }

    pub fn market(&self) -> &Market {
        &self.market
    }

    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        let end = self
            .current_time_ms
            .checked_add(elapsed_ms)
            .ok_or(SimulationError::TimeOverflow)?;
        let mut universe = self.clone();
        if universe.agents.is_empty() {
            universe.market.update(end);
            universe.current_time_ms = end;
            return Ok(universe);
        }
        while universe.current_time_ms < end {
            let until_update = marketplace::until_update(universe.current_time_ms);
            let mut ids: Vec<_> = universe.agents.keys().copied().collect();
            ids.sort_by_key(|id| id.0);
            let boundary = ids
                .iter()
                .filter_map(|id| {
                    let AgentKind::Citizen(citizen) = &universe.agents[id].kind;
                    citizen.active_action().map(|a| a.remaining_ms())
                })
                .min()
                .unwrap_or(end - universe.current_time_ms);
            let step = (end - universe.current_time_ms)
                .min(until_update)
                .min(boundary);
            let finishing_time = universe.current_time_ms + step;
            for id in &ids {
                let agent = universe.agents.get_mut(id).unwrap();
                let AgentKind::Citizen(citizen) = &mut agent.kind;
                citizen.market = universe.market.clone();
                citizen.prices = universe.market.prices;
                citizen.market_time_ms = universe.current_time_ms;
                let execution = citizen.active_plan.take();
                let (updated, payments) =
                    citizen.advance_action_with_settlement(step, false, true)?;
                *citizen = updated;
                if let Some(mut execution) = execution {
                    execution.record_elapsed(step)?;
                    citizen.active_plan = Some(execution);
                }
                universe.market = citizen.market.clone();
                for (seller, coins) in payments {
                    let seller = universe
                        .agents
                        .get_mut(&seller)
                        .ok_or(SimulationError::AgentNotFound)?;
                    let AgentKind::Citizen(seller) = &mut seller.kind;
                    seller.coins += coins;
                    if !seller.coins.is_finite() {
                        return Err(SimulationError::WealthOverflow);
                    }
                }
            }
            universe.current_time_ms = finishing_time;
            if step == until_update {
                universe.market.update(finishing_time);
            }
            for id in ids {
                let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
                citizen.market = universe.market.clone();
                citizen.prices = universe.market.prices;
                citizen.market_time_ms = finishing_time;
                if citizen.active_action.is_none()
                    && let Some(mut execution) = citizen.active_plan.take()
                {
                    execution.finish_action();
                    *citizen = execution.resume(citizen.clone())?;
                    citizen.active_plan = Some(execution);
                }
            }
        }
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
    InvalidInventory,
    InventoryOverflow,
    InvalidSkill,
    MissingSkill,
    MissingProperty,
    MissingInputs,
    NotFood,
    InvalidOwnership,
    PrivateProperty,
    PlaceNotFound,
    InvalidHunger,
    InvalidHungerRate,
    InvalidTiredness,
    InvalidBerries,
    InvalidQuantity,
    InvalidCoins,
    InvalidPrices,
    InvalidPosition,
    WrongLocation,
    WealthOverflow,
    TimeOverflow,
    HungerOverflow,
    TirednessOverflow,
    BerriesOverflow,
    WellbeingOverflow,
    CitizenBusy,
    AgentNotFound,
    AgentAlreadyExists,
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidInventory => "goods must have finite nonnegative grams",
            Self::InventoryOverflow => "production exceeds representable inventory quantities",
            Self::InvalidSkill => "skill level must be finite and nonnegative",
            Self::MissingSkill => "production requires its skill",
            Self::MissingProperty => "production requires an owned property",
            Self::MissingInputs => "production inputs are unavailable",
            Self::NotFood => "this good is not edible",
            Self::InvalidOwnership => {
                "private places require an owner and public places cannot have one"
            }
            Self::PrivateProperty => "citizen cannot use another citizen's private property",
            Self::PlaceNotFound => "place does not exist",
            Self::InvalidHunger => "hunger must be finite",
            Self::InvalidHungerRate => "hunger per hour must be finite and nonnegative",
            Self::InvalidTiredness => "tiredness must be finite",
            Self::InvalidBerries => "berry grams must be finite and nonnegative",
            Self::InvalidQuantity => "action grams must be finite and nonnegative",
            Self::InvalidPosition => "position must be finite and within the map",
            Self::WrongLocation => "action requires travel to its location first",
            Self::InvalidPrices => "prices must be finite and positive",
            Self::InvalidCoins => "coins must be finite",
            Self::WealthOverflow => "wealth exceeds the finite range",
            Self::TimeOverflow => "elapsed time exceeds the simulation clock's range",
            Self::HungerOverflow => "advancing time would produce nonfinite hunger",
            Self::TirednessOverflow => "advancing time would produce nonfinite tiredness",
            Self::BerriesOverflow => "foraging would produce nonfinite berry grams",
            Self::WellbeingOverflow => "personal wellbeing exceeds the finite score range",
            Self::CitizenBusy => "citizen is already performing an action",
            Self::AgentAlreadyExists => "citizen already exists in this universe",
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
