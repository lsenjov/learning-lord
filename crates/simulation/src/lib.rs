use imbl::HashMap;
use marketplace::{Good, Market, Prices};
use rand::{RngExt, rngs::Xoshiro256PlusPlus};
use std::fmt;
use uuid::Uuid;

pub mod calendar;
pub mod storage;
pub mod taxation;
use storage::{GoodsOwner, StockKey, Storage};
pub mod locations;
pub mod marketplace;
mod objects;
use locations::{Location, Map, PlaceId, Position, WALK_MS_PER_METRE};
mod background;
pub mod planning;
pub use background::PlanningRuntime;
pub mod brewing;
pub mod diet;
pub mod farming;
pub mod guild_history;
pub mod guilds;
pub mod history;
pub mod household_history;
pub mod households;
pub mod maintenance;
pub mod persistence;
pub mod production;
pub mod production_trials;

pub type Quantity = u64;
pub type Coins = i64;

type ActionSettlement = (Citizen, Vec<(AgentId, Coins)>, bool);

pub const WEALTH_WELLBEING_PER_COIN: f64 = 0.1;
pub const FOOD_RESERVE_FULL_BONUS_NUTRITION: f64 = 100.0;
pub const FOOD_RESERVE_CAP_NUTRITION: f64 = 300.0;
pub const WATER_RESERVE_GRAMS: Quantity = 400;
pub const WATER_RESERVE_CAP_GRAMS: Quantity = 1_200;

pub(crate) fn food_reserve_bonus(nutrition: f64) -> f64 {
    0.10 * nutrition.min(FOOD_RESERVE_FULL_BONUS_NUTRITION)
        + 0.02
            * (nutrition.min(FOOD_RESERVE_CAP_NUTRITION) - FOOD_RESERVE_FULL_BONUS_NUTRITION)
                .max(0.0)
}

pub(crate) fn water_reserve_bonus(grams: f64) -> f64 {
    0.025 * grams.min(WATER_RESERVE_GRAMS as f64)
        + 0.005 * (grams.min(WATER_RESERVE_CAP_GRAMS as f64) - WATER_RESERVE_GRAMS as f64).max(0.0)
}

