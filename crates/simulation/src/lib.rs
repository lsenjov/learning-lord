use imbl::HashMap;
use marketplace::{Good, Market, Prices};
use rand::{RngExt, rngs::SmallRng};
use std::fmt;
use uuid::Uuid;

pub mod calendar;
pub mod storage;
pub mod taxation;
use storage::{GoodsOwner, StockKey, Storage};
pub mod locations;
pub mod marketplace;
use locations::{Location, Map, PlaceId, Position, WALK_MS_PER_METRE};
mod background;
pub mod planning;
pub use background::PlanningRuntime;
pub mod history;
pub mod production;

pub type Quantity = u64;
pub type Coins = i64;

pub const WEALTH_WELLBEING_PER_COIN: f64 = 0.1;
pub const FOOD_RESERVE_FULL_BONUS_NUTRITION: f64 = 100.0;
pub const FOOD_RESERVE_CAP_NUTRITION: f64 = 300.0;

pub const HUNGER_PER_HOUR: f64 = 100.0 / 24.0;
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
    EquipClothing,
    Wait,
    Sleep,
    BuyFood(Good),
    Produce(production::Recipe),
    ListExcess,
    List(Good, Quantity),
    Buy(marketplace::ShoppingList),
    BuyAt {
        place: PlaceId,
        list: marketplace::ShoppingList,
    },
    Withdraw(Good, Quantity),
    Travel(PlaceId),
}

impl CitizenAction {
    pub fn required_place(self, citizen: &Citizen) -> Option<PlaceId> {
        match self {
            Self::Sleep => Some(citizen.home),
            Self::Produce(recipe) => citizen.production_place(recipe).ok(),
            Self::BuyFood(_) | Self::Buy(..) => Some(citizen.map.public_place(Location::Market)),
            Self::BuyAt { place, .. } => Some(place),
            Self::ListExcess | Self::List(..) | Self::Withdraw(..) => Some(citizen.selling_place()),
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
    meal_units: [Quantity; Good::COUNT],
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
    Weaver,
    Tailor,
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
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Citizen {
    hunger: f64,
    garment_condition: Option<f64>,
    hunger_per_hour: f64,
    tiredness: f64,
    inventory: HashMap<Good, Quantity>,
    tax_reserved: HashMap<Good, Quantity>,
    town_inventory: HashMap<Good, Quantity>,
    socage_rates: HashMap<(production::Recipe, Good), taxation::TaxRate>,
    storage: Storage,
    id: AgentId,
    home: PlaceId,
    starting_role: Option<StartingRole>,
    skills: [f64; production::Skill::COUNT],
    production_targets: Option<production::ProductionTargets>,
    work_period: u64,
    work_ms: u64,
    coins: Coins,
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
            garment_condition: None,
            hunger_per_hour,
            tiredness: 0.0,
            inventory: Good::ALL.into_iter().map(|good| (good, 0)).collect(),
            tax_reserved: HashMap::new(),
            town_inventory: HashMap::new(),
            socage_rates: HashMap::new(),
            storage: Storage::default(),
            id,
            home,
            starting_role: None,
            skills: [0.0; production::Skill::COUNT],
            production_targets: None,
            work_period: 0,
            work_ms: 0,
            coins: 0,
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
            StartingRole::Weaver => production::Skill::Weaving,
            StartingRole::Tailor => production::Skill::Tailoring,
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
        self.units(good) - self.tax_reserved_units(good)
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
    pub fn with_good(&self, good: Good, units: Quantity) -> Result<Self, SimulationError> {
        if self.active_action.is_some() || self.active_plan.is_some() {
            return Err(SimulationError::CitizenBusy);
        }
        if units < self.tax_reserved_units(good) {
            return Err(SimulationError::MissingInputs);
        }
        let mut citizen = self.clone();
        citizen.inventory.insert(good, units);
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
        let mut wealth = self.coins as f64
            + self.prices.value(Good::FlaxGarment, 1).unwrap_or(0.0)
                * self.garment_condition.unwrap_or(0.0);
        for good in Good::ALL {
            let units = self
                .units(good)
                .checked_add(self.market.listed_units(self.id, good))
                .and_then(|units| {
                    units.checked_add(self.storage.owned_units(GoodsOwner::Agent(self.id), good))
                })
                .ok_or(SimulationError::WealthOverflow)?;
            wealth += self.prices.value(good, units).unwrap_or(0.0);
        }
        if let Some(active) = self.active_action {
            let remaining = active.remaining_ms as f64 / active.duration_ms as f64;
            for good in Good::FOOD {
                wealth += self
                    .prices
                    .value(good, active.meal_units[good as usize])
                    .unwrap_or(0.0)
                    * remaining;
            }
        }
        if !wealth.is_finite() {
            return Err(SimulationError::WealthOverflow);
        }
        Ok(wealth)
    }

    fn accessible_place(&self, id: PlaceId) -> Result<&locations::Place, SimulationError> {
        let place = self.map.place(id)?;
        if place.kind == Location::Home && place.owner.is_some_and(|owner| owner != self.id) {
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
                .map(|(index, units)| units * Good::ALL[index].eating_ms_per_unit().unwrap_or(0))
                .sum::<u64>(),
            CitizenAction::Produce(recipe) => {
                self.production_place(recipe)?;
                recipe.duration_ms(self)?
            }
            CitizenAction::Wait => ACTION_DURATION_MS,
            CitizenAction::Sleep => SLEEP_DURATION_MS,
            CitizenAction::EquipClothing => EQUIP_CLOTHING_DURATION_MS,
            CitizenAction::BuyFood(_)
            | CitizenAction::ListExcess
            | CitizenAction::List(..)
            | CitizenAction::Buy(..)
            | CitizenAction::BuyAt { .. }
            | CitizenAction::Withdraw(..) => TRADE_DURATION_MS,
        })
    }

    pub fn food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| self.units(good) as f64 * good.nutrition_per_unit().unwrap())
            .sum()
    }