pub const HUNGER_PER_HOUR: f64 = 100.0 / 24.0;
pub const THIRST_PER_HOUR: f64 = 100.0 / 24.0;
pub const DRINK_WATER_GRAMS: Quantity = 200;
pub const DRINK_DURATION_MS: u64 = 60_000;
pub const TIREDNESS_PER_HOUR: f64 = 100.0 / 24.0;
pub const MEAL_NOURISHMENT: f64 = 50.0;
pub const ACTION_DURATION_MS: u64 = 30 * 60 * 1000;
pub const GARMENT_LIFETIME_MS: u64 = 10 * 24 * 60 * 60_000;
pub const CLOTHING_MAX_PENALTY: f64 = 20.0;
pub const EQUIP_CLOTHING_DURATION_MS: u64 = 5 * 60_000;
pub const TRADE_DURATION_MS: u64 = 5 * 60 * 1000;
pub const SLEEP_DURATION_MS: u64 = 8 * 60 * 60 * 1000;
pub const SLEEP_RECOVERY: f64 = 100.0;
pub const BERRY_NUTRITION_PER_GRAM: f64 = 100.0 / 310.0;
pub const BERRY_EATING_MS_PER_GRAM: f64 = 1000.0;
pub const FORAGE_MIN_GRAMS: Quantity = 5;
pub const FORAGE_MAX_GRAMS: Quantity = 15;
pub const FORAGE_AVERAGE_GRAMS: Quantity = 10;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(pub Uuid);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum AgentKind {
    Citizen(Citizen),
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum CitizenAction {
    Eat,
    Drink,
    EquipClothing,
    Wait,
    Sleep,
    BuyFood(Good),
    Produce(production::Recipe),
    // A recipe alone cannot identify colocated workplaces.
    ProduceAt {
        place: PlaceId,
        recipe: production::Recipe,
    },
    Plant {
        field: PlaceId,
        plot: usize,
        crop: farming::CropKind,
    },
    Tend {
        field: PlaceId,
        plot: usize,
        stage: usize,
    },
    Harvest {
        field: PlaceId,
        plot: usize,
    },
    Clear {
        field: PlaceId,
        plot: usize,
    },
    CollectBeer {
        brewery: PlaceId,
        slot: usize,
    },
    ListExcess,
    List(Good, Quantity),
    ListAt {
        place: PlaceId,
        list: marketplace::ShoppingList,
    },
    Buy(marketplace::ShoppingList),
    BuyAt {
        place: PlaceId,
        list: marketplace::ShoppingList,
    },
    Withdraw(Good, Quantity),
    WithdrawAt {
        place: PlaceId,
        good: Good,
        units: Quantity,
    },
    CollectStored {
        place: PlaceId,
        list: marketplace::ShoppingList,
    },
    DepositStored {
        place: PlaceId,
        list: marketplace::ShoppingList,
    },
    Travel(PlaceId),
}

impl CitizenAction {
    pub fn production_recipe(self) -> Option<production::Recipe> {
        match self {
            Self::Produce(recipe) | Self::ProduceAt { recipe, .. } => Some(recipe),
            _ => None,
        }
    }

    pub fn required_place(self, citizen: &Citizen) -> Option<PlaceId> {
        match self {
            Self::CollectBeer { brewery, .. } => Some(brewery),
            Self::Sleep | Self::Eat | Self::Drink => Some(citizen.home),
            Self::Plant { field, .. }
            | Self::Tend { field, .. }
            | Self::Harvest { field, .. }
            | Self::Clear { field, .. } => Some(field),
            Self::Produce(recipe) => citizen.production_place(recipe).ok(),
            Self::ProduceAt { place, .. } => Some(place),
            Self::BuyFood(_) | Self::Buy(..) => Some(citizen.map.public_place(Location::Market)),
            Self::WithdrawAt { place, .. }
            | Self::BuyAt { place, .. }
            | Self::CollectStored { place, .. }
            | Self::DepositStored { place, .. } => Some(place),
            Self::ListAt { place, .. } => Some(place),
            Self::List(good, _) => Some(citizen.listing_place(good)),
            Self::ListExcess | Self::Withdraw(..) => Some(citizen.selling_place()),
            _ => None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ActiveAction {
    claimed_objects: [Option<locations::ObjectId>; 2],
    action: CitizenAction,
    remaining_ms: u64,
    duration_ms: u64,
    meal_duration_ms: u64,
    drink_good: Good,
    drink_units: Quantity,
    meal_nutrition: f64,
    meal_units: [Quantity; Good::COUNT],
    stored_inputs: marketplace::ShoppingList,
    listing_place: Option<PlaceId>,
    travel_origin: Position,
    travel_destination: Option<Position>,
}

impl ActiveAction {
    pub fn claimed_objects(&self) -> impl Iterator<Item = locations::ObjectId> {
        self.claimed_objects.into_iter().flatten()
    }

    pub fn action(&self) -> CitizenAction {
        self.action
    }

    pub fn remaining_ms(&self) -> u64 {
        self.remaining_ms
    }

    fn next_boundary_ms(&self) -> u64 {
        let elapsed = self.duration_ms - self.remaining_ms;
        [
            self.meal_duration_ms,
            self.drink_units * DRINK_DURATION_MS / DRINK_WATER_GRAMS,
        ]
        .into_iter()
        .filter(|duration| *duration > elapsed)
        .map(|duration| duration - elapsed)
        .min()
        .unwrap_or(self.remaining_ms)
        .min(self.remaining_ms)
    }

    pub fn drinking_good(&self) -> Good {
        self.drink_good
    }

    fn drink_duration_ms(&self) -> u64 {
        self.drink_units * DRINK_DURATION_MS / DRINK_WATER_GRAMS
    }

    fn drink_nutrition(&self) -> f64 {
        if self.drink_good == Good::Beer {
            self.drink_units as f64 * 0.05
        } else {
            0.0
        }
    }

    fn remaining_fraction(&self, duration: u64) -> f64 {
        duration.saturating_sub(self.duration_ms - self.remaining_ms) as f64
            / duration.max(1) as f64
    }

    pub fn drinking_units(&self) -> Quantity {
        self.drink_units
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartingRole {
    Farmer,
    Miller,
    Woodcutter,
    Baker,
    Weaver,
    Tailor,
    Fisher,
    Brewer,
}

impl StartingRole {
    pub fn name(self) -> &'static str {
        match self {
            Self::Farmer => "Farmer",
            Self::Miller => "Miller",
            Self::Woodcutter => "Woodcutter",
            Self::Baker => "Baker",
            Self::Weaver => "Weaver",
            Self::Tailor => "Tailor",
            Self::Fisher => "Fisher",
            Self::Brewer => "Brewer",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Citizen {
    household: Option<households::HouseholdContext>,
    guilds: HashMap<AgentId, guilds::GuildAccess>,
    workshop_fees: HashMap<AgentId, guilds::WorkshopFeeAccount>,
    maintenance: maintenance::MaintenanceState,
    designated_workplace: Option<PlaceId>,
    workplace_supplies: HashMap<PlaceId, (marketplace::ShoppingList, marketplace::ShoppingList)>,
    #[serde(default)]
    own_workplace_supplies:
        HashMap<PlaceId, (marketplace::ShoppingList, marketplace::ShoppingList)>,
    incoming_supplies: HashMap<PlaceId, marketplace::ShoppingList>,
    market_request: marketplace::ShoppingList,
    request_group: Option<PlaceId>,
    hunger: f64,
    thirst: f64,
    diet: [f64; 4],
    garment_condition: Option<f64>,
    hunger_per_hour: f64,
    tiredness: f64,
    inventory: HashMap<Good, Quantity>,
    tax_reserved: HashMap<Good, Quantity>,
    town_inventory: HashMap<Good, Quantity>,
    socage_rates: HashMap<(production::Recipe, Good), taxation::TaxRate>,
    site_socage_rates: HashMap<(PlaceId, Good), taxation::TaxRate>,
    sale_income_basis_points: HashMap<(PlaceId, Good), u32>,
    storage: Storage,
    id: AgentId,
    home: PlaceId,
    starting_role: Option<StartingRole>,
    skills: [Option<u64>; production::Skill::COUNT],
    production_targets: Option<production::ProductionTargets>,
    production_acquisitions: [Quantity; Good::COUNT],
    work_period: u64,
    work_ms: u64,
    coins: Coins,
    forage_rng: Xoshiro256PlusPlus,
    prices: Prices,
    market: Market,
    market_time_ms: u64,
    last_meal_started_ms: Option<u64>,
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
            thirst: 0.0,
            diet: [0.0; 4],
            garment_condition: None,
            hunger_per_hour,
            tiredness: 0.0,
            inventory: Good::ALL.into_iter().map(|good| (good, 0)).collect(),
            tax_reserved: HashMap::new(),
            town_inventory: HashMap::new(),
            socage_rates: HashMap::new(),
            site_socage_rates: HashMap::new(),
            sale_income_basis_points: HashMap::new(),
            storage: Storage::default(),
            household: None,
            guilds: HashMap::new(),
            workshop_fees: HashMap::new(),
            maintenance: maintenance::MaintenanceState::default(),
            designated_workplace: None,
            workplace_supplies: HashMap::new(),
            own_workplace_supplies: HashMap::new(),
            incoming_supplies: HashMap::new(),
            market_request: marketplace::ShoppingList::default(),
            request_group: None,
            id,
            home,
            starting_role: None,
            skills: [None; production::Skill::COUNT],
            production_targets: None,
            production_acquisitions: [0; Good::COUNT],
            work_period: 0,
            work_ms: 0,
            coins: 0,
            forage_rng: rand::make_rng(),
            prices: Prices::default(),
            market: Market::default(),
            market_time_ms: 0,
            last_meal_started_ms: None,
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

    pub fn thirst(&self) -> f64 {
        self.thirst
    }

    pub fn with_thirst(&self, thirst: f64) -> Result<Self, SimulationError> {
        if !thirst.is_finite() {
            return Err(SimulationError::InvalidThirst);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.thirst = thirst.max(-100.0);
        Ok(citizen)
    }

    fn drink_units_for(&self, good: Good) -> Quantity {
        self.drink_units_from_available(self.consumption_available_units()[good as usize])
    }

    fn drink_units_from_available(&self, available: Quantity) -> Quantity {
        available
            .min(DRINK_WATER_GRAMS)
            .min(((self.thirst + 100.0).max(0.0) * 4.0).floor() as Quantity)
    }

    #[cfg(test)]
    fn drink_units(&self) -> Quantity {
        self.preferred_drink().1
    }

    pub(crate) fn available_drink_units(&self) -> Quantity {
        self.available_units(Good::Water)
            .saturating_add(self.available_units(Good::Beer))
    }

    fn preferred_drink(&self) -> (Good, Quantity) {
        let available = self.consumption_available_units();
        let reserve =
            available[Good::Water as usize].saturating_add(available[Good::Beer as usize]);
        [Good::Water, Good::Beer]
            .into_iter()
            .map(|good| {
                (
                    good,
                    self.drink_units_from_available(available[good as usize]),
                )
            })
            .max_by(|a, b| {
                self.consumption_wellbeing([0; Good::COUNT], a.0, a.1, reserve)
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(
                        &self
                            .consumption_wellbeing([0; Good::COUNT], b.0, b.1, reserve)
                            .unwrap_or(f64::NEG_INFINITY),
                    )
                    .then_with(|| (b.0 as usize).cmp(&(a.0 as usize)))
            })
            .unwrap()
    }

    fn consumption_action(&self, action: CitizenAction) -> ActiveAction {
        let (meal_units, meal_nutrition, drink_good, drink_units) = if action == CitizenAction::Eat
        {
            self.preferred_consumption()
        } else if action == CitizenAction::Drink {
            let (good, units) = self.preferred_drink();
            ([0; Good::COUNT], 0.0, good, units)
        } else {
            ([0; Good::COUNT], 0.0, Good::Water, 0)
        };
        let meal_duration_ms = Good::FOOD
            .into_iter()
            .map(|good| meal_units[good as usize] * good.eating_ms_per_unit().unwrap())
            .sum::<u64>();
        let duration_ms = meal_duration_ms.max(drink_units * DRINK_DURATION_MS / DRINK_WATER_GRAMS);
        ActiveAction {
            claimed_objects: [None; 2],
            action,
            remaining_ms: duration_ms,
            duration_ms,
            meal_duration_ms,
            drink_good,
            drink_units,
            meal_nutrition,
            meal_units,
            stored_inputs: marketplace::ShoppingList::default(),
            listing_place: None,
            travel_origin: self.position,
            travel_destination: None,
        }
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
            StartingRole::Weaver => production::Skill::Weaving,
            StartingRole::Tailor => production::Skill::Tailoring,
            StartingRole::Fisher => production::Skill::Fishing,
            StartingRole::Brewer => production::Skill::Brewing,
        };
        citizen.skills[skill as usize] = Some(0);
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
    pub(crate) fn accessible_properties(&self) -> impl Iterator<Item = &locations::Place> {
        self.map
            .places()
            .values()
            .filter(|place| self.may_use_place(place))
    }
    pub fn units(&self, good: Good) -> Quantity {
        self.inventory.get(&good).copied().unwrap_or(0)
    }
    pub fn tax_reserved_units(&self, good: Good) -> Quantity {
        self.tax_reserved.get(&good).copied().unwrap_or(0)
    }
    pub fn town_carried_units(&self, good: Good) -> Quantity {
        self.town_inventory.get(&good).copied().unwrap_or(0)
    }
    pub fn available_units(&self, good: Good) -> Quantity {
        self.carried_available_units(good).saturating_add(
            self.colocated_storage_places()
                .into_iter()
                .map(|place| self.stored_available_units(place, good))
                .fold(0_u64, u64::saturating_add),
        )
    }
    pub fn production_socage_rate(
        &self,
        recipe: production::Recipe,
        good: Good,
    ) -> taxation::TaxRate {
        self.socage_rates
            .get(&(recipe, good))
            .copied()
            .unwrap_or_else(|| taxation::TaxRate::new(0).unwrap())
    }
    pub fn production_socage_rate_at(&self, place: PlaceId, good: Good) -> taxation::TaxRate {
        self.site_socage_rates
            .get(&(place, good))
            .copied()
            .unwrap_or_else(|| taxation::TaxRate::new(0).unwrap())
    }

    pub fn with_good(&self, good: Good, units: Quantity) -> Result<Self, SimulationError> {
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        if units < self.tax_reserved_units(good) {
            return Err(SimulationError::MissingInputs);
        }
        let mut citizen = self.clone();
        citizen.set_inventory_units(good, units);
        Ok(citizen)
    }
    pub fn berries_units(&self) -> Quantity {
        self.units(Good::Berries)
    }
    pub fn with_berries(&self, units: Quantity) -> Result<Self, SimulationError> {
        self.with_good(Good::Berries, units).map_err(|error| {
            if error == SimulationError::InvalidInventory {
                SimulationError::InvalidBerries
            } else {
                error
            }
        })
    }
    pub fn coins(&self) -> Coins {
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

    pub fn with_coins(&self, coins: Coins) -> Result<Self, SimulationError> {
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

    pub fn garment_condition(&self) -> Option<f64> {
        self.garment_condition
    }

    pub fn clothing_need(&self) -> f64 {
        CLOTHING_MAX_PENALTY * (1.0 - self.garment_condition.unwrap_or(0.0))
    }

    pub fn with_garment_condition(&self, condition: Option<f64>) -> Result<Self, SimulationError> {
        if condition.is_some_and(|value| !value.is_finite()) {
            return Err(SimulationError::InvalidGarmentCondition);
        }
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.garment_condition = condition.map(|value| value.clamp(0.0, 1.0));
        Ok(citizen)
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn wealth(&self) -> Result<f64, SimulationError> {
        if self.household.is_some() {
            return Ok(self.prices.value(Good::FlaxGarment, 1).unwrap_or(0.0)
                * self.garment_condition.unwrap_or(0.0));
        }
        self.economic_wealth()
    }

    pub(crate) fn economic_wealth(&self) -> Result<f64, SimulationError> {
        let mut wealth = self.coins as f64 - self.workshop_fee_liability()
            + self.prices.value(Good::FlaxGarment, 1).unwrap_or(0.0)
                * self.garment_condition.unwrap_or(0.0);
        for good in Good::ALL {
            let units = self
                .units(good)
                .checked_add(self.market.listed_units(self.market_party(), good))
                .and_then(|units| {
                    units.checked_add(self.storage.owned_units(self.goods_owner(), good))
                })
                .ok_or(SimulationError::WealthOverflow)?;
            wealth += self.prices.value(good, units).unwrap_or(0.0);
        }
        if let Some(active) = self.active_action {
            let drink_duration = active.drink_units * DRINK_DURATION_MS / DRINK_WATER_GRAMS;
            let elapsed = active.duration_ms - active.remaining_ms;
            if drink_duration > 0 {
                wealth += self
                    .prices
                    .value(active.drink_good, active.drink_units)
                    .unwrap_or(0.0)
                    * drink_duration.saturating_sub(elapsed) as f64
                    / drink_duration as f64;
            }
            let remaining = if active.meal_duration_ms == 0 {
                0.0
            } else {
                active.meal_duration_ms.saturating_sub(elapsed) as f64
                    / active.meal_duration_ms as f64
            };
            for good in Good::FOOD {
                wealth += self
                    .prices
                    .value(good, active.meal_units[good as usize])
                    .unwrap_or(0.0)
                    * remaining;
            }
        }
        for place in self.accessible_properties() {
            wealth += self.accessible_batches(place).flatten().count() as f64
                * self.prices.value(Good::Beer, 1000).unwrap_or(0.0)
                * 0.70;
            for crop in self.accessible_crops(place).flatten() {
                wealth += self
                    .prices
                    .value(crop.kind.good(), crop.expected_yield(self.market_time_ms))
                    .unwrap_or(0.0)
                    * 0.70;
            }
        }
        if let Some(household) = &self.household {
            for good in Good::ALL {
                wealth += self
                    .prices
                    .value(good, household.other_carried[good as usize])
                    .unwrap_or(0.0);
            }
            wealth += household.other_consumption_value;
            wealth -= self.prices.value(Good::FlaxGarment, 1).unwrap_or(0.0)
                * self.garment_condition.unwrap_or(0.0);
        }
        if !wealth.is_finite() {
            return Err(SimulationError::WealthOverflow);
        }
        Ok(wealth)
    }

    fn accessible_place(&self, id: PlaceId) -> Result<&locations::Place, SimulationError> {
        let place = self.map.place(id)?;
        if place.kind == Location::Home && !self.may_use_place(place) {
            return Err(SimulationError::PrivateProperty);
        }
        Ok(place)
    }

    pub fn action_duration_ms(&self, action: CitizenAction) -> Result<u64, SimulationError> {
        self.action_objects(action)?;
        Ok(match action {
            CitizenAction::Travel(destination) => (self
                .position
                .distance(self.accessible_place(destination)?.position)
                * WALK_MS_PER_METRE)
                .ceil() as u64,
            CitizenAction::Eat | CitizenAction::Drink => {
                self.consumption_action(action).duration_ms
            }
            CitizenAction::Produce(recipe) | CitizenAction::ProduceAt { recipe, .. } => {
                if matches!(
                    recipe,
                    production::Recipe::GrowWheat
                        | production::Recipe::GrowFlax
                        | production::Recipe::GrowCabbage
                        | production::Recipe::GrowHops
                ) {
                    return Err(SimulationError::MissingInputs);
                }
                if let CitizenAction::ProduceAt { place, .. } = action {
                    self.validate_production_site(place, recipe)?;
                } else {
                    self.production_place(recipe)?;
                }
                if recipe == production::Recipe::BrewBeer {
                    self.brewing_preparation_duration(action)?
                } else {
                    recipe.duration_ms(self)?
                }
            }
            CitizenAction::Plant { .. }
            | CitizenAction::Tend { .. }
            | CitizenAction::Harvest { .. }
            | CitizenAction::Clear { .. } => self.farming_duration_ms(action)?,
            CitizenAction::CollectBeer { brewery, slot } => {
                self.beer_collection_duration(brewery, slot)?
            }
            CitizenAction::Wait => ACTION_DURATION_MS,
            CitizenAction::Sleep => SLEEP_DURATION_MS,
            CitizenAction::EquipClothing => EQUIP_CLOTHING_DURATION_MS,
            CitizenAction::BuyFood(_)
            | CitizenAction::ListExcess
            | CitizenAction::List(..)
            | CitizenAction::ListAt { .. }
            | CitizenAction::Buy(..)
            | CitizenAction::BuyAt { .. }
            | CitizenAction::Withdraw(..)
            | CitizenAction::WithdrawAt { .. }
            | CitizenAction::CollectStored { .. }
            | CitizenAction::DepositStored { .. } => TRADE_DURATION_MS,
        })
    }

    pub fn food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| self.units(good) as f64 * good.nutrition_per_unit().unwrap())
            .sum::<f64>()
            + self.available_units(Good::Beer) as f64 * 0.05
    }

    pub fn available_food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| self.available_units(good) as f64 * good.nutrition_per_unit().unwrap())
            .sum::<f64>()
            + self.drink_units_for(Good::Beer) as f64 * 0.05
    }

    pub fn has_complete_meal(&self) -> bool {
        self.available_food_nutrition() >= MEAL_NOURISHMENT
    }

    pub fn skill_practice_ms(&self, skill: production::Skill) -> Option<u64> {
        self.skills[skill as usize]
    }

    /// Sets qualification and practice, capped at the maximum duration benefit.
    pub fn with_skill_practice_ms(
        &self,
        skill: production::Skill,
        practice_ms: Option<u64>,
    ) -> Result<Self, SimulationError> {
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        let mut citizen = self.clone();
        citizen.skills[skill as usize] =
            practice_ms.map(|ms| ms.min(production::MAX_SKILL_PRACTICE_MS));
        Ok(citizen)
    }

    pub fn selling_place(&self) -> PlaceId {
        if let Some(place) = self.designated_workplace.filter(|place| {
            self.map
                .place(*place)
                .is_ok_and(|place| !place.kind.is_public())
        }) {
            return place;
        }
        self.accessible_properties()
            .filter(|place| place.kind != Location::Home && !place.kind.is_public())
            .map(|place| place.id)
            .min_by_key(|id| id.0)
            .unwrap_or_else(|| self.map.public_place(Location::Market))
    }

    pub(crate) fn listing_place(&self, good: Good) -> PlaceId {
        let preferred = self.selling_place();
        if self.storage_object(preferred, good).is_ok() {
            preferred
        } else {
            self.map.public_place(Location::Market)
        }
    }

    pub fn production_place(&self, recipe: production::Recipe) -> Result<PlaceId, SimulationError> {
        let kind = recipe.location();
        if kind.is_public() {
            return Ok(self.map.public_place(kind));
        }
        self.accessible_properties()
            .filter(|place| self.validate_production_site(place.id, recipe).is_ok())
            .map(|place| place.id)
            .min_by_key(|id| id.0)
            .ok_or(SimulationError::MissingProperty)
    }

    pub(crate) fn validate_production_site(
        &self,
        place: PlaceId,
        recipe: production::Recipe,
    ) -> Result<(), SimulationError> {
        let place = self.map.place(place)?;
        let capability = locations::ObjectKind::for_recipe(recipe);
        if !place.objects().any(|object| {
            object.kind.same_capability(capability) && self.may_use_object(place, object)
        }) {
            return Err(SimulationError::MissingProperty);
        }
        Ok(())
    }

    pub(crate) fn production_sites(&self) -> Vec<PlaceId> {
        let mut sites: Vec<_> = self
            .map
            .places()
            .values()
            .filter(|place| {
                production::Recipe::ALL.into_iter().any(|recipe| {
                    recipe.duration_ms(self).is_ok()
                        && self.validate_production_site(place.id, recipe).is_ok()
                })
            })
            .map(|place| place.id)
            .collect();
        sites.sort_by_key(|place| place.0);
        sites
    }

    pub(crate) fn available_production_recipes(
        &self,
    ) -> impl Iterator<Item = production::Recipe> + '_ {
        production::Recipe::ALL.into_iter().filter(|recipe| {
            recipe.duration_ms(self).is_ok() && self.production_place(*recipe).is_ok()
        })
    }

    pub fn available_recipes(&self) -> impl Iterator<Item = production::Recipe> + '_ {
        self.available_production_recipes()
            .filter(|recipe| production::crop_kind(*recipe).is_none())
    }

    pub fn production_targets(&self) -> Option<&production::ProductionTargets> {
        self.production_targets.as_ref()
    }

    pub fn production_work_today_ms(&self) -> u64 {
        if self.work_period == production::work_period(self.market_time_ms) {
            self.work_ms
        } else {
            0
        }
    }

    pub(crate) fn withdrawable_units(&self, good: Good) -> Quantity {
        self.market
            .orders()
            .filter(|order| order.seller == self.market_party() && order.good == good)
            .map(|order| order.units)
            .sum()
    }

    pub(crate) fn withdraw_action(
        &self,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> CitizenAction {
        if place == self.selling_place() {
            CitizenAction::Withdraw(good, units)
        } else {
            CitizenAction::WithdrawAt { place, good, units }
        }
    }

    pub(crate) fn listed_sources(&self, good: Good) -> Vec<(PlaceId, Quantity)> {
        let mut sources = std::collections::HashMap::<PlaceId, Quantity>::new();
        for order in self
            .market
            .orders()
            .filter(|order| order.seller == self.market_party() && order.good == good)
        {
            let units = sources.entry(order.place).or_default();
            *units = units.saturating_add(order.units);
        }
        let mut sources: Vec<_> = sources.into_iter().collect();
        sources.sort_by(|(a, _), (b, _)| {
            self.position
                .distance(self.map.position(*a))
                .total_cmp(&self.position.distance(self.map.position(*b)))
                .then_with(|| a.0.cmp(&b.0))
        });
        sources
    }

    pub(crate) fn usable_owned_units(&self, good: Good) -> Quantity {
        self.carried_available_units(good)
            .saturating_add(self.withdrawable_units(good))
            .saturating_add(
                self.stored_sources(good)
                    .iter()
                    .map(|(_, units)| *units)
                    .fold(0_u64, u64::saturating_add),
            )
    }

    #[cfg(test)]
    pub(crate) fn production_restock_recipes(
        &self,
    ) -> Result<Option<Vec<production::Recipe>>, SimulationError> {
        self.production_restock_recipes_at(None)
    }

    pub(crate) fn production_restock_recipes_at(
        &self,
        site: Option<PlaceId>,
    ) -> Result<Option<Vec<production::Recipe>>, SimulationError> {
        let recipes = if let Some(targets) = self
            .production_targets
            .as_ref()
            .filter(|targets| site.is_some() && targets.site == site)
        {
            targets.projected_order.clone()
        } else {
            production::projected_recipes_at(self, site, production::DAILY_CAPACITY_MS)?
        };
        let mut duration = 0;
        let preferred: Vec<_> = recipes
            .iter()
            .copied()
            .take_while(|recipe| {
                if duration >= production::PRODUCTION_BLOCK_MS {
                    return false;
                }
                duration += recipe.duration_ms(self).unwrap_or(0);
                true
            })
            .collect();
        let threshold = production::recipe_requirements_at(self, site, &preferred)?;
        if threshold
            .items()
            .all(|(good, units)| self.usable_owned_units(good) >= units)
        {
            return Ok(None);
        }
        Ok(Some(recipes))
    }

    #[cfg(test)]
    pub(crate) fn production_restock_requirements(
        &self,
    ) -> Result<marketplace::ShoppingList, SimulationError> {
        self.production_restock_recipes()?.map_or_else(
            || Ok(marketplace::ShoppingList::default()),
            |recipes| production::recipe_requirements(self, &recipes),
        )
    }

    fn set_inventory_units(&mut self, good: Good, units: Quantity) {
        self.inventory.insert(good, units);
        self.production_acquisitions[good as usize] =
            self.production_acquisitions[good as usize].min(self.available_units(good));
    }

    fn retain_production_input(
        &mut self,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        if self.production_targets.is_some()
            && self
                .available_recipes()
                .any(|recipe| recipe.inputs().iter().any(|&(input, _)| input == good))
        {
            self.production_acquisitions[good as usize] = self.production_acquisitions
                [good as usize]
                .checked_add(units)
                .ok_or(SimulationError::InventoryOverflow)?;
        }
        Ok(())
    }

    fn production_ingredient_reserve(&self) -> Result<[Quantity; Good::COUNT], SimulationError> {
        let active = self.active_action.and_then(|active| match active.action {
            CitizenAction::Produce(recipe) | CitizenAction::ProduceAt { recipe, .. } => {
                Some(recipe)
            }
            _ => None,
        });
        let mut reserve = [0_u64; Good::COUNT];
        if let Some(targets) = &self.production_targets {
            let mut future = targets.projected_order.clone();
            if let Some(recipe) = active
                && let Some(index) = future.iter().position(|&planned| planned == recipe)
            {
                future.remove(index);
            }
            let requirements = production::recipe_requirements_at(self, targets.site, &future)?;
            for (good, units) in requirements.items() {
                reserve[good as usize] = units;
            }
        }
        if let Some(recipe) = active
            && recipe != production::Recipe::BrewBeer
        {
            for &(good, units) in recipe.inputs() {
                reserve[good as usize] = reserve[good as usize]
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?;
            }
        }
        for good in Good::ALL {
            reserve[good as usize] = reserve[good as usize]
                .max(self.production_acquisitions[good as usize].min(self.available_units(good)));
        }
        Ok(reserve)
    }

    pub(crate) fn spare_clothing_target(&self) -> u64 {
        u64::from(self.garment_condition().unwrap_or(0.0) <= 0.2)
            + self
                .household
                .as_ref()
                .map_or(0, |household| household.other_clothing_replacements)
    }

    pub fn reserved_goods(&self) -> Result<marketplace::ShoppingList, SimulationError> {
        let mut reserve = self.production_ingredient_reserve()?;
        reserve[Good::FlaxGarment as usize] = reserve[Good::FlaxGarment as usize]
            .checked_add(self.spare_clothing_target())
            .ok_or(SimulationError::InventoryOverflow)?;
        if let Some(place) = self.current_storage_place()
            && let Some((_, target)) = self.workplace_supplies.get(&place)
        {
            for good in Good::ALL {
                reserve[good as usize] = reserve[good as usize].max(target.units(good));
            }
        }
        let available = std::array::from_fn(|index| {
            self.available_units(Good::ALL[index])
                .saturating_sub(reserve[index])
        });
        let food = self.retained_food(available);
        for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
            reserve[good as usize] += food[good as usize];
        }
        let mut retained_drinks =
            food[Good::Water as usize].saturating_add(food[Good::Beer as usize]);
        for good in [Good::Water, Good::Beer] {
            let additional = available[good as usize]
                .saturating_sub(food[good as usize])
                .min(
                    WATER_RESERVE_GRAMS
                        .saturating_mul(self.supplied_members())
                        .saturating_sub(retained_drinks),
                );
            reserve[good as usize] += additional;
            retained_drinks += additional;
        }
        reserve[Good::Water as usize] = reserve[Good::Water as usize]
            .checked_add(
                WATER_RESERVE_GRAMS
                    .saturating_mul(self.supplied_members())
                    .saturating_sub(retained_drinks),
            )
            .ok_or(SimulationError::InventoryOverflow)?;
        marketplace::ShoppingList::new(
            Good::ALL
                .into_iter()
                .map(|good| (good, reserve[good as usize])),
        )
    }

    pub fn purchase_shortages(&self) -> Result<marketplace::ShoppingList, SimulationError> {
        let Some(targets) = &self.production_targets else {
            return Ok(marketplace::ShoppingList::default());
        };
        let mut duration = 0;
        let prefix: Vec<_> = targets
            .projected_order
            .iter()
            .copied()
            .take_while(|recipe| {
                if duration >= production::PRODUCTION_BLOCK_MS {
                    return false;
                }
                duration += recipe.duration_ms(self).unwrap_or(0);
                true
            })
            .collect();
        if targets.site.is_none() {
            return Ok(marketplace::ShoppingList::default());
        }
        let threshold = production::recipe_requirements_at(self, targets.site, &prefix)?;
        if threshold
            .items()
            .all(|(good, units)| self.usable_owned_units(good) >= units)
        {
            return Ok(marketplace::ShoppingList::default());
        }
        let requirements =
            production::recipe_requirements_at(self, targets.site, &targets.projected_order)?;
        let needed = std::array::from_fn::<_, { Good::COUNT }, _>(|index| {
            let good = Good::ALL[index];
            requirements
                .units(good)
                .saturating_sub(self.usable_owned_units(good))
        });
        marketplace::ShoppingList::new(
            Good::ALL
                .into_iter()
                .map(|good| (good, needed[good as usize])),
        )
    }

    pub fn excess_goods(&self) -> Result<marketplace::ShoppingList, SimulationError> {
        let reserve = self.reserved_goods()?;
        marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
            (
                good,
                self.available_units(good)
                    .saturating_sub(reserve.units(good)),
            )
        }))
    }

    pub fn excess_value(&self) -> Result<f64, SimulationError> {
        let value: f64 = self
            .excess_goods()?
            .items()
            .map(|(good, units)| self.prices.value(good, units).unwrap())
            .sum();
        if !value.is_finite() {
            return Err(SimulationError::WealthOverflow);
        }
        Ok(value)
    }

    fn refresh_production_targets(&mut self, force: bool) -> Result<(), SimulationError> {
        if self.starting_role.is_none() {
            return Ok(());
        }
        let refresh = force
            || self.production_targets.as_ref().is_none_or(|targets| {
                targets.is_empty()
                    || production::work_period(targets.calculated_at_ms)
                        != production::work_period(self.market_time_ms)
            });
        if refresh {
            self.production_targets = Some(production::ProductionTargets::calculate(
                self,
                self.market_time_ms,
            )?);
        }
        Ok(())
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
        let action = if action == CitizenAction::ListExcess {
            let place = self.selling_place();
            let list = self
                .listing_baskets(self.excess_goods()?)?
                .into_iter()
                .find(|(site, _)| *site == place)
                .map(|(_, list)| list)
                .unwrap_or_default();
            CitizenAction::ListAt { place, list }
        } else {
            action
        };
        if let CitizenAction::ProduceAt { place, recipe } = action {
            self.validate_production_site(place, recipe)?;
        }
        if let CitizenAction::ListAt { place, list } = action {
            if self.accessible_place(place)?.kind == Location::Home {
                return Err(SimulationError::PrivateProperty);
            }
            for (good, units) in list.items() {
                if units > 0 {
                    self.storage_object(place, good)?;
                }
            }
        }
        if let CitizenAction::WithdrawAt { place, .. } = action {
            self.accessible_place(place)?;
        }
        if let CitizenAction::BuyAt { place, .. } = action {
            let site = self.accessible_place(place)?;
            if site.kind == Location::Home {
                return Err(SimulationError::PrivateProperty);
            }
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
        if let CitizenAction::CollectStored { place, list }
        | CitizenAction::DepositStored { place, list } = action
        {
            self.accessible_place(place)?;
            for (good, units) in list.items() {
                let available = if matches!(action, CitizenAction::CollectStored { .. }) {
                    self.stored_available_units(place, good)
                } else {
                    self.carried_available_units(good)
                };
                if units > available {
                    return Err(SimulationError::MissingInputs);
                }
                if matches!(action, CitizenAction::DepositStored { .. }) && units > 0 {
                    self.storage_object(place, good)?;
                }
            }
        }
        let mut citizen = self.clone();
        let claimed_objects = self.action_objects(action)?;
        let consumption = self.consumption_action(action);
        let ActiveAction {
            meal_units,
            meal_nutrition,
            meal_duration_ms,
            drink_good,
            drink_units,
            ..
        } = consumption;
        let duration_ms = if matches!(action, CitizenAction::Eat | CitizenAction::Drink) {
            consumption.duration_ms
        } else {
            self.action_duration_ms(action)?
        };
        if duration_ms == 0 {
            return Ok(citizen);
        }
        if let Some(recipe) = action.production_recipe()
            && recipe.inputs().iter().any(|&(good, units)| {
                self.carried_available_units(good).saturating_add(
                    action
                        .required_place(self)
                        .map_or(0, |place| self.stored_available_units(place, good)),
                ) < units
            })
        {
            return Err(SimulationError::MissingInputs);
        }
        if let CitizenAction::BuyFood(good) = action
            && good.nutrition_per_unit().is_none()
        {
            return Err(SimulationError::NotFood);
        }
        if action == CitizenAction::EquipClothing && self.available_units(Good::FlaxGarment) == 0 {
            return Err(SimulationError::MissingInputs);
        }
        for good in Good::FOOD {
            citizen.consume_home_goods(good, meal_units[good as usize])?;
        }
        citizen.consume_home_goods(drink_good, drink_units)?;
        if action == CitizenAction::Eat {
            citizen.last_meal_started_ms = Some(self.market_time_ms);
        }
        if claimed_objects[0].is_some() {
            citizen.map.claim_objects(
                action.required_place(self).unwrap(),
                claimed_objects,
                self.id,
            )?;
        }
        if action.production_recipe() == Some(production::Recipe::BrewBeer) {
            citizen.start_brewing(action, claimed_objects[1].unwrap())?;
        }
        let mut stored_inputs = marketplace::ShoppingList::default();
        if let Some(recipe) = action
            .production_recipe()
            .filter(|recipe| *recipe != production::Recipe::BrewBeer)
            && let Some(place) = action.required_place(&citizen)
        {
            stored_inputs =
                marketplace::ShoppingList::new(recipe.inputs().iter().map(|&(good, units)| {
                    (
                        good,
                        units.saturating_sub(citizen.carried_available_units(good)),
                    )
                }))?;
            for (good, units) in stored_inputs.items() {
                citizen.storage.reserve_work_for(
                    citizen.id,
                    StockKey {
                        place,
                        owner: citizen.goods_owner(),
                        good,
                    },
                    units,
                    &citizen.accessible_storage_objects(place),
                )?;
            }
        }
        let listing = match action {
            CitizenAction::List(good, units) => Some((
                action.required_place(self).unwrap(),
                marketplace::ShoppingList::single(good, units),
            )),
            CitizenAction::ListAt { place, list } => Some((place, list)),
            _ => None,
        };
        citizen.active_action = Some(ActiveAction {
            claimed_objects,
            stored_inputs,
            listing_place: matches!(action, CitizenAction::List(..))
                .then(|| action.required_place(self).unwrap()),
            action,
            remaining_ms: duration_ms,
            duration_ms,
            meal_nutrition,
            meal_duration_ms,
            drink_good,
            drink_units,
            meal_units,
            travel_origin: self.position,
            travel_destination: if let CitizenAction::Travel(location) = action {
                Some(self.map.position(location))
            } else {
                None
            },
        });
        let active = citizen.active_action.unwrap();
        let stored_output = action.production_recipe().is_some_and(|recipe| {
            recipe != production::Recipe::BrewBeer
                && action.required_place(self).is_some_and(|place| {
                    self.map
                        .place(place)
                        .is_ok_and(|site| !site.kind.is_public())
                })
        });
        if stored_output
            || matches!(
                action,
                CitizenAction::Harvest { .. }
                    | CitizenAction::CollectBeer { .. }
                    | CitizenAction::DepositStored { .. }
            )
            || listing.is_some()
        {
            let mut preview = citizen.clone();
            match action {
                CitizenAction::Produce(recipe) | CitizenAction::ProduceAt { recipe, .. }
                    if recipe != production::Recipe::BrewBeer =>
                {
                    preview.complete_production(recipe, true, active)?
                }
                CitizenAction::Harvest { .. } => preview.complete_farming(
                    action,
                    self.market_time_ms
                        .checked_add(duration_ms)
                        .ok_or(SimulationError::TimeOverflow)?,
                )?,
                CitizenAction::CollectBeer { brewery, slot } => {
                    preview.collect_beer(brewery, slot)?
                }
                CitizenAction::DepositStored { place, list } => {
                    preview.transfer_stored_action(place, list, true)?
                }
                _ if listing.is_some() => {
                    let (place, list) = listing.unwrap();
                    for (good, units) in list.items() {
                        let object = preview.storage_object(place, good)?;
                        preview.market.list_in(
                            preview.market_party(),
                            place,
                            good,
                            units,
                            object,
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(citizen)
    }

    pub fn food_reserve_wellbeing(&self) -> f64 {
        let nutrition = self.home_food_nutrition()
            + self.active_action.map_or(0.0, |active| {
                active.drink_nutrition() * active.remaining_fraction(active.drink_duration_ms())
                    + active.meal_nutrition
                        * active
                            .meal_duration_ms
                            .saturating_sub(active.duration_ms - active.remaining_ms)
                            as f64
                        / active.meal_duration_ms.max(1) as f64
            });
        food_reserve_bonus(nutrition / self.supplied_members() as f64)
    }

    pub fn drinking_water_reserve(&self) -> f64 {
        self.home_drink_units() as f64
            + self.active_action.map_or(0.0, |active| {
                let duration = active.drink_units * DRINK_DURATION_MS / DRINK_WATER_GRAMS;
                active.drink_units as f64
                    * duration.saturating_sub(active.duration_ms - active.remaining_ms) as f64
                    / duration.max(1) as f64
            })
    }

    pub fn water_reserve_wellbeing(&self) -> f64 {
        water_reserve_bonus(self.drinking_water_reserve() / self.supplied_members() as f64)
    }

    pub fn needs_wellbeing(&self) -> f64 {
        -self.clothing_need()
            - self.thirst.max(0.0)
            - 4.0 * (self.thirst - 100.0).max(0.0)
            - self.hunger.max(0.0)
            - 4.0 * (self.hunger - 100.0).max(0.0)
            - (-self.hunger - 100.0).max(0.0)
            - self.tiredness.max(0.0)
            + self.food_reserve_wellbeing()
            + self.water_reserve_wellbeing()
            + self.dietary_wellbeing()
    }

    pub fn personal_wellbeing(&self) -> Result<f64, SimulationError> {
        let wellbeing = self.needs_wellbeing() + self.wealth()? * WEALTH_WELLBEING_PER_COIN;
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
        let (citizen, _, failed) =
            self.advance_action_with_settlement(elapsed_ms, prediction, false)?;
        if prediction && failed {
            return Err(SimulationError::MissingInputs);
        }
        Ok(citizen)
    }

    fn advance_action_with_settlement(
        &self,
        elapsed_ms: u64,
        prediction: bool,
        record_trades: bool,
    ) -> Result<ActionSettlement, SimulationError> {
        let mut citizen = self.clone();
        let mut payments = Vec::new();
        let mut completion_failed = false;
        if elapsed_ms == 0 {
            return Ok((citizen, payments, false));
        }

        if let Some(mut active) = citizen.active_action {
            let action_elapsed_ms = elapsed_ms.min(active.remaining_ms);
            citizen.advance_needs(action_elapsed_ms, Some(active))?;
            if active.action.production_recipe().is_some() {
                let place = active
                    .action
                    .required_place(&citizen)
                    .ok_or(SimulationError::MissingProperty)?;
                if let Some((payee, paid)) =
                    citizen.accrue_workshop_fee(place, action_elapsed_ms)?
                {
                    payments.push((payee, paid));
                }
            }
            let work_skill = match active.action {
                CitizenAction::Produce(recipe) | CitizenAction::ProduceAt { recipe, .. } => {
                    recipe.skill()
                }
                CitizenAction::CollectBeer { .. } => Some(production::Skill::Brewing),
                action if action.crop_plot().is_some() => Some(production::Skill::Farming),
                _ => None,
            };
            if active.action.production_recipe().is_some()
                || matches!(active.action, CitizenAction::CollectBeer { .. })
                || active.action.crop_plot().is_some()
                || matches!(active.action, CitizenAction::Travel(_))
                    && citizen
                        .skill_practice_ms(production::Skill::Farming)
                        .is_some()
            {
                if let Some(skill) = work_skill {
                    let practice = citizen.skills[skill as usize]
                        .as_mut()
                        .ok_or(SimulationError::MissingSkill)?;
                    *practice = practice
                        .saturating_add(action_elapsed_ms)
                        .min(production::MAX_SKILL_PRACTICE_MS);
                }
                let end = self
                    .market_time_ms
                    .checked_add(action_elapsed_ms)
                    .ok_or(SimulationError::TimeOverflow)?;
                let period = production::work_period(end);
                if period != self.work_period {
                    citizen.work_ms =
                        (end - production::period_start(period)).min(action_elapsed_ms);
                } else {
                    citizen.work_ms = citizen
                        .work_ms
                        .checked_add(action_elapsed_ms)
                        .ok_or(SimulationError::TimeOverflow)?;
                }
                citizen.work_period = period;
            }
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

            if active.remaining_ms == 0 {
                citizen
                    .market
                    .set_time(citizen.market_time_ms.saturating_add(action_elapsed_ms));
                let checkpoint = citizen.clone();
                let payments_before = payments.len();
                let completion = (|| -> Result<(), SimulationError> {
                    match active.action {
                        CitizenAction::CollectStored { place, list }
                        | CitizenAction::DepositStored { place, list } => {
                            match citizen.transfer_stored_action(
                                place,
                                list,
                                matches!(active.action, CitizenAction::DepositStored { .. }),
                            ) {
                                Ok(()) => {}
                                Err(error) => return Err(error),
                            }
                        }
                        CitizenAction::BuyFood(good) => {
                            let units = ((MEAL_NOURISHMENT - citizen.available_food_nutrition())
                                .max(0.0)
                                / good.nutrition_per_unit().ok_or(SimulationError::NotFood)?)
                            .ceil() as Quantity;
                            payments.extend(citizen.complete_purchase(
                                citizen.map.public_place(Location::Market),
                                good,
                                units,
                                record_trades,
                                action_elapsed_ms,
                            )?);
                        }
                        CitizenAction::Produce(recipe)
                        | CitizenAction::ProduceAt { recipe, .. } => {
                            if recipe == production::Recipe::BrewBeer {
                                citizen.finish_preparation(
                                    active.action,
                                    self.market_time_ms
                                        .checked_add(action_elapsed_ms)
                                        .ok_or(SimulationError::TimeOverflow)?,
                                )?;
                            } else {
                                citizen.complete_production(recipe, prediction, active)?;
                            }
                        }
                        action if action.crop_plot().is_some() => {
                            let finish = self
                                .market_time_ms
                                .checked_add(action_elapsed_ms)
                                .ok_or(SimulationError::TimeOverflow)?;
                            citizen.complete_farming(action, finish)?;
                        }
                        CitizenAction::CollectBeer { brewery, slot } => {
                            match citizen.collect_beer(brewery, slot) {
                                Ok(()) => {}
                                Err(error) => return Err(error),
                            }
                        }
                        CitizenAction::EquipClothing => {
                            match citizen.consume_goods(Good::FlaxGarment, 1) {
                                Ok(()) => citizen.garment_condition = Some(1.0),
                                Err(SimulationError::MissingInputs) => {
                                    return Err(SimulationError::MissingInputs);
                                }
                                Err(error) => return Err(error),
                            }
                        }
                        CitizenAction::Buy(list) | CitizenAction::BuyAt { list, .. } => {
                            let place = active.action.required_place(&citizen).unwrap();
                            let mut budget = citizen.coins.min(list.spending_budget());
                            for (good, units) in list.items() {
                                let before = citizen.coins;
                                payments.extend(citizen.complete_purchase_at_quote(
                                    place,
                                    good,
                                    units,
                                    record_trades,
                                    action_elapsed_ms,
                                    (budget, list.quote_ceiling(good)),
                                )?);
                                budget -= before - citizen.coins;
                            }
                        }
                        CitizenAction::ListExcess
                        | CitizenAction::ListAt { .. }
                        | CitizenAction::List(..) => {
                            let (place, list) = match active.action {
                                CitizenAction::ListAt { place, list } => (place, list),
                                CitizenAction::List(good, units) => (
                                    active.listing_place.ok_or(SimulationError::MissingInputs)?,
                                    marketplace::ShoppingList::single(good, units),
                                ),
                                _ => return Err(SimulationError::MissingInputs),
                            };
                            let mut listed = false;
                            for (good, requested) in list.items() {
                                let units = requested.min(citizen.available_units(good));
                                citizen.list_trial_stock_at(place, good, units)?;
                                listed |= units > 0;
                            }
                            if list.items().next().is_some() && !listed {
                                return Err(SimulationError::MissingInputs);
                            }
                        }
                        CitizenAction::Withdraw(good, requested)
                        | CitizenAction::WithdrawAt {
                            good,
                            units: requested,
                            ..
                        } => {
                            let place = active.action.required_place(&citizen).unwrap();
                            let units = citizen.market.withdraw(
                                citizen.market_party(),
                                place,
                                good,
                                requested,
                            )?;
                            let stock = citizen
                                .units(good)
                                .checked_add(units)
                                .ok_or(SimulationError::WealthOverflow)?;
                            citizen.set_inventory_units(good, stock);
                            citizen.retain_production_input(good, units)?;
                        }
                        _ => {}
                    }
                    Ok(())
                })();
                match completion {
                    Ok(()) => citizen.complete_trial_output(active.action),
                    Err(SimulationError::MissingInputs) => {
                        citizen = checkpoint;
                        payments.truncate(payments_before);
                        citizen.abandon_action()?;
                        citizen.cancel_preparing_trials();
                        completion_failed = true;
                    }
                    Err(error) => return Err(error),
                }
                citizen.map.release_objects(
                    active
                        .action
                        .required_place(&citizen)
                        .unwrap_or(citizen.home),
                    active.claimed_objects,
                    citizen.id,
                );
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
        citizen.reconcile_production_trials();
        Ok((citizen, payments, completion_failed))
    }

    fn complete_production(
        &mut self,
        recipe: production::Recipe,
        prediction: bool,
        active: ActiveAction,
    ) -> Result<(), SimulationError> {
        for (good, units) in active.stored_inputs.items() {
            let place = active
                .action
                .required_place(self)
                .ok_or(SimulationError::MissingProperty)?;
            self.storage.release_work_for(
                self.id,
                StockKey {
                    place,
                    owner: self.goods_owner(),
                    good,
                },
                units,
                true,
            )?;
        }
        for &(good, units) in recipe.inputs() {
            self.consume_at(
                active
                    .action
                    .required_place(self)
                    .ok_or(SimulationError::MissingProperty)?,
                good,
                units - active.stored_inputs.units(good),
            )?;
        }
        for &(good, units) in recipe.outputs() {
            let units = if recipe == production::Recipe::Forage && !prediction {
                self.forage_rng
                    .random_range(FORAGE_MIN_GRAMS..=FORAGE_MAX_GRAMS)
            } else {
                units
            };
            self.receive_production_output(
                active
                    .action
                    .required_place(self)
                    .ok_or(SimulationError::MissingProperty)?,
                good,
                units,
            )?;
        }
        if let Some(targets) = &mut self.production_targets {
            targets.complete(recipe);
        }
        Ok(())
    }

    fn complete_purchase(
        &mut self,
        place: PlaceId,
        good: Good,
        units: Quantity,
        record_trades: bool,
        elapsed_ms: u64,
    ) -> Result<Vec<(AgentId, Coins)>, SimulationError> {
        self.complete_purchase_at_quote(
            place,
            good,
            units,
            record_trades,
            elapsed_ms,
            (self.coins, f64::MAX),
        )
    }

    fn complete_purchase_at_quote(
        &mut self,
        place: PlaceId,
        good: Good,
        units: Quantity,
        record_trades: bool,
        elapsed_ms: u64,
        bounds: (Coins, f64),
    ) -> Result<Vec<(AgentId, Coins)>, SimulationError> {
        let (budget, ceiling) = bounds;
        let purchase = self.market.purchase_at_quote(
            self.market_party(),
            place,
            (good, units),
            (self.coins.min(budget), ceiling),
            self.market_time_ms
                .checked_add(elapsed_ms)
                .ok_or(SimulationError::TimeOverflow)?,
            record_trades,
        )?;
        let stock = self
            .units(good)
            .checked_add(purchase.units)
            .ok_or(SimulationError::WealthOverflow)?;
        self.set_inventory_units(good, stock);
        self.retain_production_input(good, purchase.units)?;
        self.coins = self
            .coins
            .checked_sub(purchase.coins)
            .ok_or(SimulationError::WealthOverflow)?;
        Ok(purchase.payments)
    }

    fn advance_needs(
        &mut self,
        elapsed_ms: u64,
        active: Option<ActiveAction>,
    ) -> Result<(), SimulationError> {
        self.advance_diet(elapsed_ms, active);
        self.garment_condition = self.garment_condition.map(|condition| {
            (condition - elapsed_ms as f64 / GARMENT_LIFETIME_MS as f64).clamp(0.0, 1.0)
        });
        let elapsed_hours = elapsed_ms as f64 / 3_600_000.0;
        let consumed_fraction = |duration: u64, action: ActiveAction| {
            if duration == 0 {
                return 0.0;
            }
            let started = action.duration_ms - action.remaining_ms;
            elapsed_ms.min(duration.saturating_sub(started)) as f64 / duration as f64
        };
        let nourishment = active.map_or(0.0, |action| {
            action.meal_nutrition * consumed_fraction(action.meal_duration_ms, action)
                + action.drink_nutrition() * consumed_fraction(action.drink_duration_ms(), action)
        });
        let hydration = active.map_or(0.0, |action| {
            action.drink_units as f64 / 4.0
                * consumed_fraction(
                    action.drink_units * DRINK_DURATION_MS / DRINK_WATER_GRAMS,
                    action,
                )
        });
        let thirst = self.thirst + THIRST_PER_HOUR * elapsed_hours - hydration;
        if !thirst.is_finite() {
            return Err(SimulationError::ThirstOverflow);
        }
        self.thirst = thirst.max(-100.0);
        let recovery_per_hour =
            if active.is_some_and(|action| action.action == CitizenAction::Sleep) {
                SLEEP_RECOVERY * 3_600_000.0 / SLEEP_DURATION_MS as f64
            } else {
                0.0
            };
        let hunger = self.hunger + self.hunger_per_hour * elapsed_hours - nourishment;
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Universe {
    current_time_ms: u64,
    market: Market,
    map: Map,
    agents: HashMap<AgentId, Agent>,
    citizen_histories: HashMap<AgentId, history::CitizenHistory>,
    household_histories: HashMap<AgentId, household_history::HouseholdHistory>,
    guild_histories: HashMap<AgentId, guild_history::GuildHistory>,
    storage: Storage,
    households: HashMap<AgentId, households::Household>,
    guilds: HashMap<AgentId, guilds::Guild>,
    workshop_fees: HashMap<(AgentId, AgentId), guilds::WorkshopFeeAccount>,
    town_treasury: Coins,
    taxation: taxation::Taxation,
}

impl Default for Universe {
    fn default() -> Self {
        Self {
            current_time_ms: 0,
            market: Market::default(),
            map: Map::random(),
            agents: HashMap::new(),
            citizen_histories: HashMap::new(),
            household_histories: HashMap::new(),
            guild_histories: HashMap::new(),
            storage: Storage::default(),
            households: HashMap::new(),
            guilds: HashMap::new(),
            workshop_fees: HashMap::new(),
            town_treasury: 0,
            taxation: taxation::Taxation::default(),
        }
    }
}

impl Universe {
    /// Creates an empty universe at the given time without advancing the simulation.
    pub fn starting_at(current_time_ms: u64) -> Self {
        Self {
            current_time_ms,
            market: Market::starting_at(current_time_ms),
            ..Self::default()
        }
    }

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
        universe.refresh_market();
        universe
    }

    pub fn caravan_policy(&self) -> marketplace::CaravanPolicy {
        self.market.caravan_policy()
    }

    pub fn with_caravan_policy(
        &self,
        policy: marketplace::CaravanPolicy,
    ) -> Result<Self, SimulationError> {
        policy.validate()?;
        let mut universe = self.clone();
        universe.market.set_caravan_policy(policy)?;
        universe.refresh_market();
        Ok(universe)
    }

    pub fn town_stock(&self, good: Good) -> Quantity {
        self.town_stock_total(good).min(u128::from(Quantity::MAX)) as Quantity
    }

    fn town_stock_total(&self, good: Good) -> u128 {
        let carried: u128 = self
            .agents
            .values()
            .map(|agent| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                u128::from(citizen.units(good)) + u128::from(citizen.town_carried_units(good))
            })
            .sum();
        let stored: u128 = self
            .storage
            .stock()
            .iter()
            .filter(|(key, _)| key.good == good)
            .map(|(_, units)| u128::from(*units))
            .sum();
        let listed: u128 = self
            .market
            .orders()
            .filter(|order| order.good == good && order.seller.citizen().is_some())
            .map(|order| u128::from(order.units))
            .sum();
        carried + stored + listed
    }

    pub fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }

    pub fn weekday(&self) -> calendar::Weekday {
        calendar::Weekday::at(self.current_time_ms)
    }

    pub fn town_treasury(&self) -> Coins {
        self.town_treasury
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Sets starting stock without taking it from carried inventory.
    pub fn with_stored_good(
        &self,
        place: PlaceId,
        owner: GoodsOwner,
        good: Good,
        units: Quantity,
    ) -> Result<Self, SimulationError> {
        self.map.place(place)?;
        match owner {
            GoodsOwner::Agent(id) if !self.agents.contains_key(&id) => {
                return Err(SimulationError::AgentNotFound);
            }
            GoodsOwner::Household(id) if !self.households.contains_key(&id) => {
                return Err(SimulationError::AgentNotFound);
            }
            _ => {}
        }
        let mut universe = self.clone();
        universe.storage.set_units(
            StockKey { place, owner, good },
            units,
            &universe.map,
            &universe.market,
        )?;
        universe.refresh_market();
        universe.refresh_household_history(universe.current_time_ms, false)?;
        universe.refresh_guild_history(universe.current_time_ms, false);
        if let GoodsOwner::Agent(id) = owner {
            let AgentKind::Citizen(citizen) = &universe.agents[&id].kind;
            citizen.wealth()?;
        }
        Ok(universe)
    }

    pub fn deposit_goods(
        &self,
        id: AgentId,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> Result<Self, SimulationError> {
        self.transfer_storage(id, place, good, units, true)
    }

    pub fn withdraw_goods(
        &self,
        id: AgentId,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> Result<Self, SimulationError> {
        self.transfer_storage(id, place, good, units, false)
    }

    fn transfer_storage(
        &self,
        id: AgentId,
        place: PlaceId,
        good: Good,
        units: Quantity,
        deposit: bool,
    ) -> Result<Self, SimulationError> {
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        let location = citizen.accessible_place(place)?;
        if matches!(
            citizen.active_action.map(|active| active.action),
            Some(CitizenAction::Travel(_))
        ) {
            return Err(SimulationError::CitizenBusy);
        }
        if citizen.position.distance(location.position) > 0.001 {
            return Err(SimulationError::WrongLocation);
        }
        let owner = citizen.goods_owner();
        let allowed = citizen.accessible_storage_objects(place);
        let stored = self.storage.units(place, owner, good);
        let carried = citizen.units(good);
        let reserved = match citizen.active_action.map(|active| active.action) {
            Some(CitizenAction::Produce(recipe) | CitizenAction::ProduceAt { recipe, .. })
                if recipe != production::Recipe::BrewBeer =>
            {
                recipe
                    .inputs()
                    .iter()
                    .filter(|(input, _)| *input == good)
                    .map(|(_, units)| *units)
                    .sum::<Quantity>()
                    .saturating_sub(citizen.active_action.unwrap().stored_inputs.units(good))
            }
            Some(CitizenAction::EquipClothing) if good == Good::FlaxGarment => 1,
            _ => 0,
        };
        if deposit && units > carried.saturating_sub(reserved) {
            return Err(SimulationError::MissingInputs);
        }
        let (next_carried, next_stored) = if deposit {
            (
                carried
                    .checked_sub(units)
                    .ok_or(SimulationError::MissingInputs)?,
                stored
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?,
            )
        } else {
            (
                carried
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?,
                stored
                    .checked_sub(units)
                    .ok_or(SimulationError::MissingInputs)?,
            )
        };
        let physical = storage::StockKey { place, owner, good };
        let mut accessible = 0_u64;
        let mut accessible_tax = 0_u64;
        let mut accessible_work = 0_u64;
        for (key, quantity) in self
            .storage
            .contents()
            .iter()
            .filter(|(key, _)| key.stock == physical && allowed.contains(&key.object))
        {
            accessible += quantity;
            accessible_tax += self.storage.object_tax_reserved(*key);
            accessible_work += self.storage.object_work_reserved(*key);
        }
        let (source, tax_reserved, work_reserved) = if deposit {
            (carried, citizen.tax_reserved_units(good), reserved)
        } else {
            (accessible, accessible_tax, accessible_work)
        };
        if units > source.saturating_sub(work_reserved) {
            return Err(SimulationError::MissingInputs);
        }
        let movable_available = source
            .saturating_sub(tax_reserved)
            .saturating_sub(work_reserved);
        let moved_reserved = units.saturating_sub(movable_available);
        let mut universe = self.clone();
        let carried_reserved = citizen.tax_reserved_units(good);
        let stored_reserved = self.storage.reserved_units(place, owner, good);
        let stock_key = StockKey { place, owner, good };
        let carried_stock = taxation::ReservationStock::Carried(id, good);
        let stored_stock = taxation::ReservationStock::Stored(stock_key);
        if moved_reserved > 0 {
            let (from, to) = if deposit {
                (carried_stock, stored_stock)
            } else {
                (stored_stock, carried_stock)
            };
            universe.move_tax_reservations(from, to, moved_reserved)?;
        }
        let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
        citizen.tax_reserved.insert(
            good,
            if deposit {
                carried_reserved - moved_reserved
            } else {
                carried_reserved + moved_reserved
            },
        );
        let next_reserved = if deposit {
            stored_reserved + moved_reserved
        } else {
            stored_reserved - moved_reserved
        };
        if !deposit {
            universe
                .storage
                .set_reserved_in(stock_key, next_reserved, Some(&allowed))?;
        }
        universe.storage.set_units_in(
            stock_key,
            next_stored,
            &universe.map,
            &universe.market,
            Some(&allowed),
        )?;
        if deposit {
            universe
                .storage
                .set_reserved_in(stock_key, next_reserved, Some(&allowed))?;
        }
        citizen.inventory.insert(good, next_carried);
        citizen.production_acquisitions[good as usize] =
            citizen.production_acquisitions[good as usize].min(citizen.available_units(good));
        universe.refresh_market();
        universe.refresh_household_history(universe.current_time_ms, false)?;
        universe.refresh_guild_history(universe.current_time_ms, false);
        Ok(universe)
    }

    pub fn citizen_history(&self, id: AgentId) -> Option<&history::CitizenHistory> {
        self.citizen_histories.get(&id)
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
        if !citizen.guilds.is_empty() || !citizen.workshop_fees.is_empty() {
            return Err(SimulationError::InvalidOwnership);
        }
        if citizen.tax_reserved.values().any(|units| *units > 0) {
            return Err(SimulationError::ReservedCitizenImport);
        }
        let mut universe = self.clone();
        let mut citizen = citizen.with_prices(self.prices());
        if citizen.map != self.map {
            let (map, home) = universe.map.with_place(
                Location::Home,
                universe.map.next_position(Location::Home),
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
        citizen.storage = universe.storage.clone();
        citizen.market_time_ms = universe.current_time_ms;
        citizen.work_period = production::work_period(universe.current_time_ms);
        for (_, agent) in universe.agents.iter_mut() {
            let AgentKind::Citizen(existing) = &mut agent.kind;
            existing.map = universe.map.clone();
        }
        let mut history = history::CitizenHistory::new(universe.current_time_ms, &citizen);
        history.start_action(&citizen)?;
        universe.citizen_histories.insert(id, history);
        universe.agents.insert(
            id,
            Agent {
                name,
                kind: AgentKind::Citizen(citizen),
            },
        );
        universe.register_action_request(id)?;
        universe.refresh_tax_rates();
        universe.refresh_market();
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
            self.map.next_position(kind),
            Some(owner),
            format!("{}'s {}", agent.name, kind.name()),
        )?;
        let mut universe = self.clone();
        universe.map = map;
        for (_, agent) in universe.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.map = universe.map.clone();
        }
        universe.refresh_tax_rates();
        Ok((universe, id))
    }

    pub fn market(&self) -> &Market {
        &self.market
    }

    pub fn advance(&self, elapsed_ms: u64) -> Result<Self, SimulationError> {
        Ok(self
            .advance_internal(elapsed_ms, None, &mut || false)?
            .unwrap())
    }

    /// Returns None when interrupted, leaving the source snapshot unchanged.
    pub fn advance_with_planner(
        &self,
        runtime: &mut PlanningRuntime,
        elapsed_ms: u64,
        mut should_cancel: impl FnMut() -> bool,
    ) -> Result<Option<Self>, SimulationError> {
        let result = self.advance_internal(elapsed_ms, Some(&mut *runtime), &mut should_cancel);
        if let Ok(Some(universe)) = &result {
            runtime.finish_advance(self, universe.current_time_ms());
        }
        if !matches!(result, Ok(Some(_))) {
            runtime.abort_advance();
        }
        result
    }

    fn advance_internal(
        &self,
        elapsed_ms: u64,
        mut runtime: Option<&mut PlanningRuntime>,
        should_cancel: &mut impl FnMut() -> bool,
    ) -> Result<Option<Self>, SimulationError> {
        if should_cancel() {
            return Ok(None);
        }
        let end = self
            .current_time_ms
            .checked_add(elapsed_ms)
            .ok_or(SimulationError::TimeOverflow)?;
        let mut universe = self.clone();
        if elapsed_ms == 0
            && let Some(runtime) = runtime.as_deref_mut()
        {
            let mut ids: Vec<_> = universe.agents.keys().copied().collect();
            ids.sort_by_key(|id| id.0);
            for id in ids {
                let AgentKind::Citizen(citizen) = &universe.agents[&id].kind;
                runtime.planning_event(citizen, universe.current_time_ms)?;
            }
        }
        while universe.current_time_ms < end {
            if should_cancel() {
                return Ok(None);
            }
            let until_update = marketplace::until_update(universe.current_time_ms);
            let next_week = calendar::next_weekly_settlement(universe.current_time_ms).ok();
            let until_week = next_week.map_or(end - universe.current_time_ms, |time| {
                time - universe.current_time_ms
            });
            let mut ids: Vec<_> = universe.agents.keys().copied().collect();
            ids.sort_by_key(|id| id.0);
            let mut planning_boundary = end - universe.current_time_ms;
            if let Some(runtime) = runtime.as_deref_mut() {
                for id in &ids {
                    let AgentKind::Citizen(citizen) = &universe.agents[id].kind;
                    if let Some(event) =
                        runtime.planning_event(citizen, universe.current_time_ms)?
                    {
                        planning_boundary = planning_boundary.min(event);
                    }
                }
            }
            let boundary = ids
                .iter()
                .filter_map(|id| {
                    let AgentKind::Citizen(citizen) = &universe.agents[id].kind;
                    citizen.active_action().map(|a| a.next_boundary_ms())
                })
                .min()
                .unwrap_or(end - universe.current_time_ms);
            let step = (end - universe.current_time_ms)
                .min(until_update)
                .min(until_week)
                .min(boundary)
                .min(planning_boundary);
            let finishing_time = universe.current_time_ms + step;
            let trades_before = universe.market.trades().len();
            let taxes_before = universe.taxation.history.len();
            let histories_before = universe.citizen_histories.clone();
            let agents_before = universe.agents.clone();
            for id in &ids {
                let agent_trades_before = universe.market.trades().len();
                let agent = universe.agents.get_mut(id).unwrap();
                let AgentKind::Citizen(citizen) = &mut agent.kind;
                if let Some(account) = citizen.household_id() {
                    citizen.coins = universe.households[&account].coins;
                }
                citizen.map = universe.map.clone();
                citizen.market = universe.market.clone();
                citizen.market.set_time(universe.current_time_ms);
                citizen.storage = universe.storage.clone();
                citizen.prices = universe.market.prices;
                citizen.market_time_ms = universe.current_time_ms;
                citizen.workshop_fees = universe
                    .workshop_fees
                    .iter()
                    .filter_map(|((payer, payee), fee)| {
                        (*payer == citizen.economic_id()).then_some((*payee, *fee))
                    })
                    .collect();
                let action_before = citizen.clone();
                let execution = citizen.active_plan.take();
                let (updated, payments, completion_failed) =
                    citizen.advance_action_with_settlement(step, false, true)?;
                let lost_shared_input = completion_failed
                    || action_before.active_action().is_some_and(|active| {
                        active.remaining_ms() == step
                            && match active.action() {
                                CitizenAction::CollectStored { place, list } => {
                                    list.items().any(|(good, units)| {
                                        action_before.stored_available_units(place, good) < units
                                    })
                                }
                                CitizenAction::DepositStored { list, .. } => {
                                    list.items().any(|(good, units)| {
                                        action_before.carried_available_units(good) < units
                                    })
                                }
                                CitizenAction::CollectBeer { brewery, slot } => action_before
                                    .beer_batch(brewery, slot)
                                    .is_ok_and(|batch| batch.is_none()),
                                CitizenAction::EquipClothing => {
                                    action_before.available_units(Good::FlaxGarment) == 0
                                }
                                _ => false,
                            }
                    });
                universe.citizen_histories.get_mut(id).unwrap().advance(
                    match &agents_before[id].kind {
                        AgentKind::Citizen(before) => before,
                    },
                    &action_before,
                    &updated,
                    step,
                    finishing_time,
                    !completion_failed,
                )?;
                *citizen = updated;
                if let Some(mut execution) = execution {
                    execution.record_elapsed(step)?;
                    if lost_shared_input {
                        execution.abandon_committed_goal();
                    }
                    citizen.active_plan = Some(execution);
                }
                universe.map = citizen.map.clone();
                universe.storage = citizen.storage.clone();
                if let Some(account) = citizen.household_id() {
                    universe.households.get_mut(&account).unwrap().coins = citizen.coins;
                }
                universe.market = citizen.market.clone();
                let payer = citizen.economic_id();
                for (payee, fee) in &citizen.workshop_fees {
                    universe.workshop_fees.insert((payer, *payee), *fee);
                }
                if let Some(history) = universe.household_histories.get_mut(&payer) {
                    let paid = payments
                        .iter()
                        .filter(|(payee, _)| universe.guilds.contains_key(payee))
                        .try_fold(0i64, |total, (_, paid)| total.checked_add(*paid))
                        .ok_or(SimulationError::WealthOverflow)?;
                    history.current.workshop_fees_paid = history
                        .current
                        .workshop_fees_paid
                        .checked_add(paid)
                        .ok_or(SimulationError::WealthOverflow)?;
                }
                if lost_shared_input {
                    universe.cancel_maintenance(*id)?;
                    if let Some(runtime) = runtime.as_deref_mut() {
                        runtime.cancel(*id);
                    }
                }
                for (seller, coins) in payments {
                    universe.record_guild_payment(seller, coins)?;
                    let balance = universe
                        .account_coins(seller)?
                        .checked_add(coins)
                        .ok_or(SimulationError::WealthOverflow)?;
                    universe.set_account_coins(seller, balance)?;
                }
                if let Some(active) = action_before.active_action()
                    && active.action().production_recipe().is_some()
                {
                    let place = active
                        .action()
                        .required_place(&action_before)
                        .ok_or(SimulationError::MissingProperty)?;
                    let old = &histories_before[id].current;
                    let new = &universe.citizen_histories[id].current;
                    let produced = std::array::from_fn(|i| new.produced[i] - old.produced[i]);
                    universe.record_guild_work(
                        *id,
                        place,
                        active.claimed_objects().next().and_then(|object| {
                            action_before.map.place(place).ok()?.object_owner(object)
                        }),
                        step.min(active.remaining_ms()),
                        produced,
                    )?;
                }
                let before = &action_before;
                if let Some(active) = before
                    .active_action()
                    .filter(|active| active.remaining_ms() == step)
                {
                    let AgentKind::Citizen(after) = &universe.agents[id].kind;
                    let changed = match active.action() {
                        CitizenAction::ListExcess
                        | CitizenAction::List(..)
                        | CitizenAction::ListAt { .. } => Good::ALL.into_iter().any(|good| {
                            after.market.listed_units(after.market_party(), good)
                                > before.market.listed_units(before.market_party(), good)
                        }),
                        CitizenAction::DepositStored { place, list } if place == after.home => {
                            list.items().any(|(good, _)| {
                                after.storage.units(place, after.goods_owner(), good)
                                    > before.storage.units(place, before.goods_owner(), good)
                            })
                        }
                        _ => false,
                    };
                    if changed {
                        let goal = if matches!(active.action(), CitizenAction::DepositStored { .. })
                        {
                            planning::goals::Effect::ReplenishReserves
                        } else {
                            planning::goals::Effect::ListExcess
                        };
                        universe.record_maintenance_work(*id, goal)?;
                    }
                }
                let before = &action_before;
                if let Some(active) = before.active_action()
                    && active.remaining_ms() == step
                    && !completion_failed
                    && let Some(recipe) = active.action().production_recipe()
                    && recipe != production::Recipe::BrewBeer
                {
                    let place = active
                        .action()
                        .required_place(before)
                        .ok_or(SimulationError::MissingProperty)?;
                    for &(good, _) in recipe.outputs() {
                        let inputs: Quantity = recipe
                            .inputs()
                            .iter()
                            .filter(|(input, _)| *input == good)
                            .map(|(_, units)| *units)
                            .sum();
                        let AgentKind::Citizen(after) = &universe.agents[id].kind;
                        let gross = after
                            .resource_units(good)?
                            .checked_sub(before.resource_units(good)? - inputs)
                            .ok_or(SimulationError::InventoryOverflow)?;
                        if universe.tax_production(*id, place, good, gross, finishing_time)?
                            && let Some(runtime) = runtime.as_deref_mut()
                        {
                            runtime.cancel(*id);
                        }
                    }
                }
                if let Some(active) = before.active_action()
                    && active.remaining_ms() == step
                    && !completion_failed
                    && let CitizenAction::Harvest { field, plot } = active.action()
                {
                    let crop = before
                        .crop(field, plot)?
                        .ok_or(SimulationError::MissingInputs)?;
                    if universe.tax_production(
                        *id,
                        field,
                        crop.kind.good(),
                        crop.expected_yield(finishing_time),
                        finishing_time,
                    )? && let Some(runtime) = runtime.as_deref_mut()
                    {
                        runtime.cancel(*id);
                    }
                }
                if let Some(active) = before.active_action()
                    && active.remaining_ms() == step
                    && !completion_failed
                    && let CitizenAction::CollectBeer { brewery, .. } = active.action()
                    && {
                        let AgentKind::Citizen(after) = &universe.agents[id].kind;
                        after.resource_units(Good::Beer)? > before.resource_units(Good::Beer)?
                    }
                    && universe.tax_production(*id, brewery, Good::Beer, 1000, finishing_time)?
                    && let Some(runtime) = runtime.as_deref_mut()
                {
                    runtime.cancel(*id);
                }
                let new_trades: Vec<_> = universe
                    .market
                    .trades()
                    .iter()
                    .skip(agent_trades_before)
                    .cloned()
                    .collect();
                let changed = universe.tax_sales(&new_trades, finishing_time)?;
                if let Some(runtime) = runtime.as_deref_mut() {
                    for payer in changed {
                        for worker in universe.account_workers(payer) {
                            runtime.cancel(worker);
                        }
                    }
                }
            }
            if next_week == Some(finishing_time) {
                let mut changed = universe.settle_tax_reservations(finishing_time)?;
                changed.extend(universe.tax_week(finishing_time)?);
                if let Some(runtime) = runtime.as_deref_mut() {
                    for payer in changed {
                        for worker in universe.account_workers(payer) {
                            runtime.cancel(worker);
                        }
                    }
                }
            }
            for trade in universe.market.trades().iter().skip(trades_before) {
                for id in [trade.buyer, trade.seller]
                    .into_iter()
                    .filter_map(|party| party.citizen())
                {
                    universe
                        .citizen_histories
                        .get_mut(&id)
                        .unwrap()
                        .trade(id, trade)?;
                }
            }
            universe.record_household_contributions(&histories_before)?;
            universe.record_household_events(trades_before, taxes_before)?;
            universe.record_guild_taxes(taxes_before)?;
            universe.settle_workshop_fees()?;
            universe.refresh_household_history(finishing_time, true)?;
            universe.refresh_guild_history(finishing_time, true);
            universe.current_time_ms = finishing_time;
            universe.market.set_time(finishing_time);
            for (_, agent) in universe.agents.iter_mut() {
                let AgentKind::Citizen(citizen) = &mut agent.kind;
                citizen.map = universe.map.clone();
            }
            for id in &ids {
                let AgentKind::Citizen(citizen) = &universe.agents[id].kind;
                universe
                    .citizen_histories
                    .get_mut(id)
                    .unwrap()
                    .close_day(finishing_time, citizen);
            }
            let histories_before = universe.citizen_histories.clone();
            if step == until_update {
                universe.refresh_market();
                universe.market.update(finishing_time)?;
                let trades_before = universe.market.trades().len();
                let taxes_before = universe.taxation.history.len();
                let limits = Good::ALL.map(|good| {
                    universe
                        .town_stock_total(good)
                        .saturating_sub(u128::from(
                            universe.caravan_policy().exports[good as usize].minimum_reserve,
                        ))
                        .min(u128::from(Quantity::MAX)) as Quantity
                });
                let payments = universe.market.caravans_with_limits(
                    universe.map.public_place(Location::Market),
                    finishing_time,
                    limits,
                )?;
                universe
                    .market
                    .bind_unplaced_orders(&universe.map, &universe.storage)?;
                let mut changed = Vec::new();
                for (seller, coins) in payments {
                    let balance = universe
                        .account_coins(seller)?
                        .checked_add(coins)
                        .ok_or(SimulationError::WealthOverflow)?;
                    universe.set_account_coins(seller, balance)?;
                    changed.push(seller);
                }
                let trades: Vec<_> = universe
                    .market
                    .trades()
                    .iter()
                    .skip(trades_before)
                    .cloned()
                    .collect();
                changed.extend(universe.tax_sales(&trades, finishing_time)?);
                for trade in &trades {
                    if let Some(seller) = trade.seller.citizen() {
                        universe
                            .citizen_histories
                            .get_mut(&seller)
                            .unwrap()
                            .trade(seller, trade)?;
                    }
                }
                if let Some(runtime) = runtime.as_deref_mut() {
                    for id in changed {
                        for worker in universe.account_workers(id) {
                            runtime.cancel(worker);
                        }
                    }
                }
                universe.record_household_events(trades_before, taxes_before)?;
                universe.record_guild_taxes(taxes_before)?;
                universe.settle_workshop_fees()?;
                universe.market.prune_history(finishing_time);
                universe.taxation.prune_history(finishing_time);
                universe.refresh_market();
            }
            universe.refresh_market();
            let needs_stock_refresh = ids.iter().any(|id| {
                let AgentKind::Citizen(citizen) = &universe.agents[id].kind;
                citizen.active_action.is_none()
            });
            if needs_stock_refresh || step == until_update {
                universe.refresh_workplace_supplies()?;
            }
            for id in ids {
                let completed_maintenance = {
                    let AgentKind::Citizen(citizen) = &universe.agents[&id].kind;
                    citizen
                        .active_plan
                        .as_ref()
                        .filter(|plan| {
                            citizen.active_action.is_none() && plan.finishes_committed_goal()
                        })
                        .and_then(|plan| plan.maintenance_goal())
                };
                if let Some(goal) = completed_maintenance {
                    universe.finish_maintenance(id, goal, finishing_time, true)?;
                    universe.refresh_market();
                }
                let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
                citizen.market = universe.market.clone();
                citizen.market.set_time(universe.current_time_ms);
                citizen.storage = universe.storage.clone();
                citizen.prices = universe.market.prices;
                citizen.market_time_ms = finishing_time;
                citizen.map = universe.map.clone();
                citizen.refresh_production_targets(false)?;
                if citizen.active_action.is_none()
                    && let Some(mut execution) = citizen.active_plan.take()
                {
                    execution.finish_action();
                    *citizen = if let Some(runtime) = runtime.as_deref_mut() {
                        let resumed = execution.resume_committed(citizen.clone())?;
                        if resumed.active_action().is_none() {
                            let Some(started) = runtime.at_boundary(
                                &resumed,
                                &mut execution,
                                finishing_time,
                                should_cancel,
                            )?
                            else {
                                return Ok(None);
                            };
                            started
                        } else {
                            resumed
                        }
                    } else {
                        execution.resume(citizen.clone())?
                    };
                    citizen.active_plan = Some(execution);
                    universe
                        .citizen_histories
                        .get_mut(&id)
                        .unwrap()
                        .start_action(citizen)?;
                }
                let AgentKind::Citizen(before) = &agents_before[&id].kind;
                let AgentKind::Citizen(after) = &universe.agents[&id].kind;
                universe.market = after.market.clone();
                if before.active_action.is_some()
                    || after.active_action.is_some()
                    || after.active_plan.is_some()
                {
                    universe.register_action_request(id)?;
                }
                universe.refresh_market();
            }
            universe.refresh_market();
            if let Some(runtime) = runtime.as_deref_mut() {
                let mut ids: Vec<_> = universe.agents.keys().copied().collect();
                ids.sort_by_key(|id| id.0);
                for id in ids {
                    let AgentKind::Citizen(citizen) = &universe.agents[&id].kind;
                    runtime.planning_event(citizen, universe.current_time_ms)?;
                }
            }
            universe.record_household_contributions(&histories_before)?;
        }
        universe.refresh_household_history(universe.current_time_ms, false)?;
        universe.refresh_guild_history(universe.current_time_ms, false);
        Ok(Some(universe))
    }

    fn refresh_market(&mut self) {
        let budgets: Vec<_> = self
            .agents
            .values()
            .filter_map(|agent| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                (citizen.household.is_none()).then_some((citizen.market_party(), citizen.coins))
            })
            .chain(self.households.values().map(|household| {
                (
                    marketplace::MarketParty::Household(household.id),
                    household.coins,
                )
            }))
            .collect();
        let carried: Vec<_> = self
            .agents
            .iter()
            .map(|(id, agent)| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                (
                    *id,
                    citizen.economic_id(),
                    std::array::from_fn::<_, { Good::COUNT }, _>(|index| {
                        citizen.units(Good::ALL[index])
                    }),
                    citizen.consumption_value(),
                    citizen.garment_condition().unwrap_or(0.0) <= 0.2,
                )
            })
            .collect();

        let mut incoming: std::collections::HashMap<
            (AgentId, AgentId, PlaceId),
            marketplace::ShoppingList,
        > = std::collections::HashMap::new();
        for (worker, agent) in &self.agents {
            let AgentKind::Citizen(citizen) = &agent.kind;
            if let Some(plan) = citizen.active_plan() {
                for action in plan
                    .plan()
                    .actions()
                    .iter()
                    .take(plan.committed_actions_end())
                    .skip(plan.action_index())
                {
                    if let CitizenAction::DepositStored { place, list } = *action {
                        let key = (citizen.economic_id(), *worker, place);
                        let old = incoming.get(&key).copied().unwrap_or_default();
                        incoming.insert(
                            key,
                            marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
                                (good, old.units(good).saturating_add(list.units(good)))
                            }))
                            .expect("bounded delivery quantities"),
                        );
                    }
                }
            }
        }
        self.market.refresh_affordability(&budgets);
        for (_, agent) in self.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            if let Some(context) = &mut citizen.household {
                let household = &self.households[&context.id];
                citizen.coins = household.coins;
                context.other_carried = [0; Good::COUNT];
                context.other_consumption_value = 0.0;
                context.other_clothing_replacements = 0;
                for (id, account, units, consuming, clothing_replacement) in &carried {
                    if *account == context.id && *id != citizen.id {
                        context.other_consumption_value += consuming;
                        context.other_clothing_replacements += u64::from(*clothing_replacement);
                        for (index, units) in units.iter().enumerate() {
                            context.other_carried[index] =
                                context.other_carried[index].saturating_add(*units);
                        }
                    }
                }
                citizen.maintenance = household.maintenance;
            }
            citizen.workshop_fees = self
                .workshop_fees
                .iter()
                .filter_map(|((payer, payee), fee)| {
                    (*payer == citizen.economic_id()).then_some((*payee, *fee))
                })
                .collect();
            citizen.incoming_supplies.clear();
            for ((account, worker, place), list) in &incoming {
                if *account == citizen.economic_id() && *worker != citizen.id {
                    let old = citizen
                        .incoming_supplies
                        .get(place)
                        .copied()
                        .unwrap_or_default();
                    citizen.incoming_supplies.insert(
                        *place,
                        marketplace::ShoppingList::new(
                            Good::ALL.into_iter().map(|good| {
                                (good, old.units(good).saturating_add(list.units(good)))
                            }),
                        )
                        .expect("bounded incoming quantities"),
                    );
                }
            }
            citizen.map = self.map.clone();
            citizen.market = self.market.clone();
            citizen.storage = self.storage.clone();
            citizen.prices = self.market.prices;
        }
    }

    fn register_action_request(&mut self, id: AgentId) -> Result<(), SimulationError> {
        let obsolete_claim = {
            let AgentKind::Citizen(citizen) = &self
                .agents
                .get(&id)
                .ok_or(SimulationError::AgentNotFound)?
                .kind;
            let goal = citizen
                .active_plan()
                .and_then(|plan| plan.maintenance_goal());
            let state = self
                .households
                .get(&citizen.economic_id())
                .map_or(&citizen.maintenance, |household| &household.maintenance);
            [
                (planning::goals::Effect::ListExcess, state.listing.claimant),
                (
                    planning::goals::Effect::ReplenishReserves,
                    state.replenishment.claimant,
                ),
            ]
            .into_iter()
            .any(|(task, claimant)| claimant == Some(id) && goal != Some(task))
        };
        if obsolete_claim {
            self.cancel_maintenance(id)?;
        }
        if self.claim_maintenance(id).is_err() {
            self.cancel_maintenance(id)?;
            self.refresh_market();
            let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&id).unwrap().kind;
            citizen.abandon_action()?;
            *citizen = citizen.start_planning()?;
            self.claim_maintenance(id)?;
        }
        let AgentKind::Citizen(snapshot) = &self
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        self.storage = snapshot.storage.clone();
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        self.map = citizen.map.clone();
        let action = citizen.active_action().map(|active| active.action());
        let action = if matches!(action, Some(CitizenAction::Travel(_))) {
            citizen.active_plan().and_then(|plan| {
                plan.travelling_purchase(citizen.active_action().unwrap().remaining_ms())
            })
        } else {
            action
        };
        let request = match action {
            Some(CitizenAction::Buy(list) | CitizenAction::BuyAt { list, .. }) => list,
            Some(CitizenAction::BuyFood(good)) => marketplace::ShoppingList::single(
                good,
                ((MEAL_NOURISHMENT - citizen.available_food_nutrition()).max(0.0)
                    / good.nutrition_per_unit().ok_or(SimulationError::NotFood)?)
                .ceil() as Quantity,
            ),
            _ => marketplace::ShoppingList::default(),
        };
        let request = citizen
            .active_plan()
            .map(|plan| {
                plan.shopping_request(
                    citizen
                        .active_action()
                        .map_or(0, |active| active.remaining_ms()),
                )
            })
            .transpose()?
            .flatten()
            .unwrap_or(request);
        let shortages = if citizen.production_targets.is_some() {
            citizen.purchase_shortages()?
        } else {
            marketplace::ShoppingList::default()
        };
        let blocked = citizen
            .active_plan()
            .and_then(|plan| plan.plan().decision())
            .map_or_else(marketplace::ShoppingList::default, |decision| {
                decision.blocked_requests
            });
        let request = marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
            (
                good,
                request
                    .units(good)
                    .max(shortages.units(good))
                    .max(blocked.units(good)),
            )
        }))?;
        let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&id).unwrap().kind;
        citizen.market_request = request;
        citizen.request_group = citizen
            .active_plan
            .as_ref()
            .and_then(|plan| plan.plan().goals().first())
            .and_then(|goal| match goal.goal {
                planning::goals::Effect::Production => citizen.designated_workplace,
                planning::goals::Effect::ReplenishReserves
                | planning::goals::Effect::ListExcess => Some(citizen.home),
                _ => None,
            });
        self.refresh_account_requests()
    }

    /// Replaces an actual purchase intention; unavailable stock is priced at the reference price.
    pub fn with_purchase_request(
        &self,
        id: AgentId,
        request: marketplace::ShoppingList,
    ) -> Result<Self, SimulationError> {
        if !self.agents.contains_key(&id) {
            return Err(SimulationError::AgentNotFound);
        }
        let mut universe = self.clone();
        let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
        citizen.market_request = request;
        citizen.request_group = None;
        universe.refresh_account_requests()?;
        universe.refresh_market();
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
        let histories_before = universe.citizen_histories.clone();
        let AgentKind::Citizen(citizen) = &updated.kind;
        universe
            .citizen_histories
            .get_mut(&id)
            .unwrap()
            .start_action(citizen)?;
        universe.market = citizen.market.clone();
        universe.agents.insert(id, updated);
        universe.record_household_contributions(&histories_before)?;
        universe.register_action_request(id)?;
        universe.refresh_market();
        universe.refresh_household_history(universe.current_time_ms, false)?;
        universe.refresh_guild_history(universe.current_time_ms, false);
        Ok(universe)
    }

    pub fn start_planning(&self, id: AgentId) -> Result<Self, SimulationError> {
        let mut prepared = self.clone();
        prepared.refresh_workplace_supplies()?;
        let mut agent = prepared
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .clone();
        let AgentKind::Citizen(citizen) = &mut agent.kind;
        citizen.refresh_production_targets(true)?;
        let updated = agent.start_planning()?;
        let mut universe = self.clone();
        let histories_before = universe.citizen_histories.clone();
        let AgentKind::Citizen(citizen) = &updated.kind;
        universe
            .citizen_histories
            .get_mut(&id)
            .unwrap()
            .start_action(citizen)?;
        universe.market = citizen.market.clone();
        universe.agents.insert(id, updated);
        universe.record_household_contributions(&histories_before)?;
        universe.register_action_request(id)?;
        universe.refresh_market();
        universe.refresh_household_history(universe.current_time_ms, false)?;
        universe.refresh_guild_history(universe.current_time_ms, false);
        Ok(universe)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationError {
    InvalidInventory,
    InvalidTaxRate,
    InvalidTaxRule,
    TaxRuleNotFound,
    InventoryOverflow,
    MissingSkill,
    MissingProperty,
    MissingInputs,
    NotFood,
    InvalidOwnership,
    PrivateProperty,
    PlaceNotFound,
    InvalidThirst,
    ThirstOverflow,
    InvalidHunger,
    InvalidHungerRate,
    InvalidTiredness,
    InvalidGarmentCondition,
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
    PlanningFailed,
    AgentNotFound,
    AgentAlreadyExists,
    ReservedCitizenImport,
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidTaxRate => "tax rates must be between zero and 100 percent",
            Self::InvalidTaxRule => {
                "tax rules require private locations, valid payers and combined socage at most 100 percent"
            }
            Self::TaxRuleNotFound => "active tax rule does not exist",
            Self::InvalidInventory => "goods use whole nonnegative units",
            Self::InventoryOverflow => "production exceeds representable inventory quantities",
            Self::MissingSkill => "production requires its skill",
            Self::MissingProperty => "production requires an owned property",
            Self::MissingInputs => "production inputs are unavailable",
            Self::NotFood => "this good is not edible",
            Self::InvalidOwnership => {
                "private places require an owner and public places cannot have one"
            }
            Self::PrivateProperty => "citizen cannot use another citizen's private property",
            Self::PlaceNotFound => "place does not exist",
            Self::InvalidThirst => "thirst must be finite",
            Self::ThirstOverflow => "advancing time would produce nonfinite thirst",
            Self::InvalidHunger => "hunger must be finite",
            Self::InvalidHungerRate => "hunger per hour must be finite and nonnegative",
            Self::InvalidGarmentCondition => "garment condition must be finite",
            Self::InvalidTiredness => "tiredness must be finite",
            Self::InvalidBerries => "berries use whole nonnegative units",
            Self::InvalidQuantity => "action quantities use whole nonnegative units",
            Self::InvalidPosition => "position must be finite and within the map",
            Self::WrongLocation => "action requires travel to its location first",
            Self::InvalidPrices => "prices must be finite and positive",
            Self::InvalidCoins => "coins must be valid integers",
            Self::WealthOverflow => "wealth exceeds the finite range",
            Self::TimeOverflow => "elapsed time exceeds the simulation clock's range",
            Self::HungerOverflow => "advancing time would produce nonfinite hunger",
            Self::TirednessOverflow => "advancing time would produce nonfinite tiredness",
            Self::BerriesOverflow => "foraging would overflow berry inventory",
            Self::WellbeingOverflow => "personal wellbeing exceeds the finite score range",
            Self::PlanningFailed => "planning worker failed",
            Self::CitizenBusy => "citizen is already performing an action",
            Self::AgentAlreadyExists => "citizen already exists in this universe",
            Self::ReservedCitizenImport => {
                "cannot import a citizen with tax reservations from another universe"
            }
            Self::AgentNotFound => "agent does not exist in this universe",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SimulationError {}

#[cfg(test)]
impl Citizen {
    pub(crate) fn with_home_good(
        &self,
        good: Good,
        units: Quantity,
    ) -> Result<Self, SimulationError> {
        let mut citizen = self.clone();
        citizen.storage.set_units(
            storage::StockKey {
                place: citizen.home,
                owner: citizen.goods_owner(),
                good,
            },
            units,
            &citizen.map,
            &citizen.market,
        )?;
        Ok(citizen)
    }

    pub(crate) fn reserve_home_good(&mut self, good: Good, units: Quantity) {
        self.storage
            .set_reserved(
                storage::StockKey {
                    place: self.home,
                    owner: self.goods_owner(),
                    good,
                },
                units,
            )
            .unwrap();
    }

    pub(crate) fn home_units(&self, good: Good) -> Quantity {
        self.storage.units(self.home, self.goods_owner(), good)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn missing_site_socage_rate_does_not_fall_back_to_recipe_rate() {
        let mut worker = Citizen::new(0.0).unwrap();
        let recipe = production::Recipe::ChopWood;
        let place = worker.production_place(recipe).unwrap();
        worker
            .socage_rates
            .insert((recipe, Good::Wood), taxation::TaxRate::new(5000).unwrap());
        assert_eq!(
            worker
                .production_socage_rate(recipe, Good::Wood)
                .basis_points(),
            5000
        );
        assert_eq!(
            worker
                .production_socage_rate_at(place, Good::Wood)
                .basis_points(),
            0
        );
    }

    fn baker_with_targets() -> Citizen {
        let worker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_coins(10000)
            .unwrap();
        let (universe, id) = Universe::with_map(Map::default())
            .with_citizen("Baker", worker)
            .unwrap();
        let (universe, _) = universe
            .with_property(id, production::Recipe::BakeRolls.location())
            .unwrap();
        let AgentKind::Citizen(worker) = &universe.agents()[&id].kind;
        let mut worker = worker.clone();
        let seller = AgentId(uuid::Uuid::new_v4());
        for good in [Good::Flour, Good::Wood, Good::Water, Good::Berries] {
            worker
                .market
                .list_at_quote(
                    seller,
                    worker.selling_place(),
                    good,
                    100000,
                    worker.prices.price(good).unwrap(),
                )
                .unwrap();
        }
        worker.production_targets =
            Some(production::ProductionTargets::calculate(&worker, 0).unwrap());
        worker
    }

    #[test]
    fn drinking_excludes_reserved_and_town_water_and_preserves_active_wealth() {
        let mut source = Citizen::new(0.0)
            .unwrap()
            .with_home_good(Good::Water, 300)
            .unwrap();
        source.reserve_home_good(Good::Water, 250);
        source.town_inventory.insert(Good::Water, 500);
        let wealth = source.wealth().unwrap();
        let started = source.start_action(CitizenAction::Drink).unwrap();
        assert_eq!(started.active_action().unwrap().drinking_units(), 50);
        assert_eq!(started.home_units(Good::Water), 250);
        assert!((started.wealth().unwrap() - wealth).abs() < 1e-9);
        let middle = started.advance(7500).unwrap();
        let water_value = source.prices.value(Good::Water, 25).unwrap();
        assert!((middle.wealth().unwrap() - wealth + water_value).abs() < 1e-9);
        let done = middle.advance(7500).unwrap();
        assert!(
            done.start_action(CitizenAction::Drink)
                .unwrap()
                .active_action()
                .is_none()
        );
        assert_eq!(done.town_inventory[&Good::Water], 500);
        assert_eq!(
            done.storage
                .reserved_units(done.home, done.goods_owner(), Good::Water),
            250
        );
        assert!(matches!(
            source.with_thirst(f64::NAN),
            Err(SimulationError::InvalidThirst)
        ));
    }

    #[test]
    fn water_reserves_credit_only_usable_home_stock_and_unconsumed_drinks() {
        let carried = Citizen::new(0.0)
            .unwrap()
            .with_good(Good::Water, 400)
            .unwrap();
        assert_eq!(carried.drinking_water_reserve(), 0.0);
        assert_eq!(carried.water_reserve_wellbeing(), 0.0);
        let source = carried
            .with_good(Good::Water, 0)
            .unwrap()
            .with_home_good(Good::Water, 400)
            .unwrap();
        assert_eq!(source.wealth().unwrap(), carried.wealth().unwrap());
        assert_eq!(source.water_reserve_wellbeing(), 10.0);
        assert_eq!(
            source
                .with_home_good(Good::Water, 1200)
                .unwrap()
                .water_reserve_wellbeing(),
            14.0
        );
        let drinking = source.start_action(CitizenAction::Drink).unwrap();
        assert_eq!(drinking.water_reserve_wellbeing(), 10.0);
        assert_eq!(
            drinking.advance(30_000).unwrap().water_reserve_wellbeing(),
            7.5
        );
        assert_eq!(source.home_units(Good::Water), 400);
        let mut taxed = source.clone();
        taxed.reserve_home_good(Good::Water, 400);
        assert_eq!(taxed.wealth().unwrap(), source.wealth().unwrap());
        assert_eq!(taxed.water_reserve_wellbeing(), 0.0);
        taxed.town_inventory.insert(Good::Water, 500);
        assert_eq!(taxed.water_reserve_wellbeing(), 0.0);
    }

    #[test]
    fn mixed_ingredient_targets_share_one_working_horizon_and_retain_the_basket() {
        let worker = baker_with_targets();
        let targets = worker.production_targets().unwrap();
        let work: u64 = targets
            .batches()
            .map(|(recipe, count)| recipe.duration_ms(&worker).unwrap() * count)
            .sum();
        assert!(work >= production::DAILY_CAPACITY_MS);
        assert!(work < production::DAILY_CAPACITY_MS + 90 * 60_000);
        assert!(targets.remaining_batches(production::Recipe::BakeRolls) > 0);
        assert!(targets.remaining_batches(production::Recipe::BakeBerryPie) > 0);
        let reserve = worker.reserved_goods().unwrap();
        assert!(reserve.units(Good::Flour) < 1200);
        let stocked = worker
            .with_good(Good::Flour, reserve.units(Good::Flour))
            .unwrap();
        assert_eq!(stocked.excess_goods().unwrap().units(Good::Flour), 0);
    }

    #[test]
    fn active_ingredients_remain_reserved_in_addition_to_future_work() {
        let worker = baker_with_targets()
            .with_good(Good::Flour, 100)
            .unwrap()
            .with_good(Good::Wood, 25)
            .unwrap()
            .with_good(Good::Water, 100)
            .unwrap()
            .with_good(Good::Berries, 100)
            .unwrap();
        let reserve = worker.reserved_goods().unwrap();
        for recipe in [
            production::Recipe::BakeRolls,
            production::Recipe::BakeBerryPie,
        ] {
            let active = worker.start_action(CitizenAction::Produce(recipe)).unwrap();
            assert_eq!(
                active.reserved_goods().unwrap().units(Good::Flour),
                reserve.units(Good::Flour)
            );
            assert_eq!(active.excess_goods().unwrap().units(Good::Flour), 0);
        }
    }

    #[test]
    fn restock_shortages_deduct_carried_and_withdrawable_listed_stock() {
        let worker = baker_with_targets().with_good(Good::Flour, 500).unwrap();
        let before = worker.purchase_shortages().unwrap();
        let listed = worker
            .start_action(CitizenAction::List(Good::Flour, 200))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(listed.units(Good::Flour), 300);
        assert_eq!(listed.withdrawable_units(Good::Flour), 200);
        assert_eq!(listed.purchase_shortages().unwrap(), before);
    }

    #[test]
    fn acquired_inputs_survive_changed_mix_replanning_and_repeated_excess_sales() {
        let mut worker = baker_with_targets()
            .with_good(Good::BreadRoll, 115)
            .unwrap()
            .with_home_good(Good::BreadRoll, 5)
            .unwrap()
            .with_good(Good::BerryPie, 16)
            .unwrap()
            .with_good(Good::Berries, 5520)
            .unwrap()
            .with_good(Good::Wood, 10000)
            .unwrap()
            .with_good(Good::Water, 10000)
            .unwrap();
        worker.tax_reserved.insert(Good::Berries, 5520);
        let mut policy = worker.market.caravan_policy();
        policy.exports[Good::BreadRoll as usize].allowed = false;
        policy.exports[Good::BerryPie as usize].allowed = false;
        worker.market.set_caravan_policy(policy).unwrap();
        worker.refresh_production_targets(true).unwrap();
        assert_eq!(
            worker.production_ingredient_reserve().unwrap()[Good::Flour as usize],
            0
        );
        let place = worker.selling_place();
        worker.position = worker.map.position(worker.home);
        let eating = worker.start_action(CitizenAction::Eat).unwrap();
        let mut worker = eating
            .advance(eating.active_action().unwrap().duration_ms())
            .unwrap();
        worker.position = worker.map.position(place);
        assert!(
            production::projected_recipes(&worker, production::DAILY_CAPACITY_MS)
                .unwrap()
                .contains(&production::Recipe::BakeRolls)
        );
        worker
            .market
            .list(AgentId(uuid::Uuid::new_v4()), place, Good::Flour, 1000)
            .unwrap();
        let buying = worker
            .start_action(CitizenAction::BuyAt {
                place,
                list: marketplace::ShoppingList::single(Good::Flour, 100),
            })
            .unwrap();
        let mut worker = buying.advance(TRADE_DURATION_MS).unwrap();
        assert_eq!(worker.production_acquisitions[Good::Flour as usize], 100);
        worker = worker.with_good(Good::Flour, 300).unwrap();
        for _ in 0..2 {
            worker = worker
                .start_action(CitizenAction::ListExcess)
                .unwrap()
                .advance(TRADE_DURATION_MS)
                .unwrap();
            assert_eq!(worker.units(Good::Flour), 100);
            assert_eq!(worker.production_acquisitions[Good::Flour as usize], 100);
        }
        worker.refresh_production_targets(true).unwrap();
        assert_eq!(worker.excess_goods().unwrap().units(Good::Flour), 0);
        worker = worker.with_good(Good::Wood, 25).unwrap();
        let workshop = worker
            .production_place(production::Recipe::BakeRolls)
            .unwrap();
        worker.position = worker.map.position(workshop);
        let working = worker
            .start_action(CitizenAction::Produce(production::Recipe::BakeRolls))
            .unwrap();
        let worker = working
            .advance(working.active_action().unwrap().duration_ms())
            .unwrap();
        assert_eq!(worker.production_acquisitions[Good::Flour as usize], 0);
        let worker = worker.with_good(Good::Flour, 100).unwrap();
        assert_eq!(worker.production_acquisitions[Good::Flour as usize], 0);
    }

    #[test]
    fn withdrawn_recipe_inputs_are_retained_and_explicit_removal_does_not_revive_them() {
        let mut worker = baker_with_targets();
        worker
            .production_targets
            .as_mut()
            .unwrap()
            .projected_order
            .clear();
        let place = worker.selling_place();
        worker.position = worker.map.position(place);
        worker
            .market
            .list(worker.id(), place, Good::Flour, 100)
            .unwrap();
        let worker = worker
            .start_action(CitizenAction::Withdraw(Good::Flour, 100))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(worker.excess_goods().unwrap().units(Good::Flour), 0);
        let worker = worker
            .start_action(CitizenAction::List(Good::Flour, 100))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(worker.production_acquisitions[Good::Flour as usize], 0);
        let worker = worker.with_good(Good::Flour, 100).unwrap();
        assert_eq!(worker.excess_goods().unwrap().units(Good::Flour), 100);
    }

    #[test]
    fn forage_at_cap_has_no_commercial_target_but_direct_gathering_remains_available() {
        let recipe = production::Recipe::Forage;
        let reserve = (production::PERSONAL_FOOD_RESERVE / BERRY_NUTRITION_PER_GRAM).ceil() as u64;
        let mut source = Citizen::new(0.0)
            .unwrap()
            .with_berries(reserve + 230)
            .unwrap();
        source.production_targets =
            Some(production::ProductionTargets::calculate(&source, 0).unwrap());
        assert_eq!(
            source
                .production_targets()
                .unwrap()
                .remaining_batches(recipe),
            0
        );
        let started = source.start_action(CitizenAction::Produce(recipe)).unwrap();
        let refreshed = production::ProductionTargets::calculate(&started, 0).unwrap();
        assert_eq!(refreshed.remaining_batches(recipe), 1);
        let predicted = started.advance_predicted(ACTION_DURATION_MS).unwrap();
        let actual = started.advance(ACTION_DURATION_MS).unwrap();
        for completed in [&predicted, &actual] {
            assert_eq!(
                completed
                    .production_targets()
                    .unwrap()
                    .remaining_batches(recipe),
                0
            );
            assert_eq!(completed.production_work_today_ms(), ACTION_DURATION_MS);
        }
        assert_eq!(predicted.berries_units(), reserve + 240);
        assert!((reserve + 235..=reserve + 245).contains(&actual.berries_units()));
    }

    #[test]
    fn prediction_uses_average_yield_without_advancing_the_random_stream() {
        let mut citizen = Citizen::new(0.0).unwrap();
        citizen.forage_rng = Xoshiro256PlusPlus::seed_from_u64(42);
        let started = citizen
            .start_action(CitizenAction::Produce(crate::production::Recipe::Forage))
            .unwrap();
        let predicted = started.advance_predicted(ACTION_DURATION_MS).unwrap();
        assert_eq!(predicted.berries_units(), 10);
        assert_eq!(predicted.forage_rng, citizen.forage_rng);
        let actual = started.advance(ACTION_DURATION_MS).unwrap();
        assert_ne!(actual.forage_rng, citizen.forage_rng);
        assert!((5..=15).contains(&actual.berries_units()));

        let mut total = 0_u64;
        for _ in 0..2000 {
            let next = citizen
                .start_action(CitizenAction::Produce(crate::production::Recipe::Forage))
                .unwrap()
                .advance(ACTION_DURATION_MS)
                .unwrap();
            let yield_units = next.berries_units() - citizen.berries_units();
            assert!((5..=15).contains(&yield_units));
            total += yield_units;
            citizen = next;
        }
        assert!((total as f64 / 2000.0 - 10.0).abs() < 0.2);
    }
    #[test]
    fn complete_meals_consider_mixed_food_and_quoted_value() {
        let source = Citizen::new(50.0)
            .unwrap()
            .with_prices(
                Prices::new(5.0)
                    .unwrap()
                    .with_price(Good::BreadRoll, 3.0)
                    .unwrap()
                    .with_price(Good::BerryPie, 25.0)
                    .unwrap(),
            )
            .with_home_good(Good::Berries, 154)
            .unwrap()
            .with_home_good(Good::BreadRoll, 5)
            .unwrap()
            .with_home_good(Good::BerryPie, 1)
            .unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.home_units(Good::BreadRoll), 4);
        assert_eq!(started.home_units(Good::BerryPie), 1);
        assert!(started.active_action().unwrap().meal_nutrition >= MEAL_NOURISHMENT);
        assert!(started.home_units(Good::Berries) < source.home_units(Good::Berries));
        let source = source.with_home_good(Good::Berries, 155).unwrap();
        let source = source.with_prices(
            source
                .prices()
                .with_price(Good::Berries, 200.0)
                .unwrap()
                .with_price(Good::BreadRoll, 6.0)
                .unwrap(),
        );
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.home_units(Good::BerryPie), 0);
        assert_eq!(started.home_units(Good::Berries), 155);
        assert_eq!(started.active_action().unwrap().duration_ms(), 125_000);
        let halfway = started.advance(62_500).unwrap();
        assert!((source.wealth().unwrap() - halfway.wealth().unwrap() - 12.5).abs() < 1e-10);
        let completed = halfway.advance(62_500).unwrap();
        assert!(
            (completed.hunger() - (50.0 + HUNGER_PER_HOUR * 125_000.0 / 3_600_000.0 - 60.0)).abs()
                < 1e-10
        );
    }
}