    pub fn available_food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| self.available_units(good) as f64 * good.nutrition_per_unit().unwrap())
            .sum()
    }

    pub fn has_complete_meal(&self) -> bool {
        Good::FOOD.into_iter().any(|good| {
            self.available_units(good)
                >= (MEAL_NOURISHMENT / good.nutrition_per_unit().unwrap()).ceil() as Quantity
        })
    }

    fn meal(&self) -> ([Quantity; Good::COUNT], f64) {
        let complete = Good::FOOD
            .into_iter()
            .filter_map(|good| {
                let nutrition = good.nutrition_per_unit()?;
                let needed = (MEAL_NOURISHMENT / nutrition).ceil() as Quantity;
                if self.available_units(good) < needed {
                    return None;
                }
                let cost_per_nutrition =
                    self.prices.value(good, needed)? / (needed as f64 * nutrition);
                Some((good, needed, cost_per_nutrition))
            })
            .min_by(|a, b| {
                a.2.total_cmp(&b.2)
                    .then_with(|| (a.0 as usize).cmp(&(b.0 as usize)))
            });
        let mut units = [0; Good::COUNT];
        if let Some((good, needed, _)) = complete {
            units[good as usize] = needed;
            return (units, needed as f64 * good.nutrition_per_unit().unwrap());
        }
        let mut remaining = MEAL_NOURISHMENT;
        let mut total = 0.0;
        for good in Good::FOOD {
            let nutrition = good.nutrition_per_unit().unwrap();
            let portion = self
                .available_units(good)
                .min((remaining / nutrition).ceil() as Quantity);
            units[good as usize] = portion;
            total += portion as f64 * nutrition;
            remaining = (remaining - portion as f64 * nutrition).max(0.0);
        }
        (units, total)
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

    pub fn selling_place(&self) -> PlaceId {
        self.owned_properties()
            .filter(|place| place.kind != Location::Home && !place.kind.is_public())
            .map(|place| place.id)
            .min_by_key(|id| id.0)
            .unwrap_or_else(|| self.map.public_place(Location::Market))
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

    fn production_ingredient_reserve(&self) -> Result<[Quantity; Good::COUNT], SimulationError> {
        let active = self.active_action.and_then(|active| match active.action {
            CitizenAction::Produce(recipe) => Some(recipe),
            _ => None,
        });
        let mut reserve = [0_u64; Good::COUNT];
        if let Some(targets) = &self.production_targets {
            for (recipe, count) in targets.batches() {
                let future = count.saturating_sub(u64::from(active == Some(recipe)));
                for &(good, units) in recipe.inputs() {
                    reserve[good as usize] = reserve[good as usize].max(
                        units
                            .checked_mul(future)
                            .ok_or(SimulationError::InventoryOverflow)?,
                    );
                }
            }
        }
        if let Some(recipe) = active {
            for &(good, units) in recipe.inputs() {
                reserve[good as usize] = reserve[good as usize]
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?;
            }
        }
        Ok(reserve)
    }

    pub fn reserved_goods(&self) -> Result<marketplace::ShoppingList, SimulationError> {
        let mut reserve = self.production_ingredient_reserve()?;
        let mut nutrition = FOOD_RESERVE_CAP_NUTRITION;
        for good in Good::FOOD {
            let available = self
                .available_units(good)
                .saturating_sub(reserve[good as usize]);
            let kept =
                available.min((nutrition / good.nutrition_per_unit().unwrap()).ceil() as Quantity);
            reserve[good as usize] += kept;
            nutrition = (nutrition - kept as f64 * good.nutrition_per_unit().unwrap()).max(0.0);
        }
        marketplace::ShoppingList::new(
            Good::ALL
                .into_iter()
                .map(|good| (good, reserve[good as usize])),
        )
    }

    pub fn purchase_shortages(&self) -> Result<marketplace::ShoppingList, SimulationError> {
        let mut needed = self.production_ingredient_reserve()?;
        for good in Good::ALL {
            let held = self
                .available_units(good)
                .checked_add(self.market.listed_units(self.id, good))
                .ok_or(SimulationError::InventoryOverflow)?;
            needed[good as usize] = needed[good as usize].saturating_sub(held);
        }
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
        let mut citizen = self.clone();
        let duration_ms = self.action_duration_ms(action)?;
        if duration_ms == 0 {
            return Ok(citizen);
        }
        if let CitizenAction::Produce(recipe) = action
            && recipe
                .inputs()
                .iter()
                .any(|&(good, units)| self.available_units(good) < units)
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
        let (meal_units, meal_nutrition) = if action == CitizenAction::Eat {
            self.meal()
        } else {
            ([0; Good::COUNT], 0.0)
        };
        for good in Good::FOOD {
            citizen
                .inventory
                .insert(good, self.units(good) - meal_units[good as usize]);
        }
        citizen.active_action = Some(ActiveAction {
            action,
            remaining_ms: duration_ms,
            duration_ms,
            meal_nutrition,
            meal_units,
            travel_origin: self.position,
            travel_destination: if let CitizenAction::Travel(location) = action {
                Some(self.map.position(location))
            } else {
                None
            },
        });
        Ok(citizen)
    }

    pub fn food_reserve_wellbeing(&self) -> f64 {
        let nutrition = self.food_nutrition()
            + self.active_action.map_or(0.0, |active| {
                active.meal_nutrition * active.remaining_ms as f64 / active.duration_ms as f64
            });
        0.10 * nutrition.min(FOOD_RESERVE_FULL_BONUS_NUTRITION)
            + 0.02
                * (nutrition.min(FOOD_RESERVE_CAP_NUTRITION) - FOOD_RESERVE_FULL_BONUS_NUTRITION)
                    .max(0.0)
    }

    pub fn personal_wellbeing(&self) -> Result<f64, SimulationError> {
        let wellbeing = -self.clothing_need()
            - self.hunger.max(0.0)
            - 4.0 * (self.hunger - 100.0).max(0.0)
            - (-self.hunger - 100.0).max(0.0)
            - self.tiredness.max(0.0)
            + self.wealth()? * WEALTH_WELLBEING_PER_COIN
            + self.food_reserve_wellbeing();
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
    ) -> Result<(Self, Vec<(AgentId, Coins)>), SimulationError> {
        let mut citizen = self.clone();
        let mut payments = Vec::new();
        if elapsed_ms == 0 {
            return Ok((citizen, payments));
        }

        if let Some(mut active) = citizen.active_action {
            let action_elapsed_ms = elapsed_ms.min(active.remaining_ms);
            citizen.advance_needs(action_elapsed_ms, Some(active))?;
            if matches!(active.action, CitizenAction::Produce(_)) {
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
                match active.action {
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
                    CitizenAction::Produce(recipe) => {
                        citizen.complete_production(recipe, prediction)?
                    }
                    CitizenAction::EquipClothing => {
                        let remaining = citizen
                            .units(Good::FlaxGarment)
                            .checked_sub(1)
                            .ok_or(SimulationError::MissingInputs)?;
                        citizen.inventory.insert(Good::FlaxGarment, remaining);
                        citizen.garment_condition = Some(1.0);
                    }
                    CitizenAction::ListExcess => {
                        for (good, units) in citizen.excess_goods()?.items() {
                            citizen.market.list(
                                citizen.id,
                                citizen.selling_place(),
                                good,
                                units,
                            )?;
                            citizen.inventory.insert(good, citizen.units(good) - units);
                        }
                    }
                    CitizenAction::Buy(list) | CitizenAction::BuyAt { list, .. } => {
                        let place = active.action.required_place(&citizen).unwrap();
                        for (good, units) in list.items() {
                            payments.extend(citizen.complete_purchase(
                                place,
                                good,
                                units,
                                record_trades,
                                action_elapsed_ms,
                            )?);
                        }
                    }
                    CitizenAction::List(good, requested) => {
                        let units = requested.min(citizen.available_units(good));
                        citizen
                            .market
                            .list(citizen.id, citizen.selling_place(), good, units)?;
                        citizen.inventory.insert(good, citizen.units(good) - units);
                    }
                    CitizenAction::Withdraw(good, requested) => {
                        let units = citizen.market.withdraw(
                            citizen.id,
                            citizen.selling_place(),
                            good,
                            requested,
                        )?;
                        let stock = citizen
                            .units(good)
                            .checked_add(units)
                            .ok_or(SimulationError::WealthOverflow)?;
                        citizen.inventory.insert(good, stock);
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

    fn complete_production(
        &mut self,
        recipe: production::Recipe,
        prediction: bool,
    ) -> Result<(), SimulationError> {
        for &(good, units) in recipe.inputs() {
            let stock = self.units(good);
            if self.available_units(good) < units {
                return Err(SimulationError::MissingInputs);
            }
            self.inventory.insert(good, stock - units);
        }
        for &(good, units) in recipe.outputs() {
            let units = if recipe == production::Recipe::Forage && !prediction {
                self.forage_rng
                    .random_range(FORAGE_MIN_GRAMS..=FORAGE_MAX_GRAMS)
            } else {
                units
            };
            let stock = self.units(good);
            let next = stock
                .checked_add(units)
                .ok_or(SimulationError::InventoryOverflow)?;
            self.inventory.insert(good, next);
        }
        if let Some(targets) = &mut self.production_targets {
            targets.complete(recipe);
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
        place: PlaceId,
        good: Good,
        units: Quantity,
        record_trades: bool,
        elapsed_ms: u64,
    ) -> Result<Vec<(AgentId, Coins)>, SimulationError> {
        let purchase = self.market.purchase(
            self.id,
            place,
            (good, units),
            self.coins,
            self.market_time_ms
                .checked_add(elapsed_ms)
                .ok_or(SimulationError::TimeOverflow)?,
            record_trades,
        )?;
        let stock = self
            .units(good)
            .checked_add(purchase.units)
            .ok_or(SimulationError::WealthOverflow)?;
        self.inventory.insert(good, stock);
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
        self.garment_condition = self.garment_condition.map(|condition| {
            (condition - elapsed_ms as f64 / GARMENT_LIFETIME_MS as f64).clamp(0.0, 1.0)
        });
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
    citizen_histories: HashMap<AgentId, history::CitizenHistory>,
    storage: Storage,
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
            storage: Storage::default(),
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
        if let GoodsOwner::Agent(id) = owner
            && !self.agents.contains_key(&id)
        {
            return Err(SimulationError::AgentNotFound);
        }
        let mut universe = self.clone();
        universe
            .storage
            .set_units(StockKey { place, owner, good }, units)?;
        universe.refresh_market();
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
        let owner = GoodsOwner::Agent(id);
        let stored = self.storage.units(place, owner, good);
        let carried = citizen.units(good);
        let reserved = match citizen.active_action.map(|active| active.action) {
            Some(CitizenAction::Produce(recipe)) => recipe
                .inputs()
                .iter()
                .filter(|(input, _)| *input == good)
                .map(|(_, units)| *units)
                .sum(),
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
        let tax_reserved = if deposit {
            citizen.tax_reserved_units(good)
        } else {
            self.storage.reserved_units(place, owner, good)
        };
        let source = if deposit { carried } else { stored };
        let movable_available =
            (source - tax_reserved).saturating_sub(if deposit { reserved } else { 0 });
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
        universe.storage.set_reserved(
            stock_key,
            if deposit {
                stored_reserved + moved_reserved
            } else {
                stored_reserved - moved_reserved
            },
        );
        citizen.inventory.insert(good, next_carried);
        universe
            .storage
            .set_units(StockKey { place, owner, good }, next_stored)?;
        universe.refresh_market();
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
        if citizen.tax_reserved.values().any(|units| *units > 0) {
            return Err(SimulationError::ReservedCitizenImport);
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
                    citizen.active_action().map(|a| a.remaining_ms())
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
            let agents_before = universe.agents.clone();
            for id in &ids {
                let agent_trades_before = universe.market.trades().len();
                let agent = universe.agents.get_mut(id).unwrap();
                let AgentKind::Citizen(citizen) = &mut agent.kind;
                citizen.market = universe.market.clone();
                citizen.prices = universe.market.prices;
                citizen.market_time_ms = universe.current_time_ms;
                let execution = citizen.active_plan.take();
                let (updated, payments) =
                    citizen.advance_action_with_settlement(step, false, true)?;
                universe.citizen_histories.get_mut(id).unwrap().advance(
                    match &agents_before[id].kind {
                        AgentKind::Citizen(before) => before,
                    },
                    &updated,
                    step,
                    finishing_time,
                )?;
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
                    seller.coins = seller
                        .coins
                        .checked_add(coins)
                        .ok_or(SimulationError::WealthOverflow)?;
                }
                let AgentKind::Citizen(before) = &agents_before[id].kind;
                if let Some(active) = before.active_action()
                    && active.remaining_ms() == step
                    && let CitizenAction::Produce(recipe) = active.action()
                {
                    let place = before.production_place(recipe)?;
                    for &(good, _) in recipe.outputs() {
                        let inputs: Quantity = recipe
                            .inputs()
                            .iter()
                            .filter(|(input, _)| *input == good)
                            .map(|(_, units)| *units)
                            .sum();
                        let AgentKind::Citizen(after) = &universe.agents[id].kind;
                        let gross = after
                            .units(good)
                            .checked_sub(before.units(good) - inputs)
                            .ok_or(SimulationError::InventoryOverflow)?;
                        if universe.tax_production(*id, place, good, gross, finishing_time)?
                            && let Some(runtime) = runtime.as_deref_mut()
                        {
                            runtime.cancel(*id);
                        }
                    }
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
                        runtime.cancel(payer);
                    }
                }
            }
            if next_week == Some(finishing_time) {
                let mut changed = universe.settle_tax_reservations(finishing_time)?;
                changed.extend(universe.tax_week(finishing_time)?);
                if let Some(runtime) = runtime.as_deref_mut() {
                    for payer in changed {
                        runtime.cancel(payer);
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
            universe.current_time_ms = finishing_time;
            for id in &ids {
                let AgentKind::Citizen(citizen) = &universe.agents[id].kind;
                universe
                    .citizen_histories
                    .get_mut(id)
                    .unwrap()
                    .close_day(finishing_time, citizen);
            }
            if step == until_update {
                universe.refresh_market();
                universe.market.update(finishing_time)?;
                let trades_before = universe.market.trades().len();
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
                let mut changed = Vec::new();
                for (seller, coins) in payments {
                    let AgentKind::Citizen(citizen) = &mut universe
                        .agents
                        .get_mut(&seller)
                        .ok_or(SimulationError::AgentNotFound)?
                        .kind;
                    citizen.coins = citizen
                        .coins
                        .checked_add(coins)
                        .ok_or(SimulationError::WealthOverflow)?;
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
                        runtime.cancel(id);
                    }
                }
                universe.refresh_market();
            }
            let mut new_actions = Vec::new();
            for id in ids {
                let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
                citizen.market = universe.market.clone();
                citizen.storage = universe.storage.clone();
                citizen.prices = universe.market.prices;
                citizen.market_time_ms = finishing_time;
                citizen.refresh_production_targets(false)?;
                if citizen.production_targets.is_some() {
                    new_actions.push(id);
                }
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
                    new_actions.push(id);
                }
            }
            for id in new_actions {
                universe.register_action_request(id)?;
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
        }
        Ok(Some(universe))
    }

    fn refresh_market(&mut self) {
        let budgets: Vec<_> = self
            .agents
            .iter()
            .map(|(id, agent)| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                (*id, citizen.coins)
            })
            .collect();
        self.market.refresh_affordability(&budgets);
        for (_, agent) in self.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.market = self.market.clone();
            citizen.storage = self.storage.clone();
            citizen.prices = self.market.prices;
        }
    }

    fn register_action_request(&mut self, id: AgentId) -> Result<(), SimulationError> {
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
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
        let request = if citizen.production_targets.is_some() {
            let shortages = citizen.purchase_shortages()?;
            marketplace::ShoppingList::new(
                Good::ALL
                    .into_iter()
                    .map(|good| (good, shortages.units(good).max(request.units(good)))),
            )?
        } else {
            request
        };
        self.market.set_request(id, request)
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
        universe.market.set_request(id, request)?;
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
        let AgentKind::Citizen(citizen) = &updated.kind;
        universe
            .citizen_histories
            .get_mut(&id)
            .unwrap()
            .start_action(citizen)?;
        universe.agents.insert(id, updated);
        universe.register_action_request(id)?;
        universe.refresh_market();
        Ok(universe)
    }

    pub fn start_planning(&self, id: AgentId) -> Result<Self, SimulationError> {
        let mut agent = self
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .clone();
        let AgentKind::Citizen(citizen) = &mut agent.kind;
        citizen.refresh_production_targets(false)?;
        let updated = agent.start_planning()?;
        let mut universe = self.clone();
        let AgentKind::Citizen(citizen) = &updated.kind;
        universe
            .citizen_histories
            .get_mut(&id)
            .unwrap()
            .start_action(citizen)?;
        universe.agents.insert(id, updated);
        universe.register_action_request(id)?;
        universe.refresh_market();
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
mod tests {
    use super::*;
    use rand::SeedableRng;

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
            .with_property(id, production::Recipe::BakeBread.location())
            .unwrap();
        let AgentKind::Citizen(worker) = &universe.agents()[&id].kind;
        let mut worker = worker.clone();
        worker.production_targets =
            Some(production::ProductionTargets::calculate(&worker, 0).unwrap());
        worker
    }

    #[test]
    fn alternative_ingredient_reserves_use_each_goods_maximum_requirement() {
        let worker = baker_with_targets();
        let targets = worker.production_targets().unwrap();
        assert_eq!(targets.remaining_batches(production::Recipe::BakeBread), 12);
        assert_eq!(
            targets.remaining_batches(production::Recipe::BakeBerryPie),
            8
        );
        let reserve = worker.reserved_goods().unwrap();
        assert_eq!(reserve.units(Good::Flour), 1200);
        assert_eq!(reserve.units(Good::Wood), 300);
        assert_eq!(reserve.units(Good::Water), 1200);
        assert_eq!(reserve.units(Good::Berries), 800);
        assert_eq!(worker.purchase_shortages().unwrap(), reserve);
    }

    #[test]
    fn active_ingredients_are_added_after_maximizing_future_alternatives() {
        let worker = baker_with_targets()
            .with_good(Good::Flour, 100)
            .unwrap()
            .with_good(Good::Wood, 25)
            .unwrap()
            .with_good(Good::Water, 100)
            .unwrap()
            .with_good(Good::Berries, 100)
            .unwrap();
        let bread = worker
            .start_action(CitizenAction::Produce(production::Recipe::BakeBread))
            .unwrap();
        assert_eq!(bread.reserved_goods().unwrap().units(Good::Flour), 1200);
        let pie = worker
            .start_action(CitizenAction::Produce(production::Recipe::BakeBerryPie))
            .unwrap();
        let reserve = pie.reserved_goods().unwrap();
        assert_eq!(reserve.units(Good::Flour), 1300);
        assert_eq!(reserve.units(Good::Wood), 325);
        assert_eq!(reserve.units(Good::Water), 1250);
        assert_eq!(reserve.units(Good::Berries), 800);
        let shortages = pie.purchase_shortages().unwrap();
        assert_eq!(shortages.units(Good::Flour), 1200);
        assert_eq!(shortages.units(Good::Berries), 700);
    }

    #[test]
    fn shortages_deduct_carried_and_listed_stock_from_alternative_reserves() {
        let worker = baker_with_targets().with_good(Good::Flour, 500).unwrap();
        let worker = worker
            .start_action(CitizenAction::List(Good::Flour, 200))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(worker.units(Good::Flour), 300);
        assert_eq!(worker.market().listed_units(worker.id(), Good::Flour), 200);
        assert_eq!(worker.reserved_goods().unwrap().units(Good::Flour), 1200);
        assert_eq!(worker.purchase_shortages().unwrap().units(Good::Flour), 700);
    }

    #[test]
    fn forage_targets_account_for_active_and_completed_batches() {
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
            1
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
        citizen.forage_rng = SmallRng::seed_from_u64(42);
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
    fn complete_meals_choose_one_available_food_by_quoted_value_per_nutrition() {
        let source = Citizen::new(50.0)
            .unwrap()
            .with_prices(
                Prices::new(5.0)
                    .unwrap()
                    .with_price(Good::Bread, 15.0)
                    .unwrap()
                    .with_price(Good::BerryPie, 25.0)
                    .unwrap(),
            )
            .with_berries(154)
            .unwrap()
            .with_good(Good::Bread, 1)
            .unwrap()
            .with_good(Good::BerryPie, 1)
            .unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.units(Good::Bread), 0);
        assert_eq!(started.units(Good::BerryPie), 1);
        assert_eq!(started.berries_units(), 154);
        assert_eq!(started.active_action().unwrap().duration_ms(), 100_000);
        let source = source.with_berries(155).unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.berries_units(), 0);
        assert_eq!(started.units(Good::Bread), 1);
        let source = source.with_prices(
            source
                .prices()
                .with_price(Good::Berries, 200.0)
                .unwrap()
                .with_price(Good::Bread, 30.0)
                .unwrap(),
        );
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.units(Good::BerryPie), 0);
        assert_eq!(started.berries_units(), 155);
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
