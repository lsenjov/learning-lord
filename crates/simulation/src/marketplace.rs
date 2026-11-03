use crate::{AgentId, Coins, PlaceId, Quantity, SimulationError};
use imbl::{HashMap, HashSet, Vector};

pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub const UPDATE_TIME_MS: u64 = 4 * 60 * 60 * 1000;
pub const MIN_QUOTED_PRICE: f64 = 0.01;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Good {
    Berries,
    Wheat,
    Flour,
    Wood,
    Water,
    BreadRoll,
    BerryPie,
    Flax,
    Thread,
    Cloth,
    FlaxBlock,
    FlaxGarment,
    Fish,
    Cabbage,
    Hops,
    Beer,
}

impl Good {
    pub const COUNT: usize = 16;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Berries,
        Self::Wheat,
        Self::Flour,
        Self::Wood,
        Self::Water,
        Self::BreadRoll,
        Self::BerryPie,
        Self::Flax,
        Self::Thread,
        Self::Cloth,
        Self::FlaxBlock,
        Self::FlaxGarment,
        Self::Fish,
        Self::Cabbage,
        Self::Hops,
        Self::Beer,
    ];
    pub const FOOD: [Self; 5] = [
        Self::Berries,
        Self::BreadRoll,
        Self::BerryPie,
        Self::Fish,
        Self::Cabbage,
    ];
    pub fn caravan_eligible(self) -> bool {
        !matches!(self, Self::Berries | Self::Water)
    }
    pub fn core_price(self) -> f64 {
        Prices::core().price(self).unwrap()
    }
    pub fn export_threshold(self) -> f64 {
        self.core_price() * 0.5
    }
    pub fn import_threshold(self) -> f64 {
        self.core_price() * 1.5
    }
    pub fn nutrition_per_unit(self) -> Option<f64> {
        match self {
            Self::Berries => Some(crate::BERRY_NUTRITION_PER_GRAM),
            Self::BreadRoll => Some(10.0),
            Self::BerryPie => Some(60.0),
            Self::Fish | Self::Cabbage => Some(25.0),
            _ => None,
        }
    }
    pub fn eating_ms_per_unit(self) -> Option<u64> {
        self.nutrition_per_unit()
            .map(|_| self.weight_grams() * 1000)
    }
    pub fn weight_grams(self) -> u64 {
        match self {
            Self::Fish => 250,
            Self::Cabbage => 500,
            Self::BreadRoll => 20,
            Self::BerryPie => 125,
            Self::FlaxBlock => 25,
            Self::FlaxGarment => 200,
            _ => 1,
        }
    }
    pub fn unit_name(self) -> &'static str {
        match self {
            Self::Fish => "fish",
            Self::Cabbage => "cabbages",
            Self::BreadRoll => "rolls",
            Self::BerryPie => "pies",
            Self::FlaxBlock => "blocks",
            Self::FlaxGarment => "garments",
            _ => "g",
        }
    }
    pub fn price_unit_name(self) -> &'static str {
        match self {
            Self::BreadRoll => "roll",
            Self::BerryPie => "pie",
            Self::FlaxBlock | Self::FlaxGarment | Self::Fish | Self::Cabbage => "each",
            _ => "kg",
        }
    }
    pub fn units_per_price_unit(self) -> u64 {
        match self {
            Self::BreadRoll
            | Self::BerryPie
            | Self::FlaxBlock
            | Self::FlaxGarment
            | Self::Fish
            | Self::Cabbage => 1,
            _ => 1000,
        }
    }
    pub fn display_quantity(self, units: Quantity) -> f64 {
        units as f64 / self.units_per_price_unit() as f64
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Berries => "Berries",
            Self::Wheat => "Wheat",
            Self::Flour => "Flour",
            Self::Wood => "Wood",
            Self::Water => "Water",
            Self::BreadRoll => "Bread rolls",
            Self::BerryPie => "Berry pie",
            Self::Flax => "Flax",
            Self::Thread => "Thread",
            Self::Cloth => "Cloth",
            Self::FlaxBlock => "Flax block",
            Self::FlaxGarment => "Flax garment",
            Self::Fish => "Fish",
            Self::Cabbage => "Cabbage",
            Self::Hops => "Hops",
            Self::Beer => "Beer",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportPolicy {
    pub allowed: bool,
    pub minimum_reserve: Quantity,
    #[serde(default = "imports_enabled")]
    pub imports_allowed: bool,
    #[serde(default)]
    pub import_tariff_basis_points: i32,
    #[serde(default)]
    pub export_tariff_basis_points: i32,
}

fn imports_enabled() -> bool {
    true
}

impl Default for ExportPolicy {
    fn default() -> Self {
        Self {
            allowed: true,
            minimum_reserve: 0,
            imports_allowed: true,
            import_tariff_basis_points: 0,
            export_tariff_basis_points: 0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaravanPolicy {
    pub exports: [ExportPolicy; Good::COUNT],
    pub import_tariff_basis_points: i32,
    pub export_tariff_basis_points: i32,
}

impl Default for CaravanPolicy {
    fn default() -> Self {
        Self {
            exports: Good::ALL.map(|good| ExportPolicy {
                allowed: good.caravan_eligible(),
                imports_allowed: good.caravan_eligible(),
                ..ExportPolicy::default()
            }),
            import_tariff_basis_points: 0,
            export_tariff_basis_points: 0,
        }
    }
}

impl CaravanPolicy {
    pub fn import_rate(self, good: Good) -> i64 {
        i64::from(self.import_tariff_basis_points)
            + i64::from(self.exports[good as usize].import_tariff_basis_points)
    }
    pub fn export_rate(self, good: Good) -> i64 {
        i64::from(self.export_tariff_basis_points)
            + i64::from(self.exports[good as usize].export_tariff_basis_points)
    }

    pub fn validate(self) -> Result<(), SimulationError> {
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
struct PurchaseBounds {
    quote_ceilings: [f64; Good::COUNT],
    spending_budget: Coins,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ShoppingList {
    units: [Quantity; Good::COUNT],
    purchase_bounds: Option<PurchaseBounds>,
}

impl Default for ShoppingList {
    fn default() -> Self {
        Self {
            units: [0; Good::COUNT],
            purchase_bounds: None,
        }
    }
}

impl ShoppingList {
    pub fn new(items: impl IntoIterator<Item = (Good, Quantity)>) -> Result<Self, SimulationError> {
        let mut list = Self::default();
        for (good, units) in items {
            list.units[good as usize] = list.units[good as usize]
                .checked_add(units)
                .ok_or(SimulationError::InvalidQuantity)?;
        }
        Ok(list)
    }

    pub fn single(good: Good, units: Quantity) -> Self {
        let mut list = Self::default();
        list.units[good as usize] = units;
        list
    }

    pub(crate) fn spending_budget(self) -> Coins {
        self.purchase_bounds
            .map_or(Coins::MAX, |bounds| bounds.spending_budget)
    }
    pub(crate) fn quote_ceiling(self, good: Good) -> f64 {
        self.purchase_bounds
            .map_or(f64::MAX, |bounds| bounds.quote_ceilings[good as usize])
    }
    pub(crate) fn with_purchase_bounds(
        mut self,
        market: &Market,
        buyer: MarketParty,
        place: PlaceId,
        budget: Coins,
    ) -> Result<Self, SimulationError> {
        if self.purchase_bounds.is_some() {
            return Ok(self);
        }
        let mut market = market.clone();
        let mut cost: Coins = 0;
        let mut quote_ceilings = [0.0; Good::COUNT];
        for (good, units) in self.items() {
            let fill = market.fill_plan(buyer, Some(place), good, units, None)?;
            market.apply_fill_budget(good, &fill);
            quote_ceilings[good as usize] = fill
                .fills
                .iter()
                .filter_map(|(id, _, _)| market.order(*id))
                .map(|order| order.quoted_price)
                .max_by(f64::total_cmp)
                .unwrap_or(0.0);
            cost = cost
                .checked_add(fill.coins)
                .ok_or(SimulationError::WealthOverflow)?;
        }
        self.purchase_bounds = Some(PurchaseBounds {
            quote_ceilings,
            spending_budget: cost.min(budget).max(0),
        });
        Ok(self)
    }

    pub(crate) fn with_quote_ceilings(
        mut self,
        quote_ceilings: [f64; Good::COUNT],
        budget: Coins,
    ) -> Self {
        self.purchase_bounds = Some(PurchaseBounds {
            quote_ceilings,
            spending_budget: budget.max(0),
        });
        self
    }

    pub(crate) fn with_units_from(mut self, list: Self) -> Self {
        self.units = list.units;
        self
    }

    #[cfg(test)]
    pub(crate) fn without_purchase_bounds(mut self) -> Self {
        self.purchase_bounds = None;
        self
    }

    pub(crate) fn validate_bounds(self) -> bool {
        self.purchase_bounds.is_none_or(|bounds| {
            bounds.spending_budget >= 0
                && bounds
                    .quote_ceilings
                    .iter()
                    .all(|price| price.is_finite() && *price >= 0.0)
        })
    }
    pub fn units(self, good: Good) -> Quantity {
        self.units[good as usize]
    }

    /// Purchases use catalogue order when the budget cannot fill the entire list.
    pub fn items(self) -> impl Iterator<Item = (Good, Quantity)> {
        Good::ALL
            .into_iter()
            .map(move |good| (good, self.units[good as usize]))
            .filter(|(_, units)| *units > 0)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Prices {
    quoted_price: [f64; Good::COUNT],
}
impl Default for Prices {
    fn default() -> Self {
        Self::core()
    }
}
impl Prices {
    pub fn core() -> Self {
        static CORE: std::sync::LazyLock<Prices> = std::sync::LazyLock::new(Prices::calculate_core);
        *CORE
    }
    fn calculate_core() -> Self {
        fn resolve(
            good: Good,
            prices: &mut [Option<f64>; Good::COUNT],
            visiting: &mut [bool; Good::COUNT],
        ) -> f64 {
            let index = good as usize;
            if let Some(price) = prices[index] {
                return price;
            }
            assert!(!visiting[index], "production recipes must be acyclic");
            visiting[index] = true;
            let recipe = crate::production::Recipe::ALL
                .into_iter()
                .find(|recipe| recipe.outputs().iter().any(|&(output, _)| output == good))
                .expect("every good must have a production recipe");
            let &[(output, quantity)] = recipe.outputs() else {
                panic!("core prices require single-output recipes");
            };
            assert_eq!(output, good);
            let inputs: f64 = recipe
                .inputs()
                .iter()
                .map(|&(input, units)| {
                    units as f64 * resolve(input, prices, visiting)
                        / input.units_per_price_unit() as f64
                })
                .sum();
            let hours = recipe.base_duration_ms() as f64 / 3_600_000.0;
            let price = (hours * 10.0 + inputs) * 1.3 / quantity as f64
                * good.units_per_price_unit() as f64;
            assert!(price.is_finite() && price >= MIN_QUOTED_PRICE);
            visiting[index] = false;
            prices[index] = Some(price);
            price
        }
        let mut prices = [None; Good::COUNT];
        let mut visiting = [false; Good::COUNT];
        let quoted_price = Good::ALL.map(|good| resolve(good, &mut prices, &mut visiting));
        Self { quoted_price }
    }
    pub fn new(berries: f64) -> Result<Self, SimulationError> {
        Self::default().with_price(Good::Berries, berries)
    }
    pub fn with_price(self, good: Good, price: f64) -> Result<Self, SimulationError> {
        if !price.is_finite() || price < MIN_QUOTED_PRICE {
            return Err(SimulationError::InvalidPrices);
        }
        let mut prices = self;
        prices.quoted_price[good as usize] = price;
        Ok(prices)
    }
    pub fn price(self, good: Good) -> Option<f64> {
        Some(self.quoted_price[good as usize])
    }
    pub fn value(self, good: Good, units: Quantity) -> Option<f64> {
        self.price(good)
            .map(|price| units as f64 / good.units_per_price_unit() as f64 * price)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OrderId(pub u64);

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MarketParty {
    Citizen(AgentId),
    Household(AgentId),
    Caravan,
}

impl From<AgentId> for MarketParty {
    fn from(id: AgentId) -> Self {
        Self::Citizen(id)
    }
}

impl MarketParty {
    pub fn account(self) -> Option<AgentId> {
        match self {
            Self::Citizen(id) | Self::Household(id) => Some(id),
            Self::Caravan => None,
        }
    }

    pub fn citizen(self) -> Option<AgentId> {
        match self {
            Self::Citizen(id) => Some(id),
            Self::Household(_) | Self::Caravan => None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct SellOrder {
    pub id: OrderId,
    pub seller: MarketParty,
    pub place: PlaceId,
    pub good: Good,
    pub units: Quantity,
    pub quoted_price: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Trade {
    pub order_id: OrderId,
    pub place: PlaceId,
    pub time_ms: u64,
    pub buyer: MarketParty,
    pub seller: MarketParty,
    pub good: Good,
    pub units: Quantity,
    pub coins: Coins,
    pub quoted_price: f64,
    pub tariff_coins: Coins,
    pub recipient_coins: Coins,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct DailyMarketActivity {
    pub local_quoted_value: f64,
    pub imported_quoted_value: f64,
    pub accepted_local_price: Option<f64>,
    pub start_ms: u64,
    pub end_ms: u64,
    pub good: Good,
    pub traded_units: Quantity,
    pub local_traded_units: Quantity,
    pub local_traded_coins: Coins,
    pub exported_units: Quantity,
    pub exported_coins: Coins,
    pub exported_receipts: Coins,
    pub export_tariff_coins: Coins,
    pub import_tariff_coins: Coins,
    pub imported_units: Quantity,
    pub caravan_purchased_units: Quantity,
    pub caravan_purchased_coins: Coins,
    pub traded_coins: Coins,
    pub remaining_supply_units: Quantity,
    pub caravan_remaining_supply_units: Quantity,
    pub unmet_demand_units: Quantity,
    pub price_before: f64,
    pub price_after: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct GoodActivity {
    pub local_quoted_value: f64,
    pub imported_quoted_value: f64,
    pub traded_coins: Coins,
    pub caravan_listed_units: Quantity,
    pub traded_units: Quantity,
    pub local_traded_units: Quantity,
    pub local_traded_coins: Coins,
    pub exported_units: Quantity,
    pub exported_coins: Coins,
    pub exported_receipts: Coins,
    pub export_tariff_coins: Coins,
    pub import_tariff_coins: Coins,
    pub imported_units: Quantity,
    pub caravan_purchased_units: Quantity,
    pub caravan_purchased_coins: Coins,
    pub listed_units: Quantity,
    pub affordable_demand_units: Quantity,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct MarketPeriod {
    pub start_ms: u64,
    pub end_ms: u64,
    pub goods: [GoodActivity; Good::COUNT],
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct SellerState {
    pub established_price: Option<f64>,
    pub quoted_price: f64,
    pub first_listed_ms: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct ListingBatch {
    quantity: Quantity,
    listed_at_ms: u64,
    local_sales_by_price: Vec<(f64, Quantity)>,
}

// Derived state must not affect market equality or save files.
#[derive(Clone, Debug, Default)]
struct AffordabilityCache(Option<AffordabilityInputs>, #[cfg(test)] usize);

impl PartialEq for AffordabilityCache {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Clone, Debug)]
struct AffordabilityInputs {
    orders: HashMap<OrderId, SellOrder>,
    prices: Prices,
    policy: CaravanPolicy,
    treasury_budget: Coins,
    accounts: HashMap<AgentId, AffordableAccount>,
}

#[derive(Clone, Debug)]
struct AffordableAccount {
    buyer: MarketParty,
    budget: Coins,
    request: ShoppingList,
    affordable: ShoppingList,
}

// This is a planning/settlement snapshot of Universe's authoritative treasury.
#[derive(Clone, Copy, Debug, Default)]
struct TreasuryBudget(Coins);
impl PartialEq for TreasuryBudget {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Market {
    containers: HashMap<OrderId, crate::locations::ObjectId>,
    batches: HashMap<OrderId, ListingBatch>,
    pub(crate) production_trials: crate::production_trials::ProductionTrials,
    seller_rates: HashMap<AgentId, u8>,
    seller_states: HashMap<(AgentId, Good, PlaceId), SellerState>,
    trial_orders: HashSet<OrderId>,
    clock_ms: u64,
    last_local_prices: [Option<(u64, f64)>; Good::COUNT],
    retained_local_prices: [Option<(u64, f64)>; Good::COUNT],
    demand_budgets: HashMap<AgentId, Coins>,
    pub prices: Prices,
    policy: CaravanPolicy,
    #[serde(skip)]
    treasury_budget: TreasuryBudget,
    import_tariff_carry: [i16; Good::COUNT],
    export_tariff_carry: [i16; Good::COUNT],
    orders: HashMap<OrderId, SellOrder>,
    trades: Vector<Trade>,
    next_order_id: u64,
    requests: HashMap<AgentId, ShoppingList>,
    affordable_requests: HashMap<AgentId, ShoppingList>,
    #[serde(skip)]
    affordability_cache: AffordabilityCache,
    history: Vector<DailyMarketActivity>,
    period_start_ms: u64,
    first_period_start_ms: u64,
    started_at_ms: u64,
    period_activity: [GoodActivity; Good::COUNT],
}

pub(crate) struct Purchase {
    pub units: Quantity,
    pub coins: Coins,
    pub payments: Vec<(AgentId, Coins)>,
}

impl Market {
    pub(crate) fn restore_treasury_context(&mut self, coins: Coins) {
        self.set_treasury_budget(coins);
        for good in Good::ALL {
            self.policy.exports[good as usize].imports_allowed &= good.caravan_eligible();
        }
    }
    pub(crate) fn set_treasury_budget(&mut self, coins: Coins) {
        self.treasury_budget.0 = coins;
    }
    fn apply_fill_budget(&mut self, good: Good, fill: &FillPlan) {
        self.treasury_budget.0 = fill.treasury_budget;
        self.import_tariff_carry[good as usize] = fill.import_carry;
    }
    pub fn order_container(&self, id: OrderId) -> Option<crate::locations::ObjectId> {
        self.containers.get(&id).copied()
    }
    pub(crate) fn bind_order(&mut self, id: OrderId, object: crate::locations::ObjectId) {
        self.containers.insert(id, object);
    }
    pub(crate) fn bind_unplaced_orders(
        &mut self,
        map: &crate::locations::Map,
        storage: &crate::storage::Storage,
    ) -> Result<(), SimulationError> {
        let mut orders: Vec<_> = self
            .orders()
            .filter(|order| !self.containers.contains_key(&order.id))
            .cloned()
            .collect();
        orders.sort_by_key(|order| order.id.0);
        for order in orders {
            let object = storage.select_object(order.place, order.good, map, self, None)?;
            self.bind_order(order.id, object);
        }
        Ok(())
    }
    pub(crate) fn list_in(
        &mut self,
        seller: MarketParty,
        place: PlaceId,
        good: Good,
        units: Quantity,
        object: crate::locations::ObjectId,
    ) -> Result<(), SimulationError> {
        let id = OrderId(self.next_order_id);
        self.list(seller, place, good, units)?;
        if units > 0 {
            self.bind_order(id, object);
        }
        Ok(())
    }
    pub(crate) fn set_time(&mut self, time_ms: u64) {
        self.clock_ms = time_ms;
    }

    pub fn retained_local_price(&self, good: Good) -> Option<(u64, f64)> {
        self.retained_local_prices[good as usize]
    }

    pub fn last_local_price(&self, good: Good) -> Option<(u64, f64)> {
        self.last_local_prices[good as usize]
    }

    pub(crate) fn ensure_seller_adjustment_rate(&mut self, account: AgentId) -> u8 {
        let rate = self.production_adjustment_rate(account);
        *self.seller_rates.entry(account).or_insert(rate)
    }

    pub(crate) fn production_adjustment_rate(&self, account: AgentId) -> u8 {
        self.seller_rates.get(&account).copied().unwrap_or_else(|| {
            account
                .0
                .as_bytes()
                .iter()
                .copied()
                .find(|byte| *byte < 250)
                .unwrap_or(0)
                % 10
                + 1
        })
    }

    pub(crate) fn production_snapshot(
        &self,
        buyer: MarketParty,
        map: &crate::locations::Map,
    ) -> (
        [Quantity; Good::COUNT],
        [Option<f64>; Good::COUNT],
        [Option<f64>; Good::COUNT],
    ) {
        let mut owned = [0; Good::COUNT];
        let mut purchases: [Option<f64>; Good::COUNT] = [None; Good::COUNT];
        let mut listings: [Option<f64>; Good::COUNT] = [None; Good::COUNT];
        for order in self.orders() {
            let index = order.good as usize;
            if order.seller == buyer {
                owned[index] += order.units;
            }
            if order.units == 0
                || !self.purchasable(order)
                || !map
                    .places()
                    .get(&order.place)
                    .is_some_and(|place| place.kind != crate::locations::Location::Home)
            {
                continue;
            }
            let price = order.quoted_price;
            if listings[index].is_none_or(|current| price.total_cmp(&current).is_lt()) {
                listings[index] = Some(price);
            }
            if order.seller != buyer
                && purchases[index].is_none_or(|current| price.total_cmp(&current).is_lt())
            {
                purchases[index] = Some(price);
            }
        }
        (owned, purchases, listings)
    }

    pub(crate) fn lowest_purchase_quote(
        &self,
        buyer: MarketParty,
        good: Good,
        map: &crate::locations::Map,
    ) -> Option<f64> {
        self.orders()
            .filter(|order| {
                order.units > 0
                    && order.good == good
                    && order.seller != buyer
                    && self.purchasable(order)
                    && self.can_fund_first_unit(order)
                    && map
                        .places()
                        .get(&order.place)
                        .is_some_and(|place| place.kind != crate::locations::Location::Home)
            })
            .map(|order| order.quoted_price)
            .min_by(f64::total_cmp)
    }

    #[cfg(test)]
    pub(crate) fn lowest_listed_quote(
        &self,
        good: Good,
        map: &crate::locations::Map,
    ) -> Option<f64> {
        self.orders()
            .filter(|order| {
                order.units > 0
                    && order.good == good
                    && self.purchasable(order)
                    && map
                        .places()
                        .get(&order.place)
                        .is_some_and(|place| place.kind != crate::locations::Location::Home)
            })
            .map(|order| order.quoted_price)
            .min_by(f64::total_cmp)
    }

    pub fn seller_adjustment_rate(&self, account: AgentId) -> Option<u8> {
        self.seller_rates.get(&account).copied()
    }

    pub fn seller_state(
        &self,
        account: AgentId,
        place: PlaceId,
        good: Good,
    ) -> Option<&SellerState> {
        self.seller_states.get(&(account, good, place))
    }

    pub(crate) fn known_trial_price(&self, account: AgentId, place: PlaceId, good: Good) -> f64 {
        self.seller_state(account, place, good)
            .map(|state| state.quoted_price)
            .unwrap_or_else(|| self.prices.price(good).unwrap())
    }
    pub(crate) fn establish_trial_price(
        &mut self,
        account: AgentId,
        place: PlaceId,
        good: Good,
        quote: f64,
    ) {
        if let Some(state) = self.seller_states.get_mut(&(account, good, place)) {
            state.established_price = Some(state.quoted_price.max(quote));
            state.quoted_price = state.quoted_price.max(quote);
        }
    }
    pub fn order(&self, id: OrderId) -> Option<&SellOrder> {
        self.orders.get(&id)
    }
    pub fn order_is_trial(&self, id: OrderId) -> bool {
        self.trial_orders.contains(&id)
    }
    pub(crate) fn mark_trial_order(&mut self, id: OrderId, trial: bool) {
        if trial {
            self.trial_orders.insert(id);
        } else {
            self.trial_orders.remove(&id);
        }
    }
    #[cfg(test)]
    pub(crate) fn set_order_price(
        &mut self,
        id: OrderId,
        quote: f64,
    ) -> Result<(), SimulationError> {
        if !quote.is_finite() || quote < MIN_QUOTED_PRICE {
            return Err(SimulationError::InvalidPrices);
        }
        if let Some(order) = self.orders.get_mut(&id) {
            order.quoted_price = quote;
        }
        Ok(())
    }

    pub fn affordable_unmet_at_quote(&self, target: Good, quote: f64) -> Quantity {
        self.requests
            .iter()
            .map(|(account, request)| {
                let mut market = self.clone();
                let mut funds = self
                    .demand_budgets
                    .get(account)
                    .copied()
                    .unwrap_or(0)
                    .max(0);
                let party = self
                    .orders()
                    .find(|order| order.seller.account() == Some(*account))
                    .map_or(MarketParty::Citizen(*account), |order| order.seller);
                let mut wanted = 0;
                for (good, requested) in request.items() {
                    let ceiling = if good == target { quote } else { f64::MAX };
                    let Ok(fill) = market.fill_plan_at_quote(
                        party,
                        None,
                        good,
                        requested,
                        Some(funds),
                        ceiling,
                    ) else {
                        continue;
                    };
                    market.apply_fill_budget(good, &fill);
                    funds -= fill.coins;
                    let price = if good == target {
                        quote
                    } else {
                        self.prices.price(good).unwrap()
                    };
                    let mut low = 0;
                    let mut high = requested.saturating_sub(fill.units);
                    while low < high {
                        let middle = low + (high - low).div_ceil(2);
                        if charge(middle as f64 * price / good.units_per_price_unit() as f64)
                            .is_ok_and(|cost| {
                                let mut carry = market.import_tariff_carry[good as usize];
                                cost <= funds
                                    && (good != target
                                        || tariff_share(
                                            cost,
                                            market.policy.import_rate(good),
                                            &mut carry,
                                        )
                                        .ok()
                                        .and_then(|tariff| {
                                            market.treasury_budget.0.checked_add(tariff)
                                        })
                                        .is_some_and(|balance| balance >= 0))
                            })
                        {
                            low = middle;
                        } else {
                            high = middle - 1;
                        }
                    }
                    if low > 0 {
                        funds -= charge(low as f64 * price / good.units_per_price_unit() as f64)
                            .unwrap();
                    }
                    if good == target {
                        wanted = low;
                    }
                }
                wanted
            })
            .sum()
    }

    pub fn caravan_policy(&self) -> CaravanPolicy {
        self.policy
    }

    pub fn effective_export_threshold(&self, good: Good) -> Option<f64> {
        (good.caravan_eligible()
            && self.policy.exports[good as usize].allowed
            && self.policy.export_rate(good) < 10_000)
            .then(|| {
                good.export_threshold() * (1.0 - self.policy.export_rate(good) as f64 / 10_000.0)
            })
    }

    pub fn effective_import_threshold(&self, good: Good) -> Option<f64> {
        (good.caravan_eligible()
            && self.policy.exports[good as usize].imports_allowed
            && self.policy.import_rate(good) < 10_000)
            .then(|| {
                good.import_threshold() / (1.0 - self.policy.import_rate(good) as f64 / 10_000.0)
            })
    }

    pub(crate) fn set_caravan_policy(
        &mut self,
        mut policy: CaravanPolicy,
    ) -> Result<(), SimulationError> {
        policy.validate()?;
        for good in Good::ALL {
            policy.exports[good as usize].allowed &= good.caravan_eligible();
            policy.exports[good as usize].imports_allowed &= good.caravan_eligible();
        }
        self.policy = policy;
        self.remove_ineligible_caravan_orders();
        for id in self.orders.keys().copied().collect::<Vec<_>>() {
            let good = self.orders[&id].good;
            if self.orders[&id].seller == MarketParty::Caravan {
                self.orders.get_mut(&id).unwrap().quoted_price = self
                    .effective_import_threshold(good)
                    .unwrap_or(good.import_threshold());
            }
        }
        Ok(())
    }

    fn remove_ineligible_caravan_orders(&mut self) {
        self.orders.retain(|_, order| {
            order.seller != MarketParty::Caravan || order.good.caravan_eligible()
        });
        self.containers.retain(|id, _| self.orders.contains_key(id));
    }

    fn can_fund_first_unit(&self, order: &SellOrder) -> bool {
        if order.seller != MarketParty::Caravan {
            return true;
        }
        let mut carry = self.import_tariff_carry[order.good as usize];
        charge(order.quoted_price / order.good.units_per_price_unit() as f64)
            .and_then(|coins| tariff_share(coins, self.policy.import_rate(order.good), &mut carry))
            .ok()
            .and_then(|tariff| self.treasury_budget.0.checked_add(tariff))
            .is_some_and(|balance| balance >= 0)
    }

    fn purchasable(&self, order: &SellOrder) -> bool {
        order.seller != MarketParty::Caravan
            || self.effective_import_threshold(order.good).is_some()
    }

    pub(crate) fn starting_at(time_ms: u64) -> Self {
        let period_start_ms =
            crate::production::period_start(crate::production::work_period(time_ms));
        Self {
            period_start_ms,
            first_period_start_ms: period_start_ms,
            started_at_ms: time_ms,
            clock_ms: time_ms,
            ..Self::default()
        }
    }

    /// Current activity; the end is the next scheduled 04:00 close.
    pub fn current_period(&self) -> MarketPeriod {
        let mut goods = self.period_activity;
        for order in self.orders() {
            if self.purchasable(order) {
                goods[order.good as usize].listed_units += order.units;
            }
            if order.seller == MarketParty::Caravan {
                goods[order.good as usize].caravan_listed_units += order.units;
            }
        }
        for request in self.affordable_requests.values() {
            for (good, units) in request.items() {
                goods[good as usize].affordable_demand_units += units;
            }
        }
        MarketPeriod {
            start_ms: self.period_start_ms,
            end_ms: if self.period_start_ms == 0 {
                UPDATE_TIME_MS
            } else {
                self.period_start_ms.saturating_add(DAY_MS)
            },
            goods,
        }
    }

    /// Latest closed period, including zero activity for inactive goods.
    pub fn previous_period(&self) -> Option<MarketPeriod> {
        if self.period_start_ms == self.first_period_start_ms {
            return None;
        }
        let mut goods = [GoodActivity::default(); Good::COUNT];
        // History omits inactive goods and entire inactive periods.
        for activity in self
            .history
            .iter()
            .rev()
            .take_while(|activity| activity.end_ms == self.period_start_ms)
        {
            goods[activity.good as usize] = GoodActivity {
                local_quoted_value: activity.local_quoted_value,
                imported_quoted_value: activity.imported_quoted_value,
                traded_units: activity.traded_units,
                traded_coins: activity.traded_coins,
                caravan_listed_units: activity.caravan_remaining_supply_units,
                local_traded_units: activity.local_traded_units,
                local_traded_coins: activity.local_traded_coins,
                exported_units: activity.exported_units,
                exported_coins: activity.exported_coins,
                exported_receipts: activity.exported_receipts,
                export_tariff_coins: activity.export_tariff_coins,
                import_tariff_coins: activity.import_tariff_coins,
                imported_units: activity.imported_units,
                caravan_purchased_units: activity.caravan_purchased_units,
                caravan_purchased_coins: activity.caravan_purchased_coins,

                listed_units: activity.remaining_supply_units,
                affordable_demand_units: activity.unmet_demand_units,
            };
        }
        Some(MarketPeriod {
            start_ms: self.period_start_ms.saturating_sub(DAY_MS),
            end_ms: self.period_start_ms,
            goods,
        })
    }

    /// Initial market period and actual world creation time.
    pub fn initial_period(&self) -> (u64, u64) {
        (self.first_period_start_ms, self.started_at_ms)
    }

    pub fn history(&self) -> &Vector<DailyMarketActivity> {
        &self.history
    }
    pub fn requested(&self, agent: AgentId) -> ShoppingList {
        self.requests.get(&agent).copied().unwrap_or_default()
    }
    pub fn affordable_request(&self, agent: AgentId) -> ShoppingList {
        self.affordable_requests
            .get(&agent)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn set_request(
        &mut self,
        agent: AgentId,
        request: ShoppingList,
    ) -> Result<(), SimulationError> {
        for good in Good::ALL {
            self.requests
                .iter()
                .filter(|(id, _)| **id != agent)
                .map(|(_, list)| list.units(good))
                .try_fold(request.units(good), Quantity::checked_add)
                .ok_or(SimulationError::InvalidQuantity)?;
        }
        if request.items().next().is_none() {
            self.requests.remove(&agent);
        } else {
            self.requests.insert(agent, request);
        }
        Ok(())
    }

    pub(crate) fn refresh_affordability<P: Copy + Into<MarketParty>>(
        &mut self,
        budgets: &[(P, Coins)],
    ) {
        let mut affordable = HashMap::new();
        self.demand_budgets = budgets
            .iter()
            .filter_map(|&(party, coins)| {
                party
                    .into()
                    .account()
                    .filter(|id| self.requests.contains_key(id))
                    .map(|id| (id, coins))
            })
            .collect();
        let previous = self.affordability_cache.0.as_ref().filter(|cached| {
            cached.orders.ptr_eq(&self.orders)
                && cached.prices == self.prices
                && cached.policy == self.policy
                && cached.treasury_budget == self.treasury_budget.0
        });
        let mut accounts = HashMap::new();
        #[cfg(test)]
        let mut calculations = 0;
        for &(buyer, budget) in budgets {
            let buyer = buyer.into();
            let Some(account) = buyer.account() else {
                continue;
            };
            let request = self.requested(account);
            if request.items().next().is_none() {
                continue;
            }
            let list = previous
                .and_then(|cached| cached.accounts.get(&account))
                .filter(|cached| {
                    cached.buyer == buyer && cached.budget == budget && cached.request == request
                })
                .map(|cached| cached.affordable)
                .unwrap_or_else(|| {
                    #[cfg(test)]
                    {
                        calculations += 1;
                    }
                    self.calculate_affordable_request(buyer, budget, request)
                });
            if list.items().next().is_some() {
                affordable.insert(account, list);
            }
            accounts.insert(
                account,
                AffordableAccount {
                    buyer,
                    budget,
                    request,
                    affordable: list,
                },
            );
        }
        self.affordable_requests = affordable;
        #[cfg(test)]
        {
            self.affordability_cache.1 += calculations;
        }
        self.affordability_cache.0 = Some(AffordabilityInputs {
            orders: self.orders.clone(),
            prices: self.prices,
            policy: self.policy,
            treasury_budget: self.treasury_budget.0,
            accounts,
        });
    }

    fn calculate_affordable_request(
        &self,
        buyer: MarketParty,
        budget: Coins,
        request: ShoppingList,
    ) -> ShoppingList {
        let mut market = self.clone();
        let mut funds = budget.max(0);
        let mut items = Vec::new();
        for (good, requested) in request.items() {
            if let Ok(fill) = market.fill_plan(buyer, None, good, requested, Some(funds)) {
                market.apply_fill_budget(good, &fill);
                funds -= fill.coins;
                let unavailable = requested.saturating_sub(self.available_units(buyer, good));
                let mut low = 0;
                let mut high = unavailable;
                while low < high {
                    let middle = low + (high - low).div_ceil(2);
                    if self
                        .prices
                        .value(good, middle)
                        .and_then(|value| charge(value).ok())
                        .is_some_and(|cost| cost <= funds)
                    {
                        low = middle;
                    } else {
                        high = middle - 1;
                    }
                }
                if low > 0 {
                    funds -= charge(self.prices.value(good, low).unwrap()).unwrap();
                }
                let units = fill.units + low;
                if units > 0 {
                    items.push((good, units));
                }
            }
        }
        ShoppingList::new(items).unwrap_or_default()
    }

    pub fn orders(&self) -> impl Iterator<Item = &SellOrder> {
        self.orders.values()
    }
    pub fn trades(&self) -> &Vector<Trade> {
        &self.trades
    }
    pub fn listed_units(&self, owner: impl Into<MarketParty>, good: Good) -> Quantity {
        let owner = owner.into();
        self.orders()
            .filter(|o| o.seller == owner && o.good == good)
            .map(|o| o.units)
            .sum()
    }
    pub fn available_units(&self, buyer: impl Into<MarketParty>, good: Good) -> Quantity {
        let buyer = buyer.into();
        self.orders()
            .filter(|o| self.purchasable(o) && o.seller != buyer && o.good == good)
            .map(|o| o.units)
            .sum()
    }
    pub fn available_units_at(
        &self,
        buyer: impl Into<MarketParty>,
        place: PlaceId,
        good: Good,
    ) -> Quantity {
        let buyer = buyer.into();
        self.orders()
            .filter(|order| {
                self.purchasable(order)
                    && order.seller != buyer
                    && order.place == place
                    && order.good == good
            })
            .map(|order| order.units)
            .sum()
    }

    pub(crate) fn affordable_purchase_list_at(
        &self,
        buyer: MarketParty,
        place: Option<PlaceId>,
        list: ShoppingList,
        budget: Coins,
    ) -> Result<ShoppingList, SimulationError> {
        let mut market = self.clone();
        let mut remaining = budget.max(0);
        let mut fills = Vec::new();
        for (good, units) in list.items() {
            let fill = market.fill_plan(buyer, place, good, units, Some(remaining))?;
            market.apply_fill_budget(good, &fill);
            remaining -= fill.coins;
            fills.push((good, fill.units));
        }
        ShoppingList::new(fills)
    }

    pub fn purchase_cost_at(
        &self,
        buyer: impl Into<MarketParty>,
        place: PlaceId,
        good: Good,
        requested: Quantity,
    ) -> Option<Coins> {
        let buyer = buyer.into();
        Some(
            self.fill_plan(buyer, Some(place), good, requested, None)
                .ok()?
                .coins,
        )
    }

    pub fn unmet_units(&self, good: Good) -> Quantity {
        self.affordable_requests
            .values()
            .map(|list| list.units(good))
            .sum()
    }

    pub(crate) fn next_purchase_stop(
        &self,
        buyer: MarketParty,
        map: &crate::locations::Map,
        position: crate::locations::Position,
        list: ShoppingList,
        budget: Option<Coins>,
    ) -> Option<(PlaceId, ShoppingList)> {
        let mut market = self.clone();
        let mut destination = None;
        let mut basket = Vec::new();
        let mut remaining_budget = budget.map(|budget| budget.min(list.spending_budget()));
        for (good, wanted) in list.items() {
            let mut orders: Vec<_> = self
                .orders()
                .filter(|order| {
                    order.units > 0
                        && order.good == good
                        && order.seller != buyer
                        && order.quoted_price <= list.quote_ceiling(good)
                        && self.purchasable(order)
                        && market.can_fund_first_unit(order)
                        && map
                            .places()
                            .get(&order.place)
                            .is_some_and(|place| place.kind != crate::locations::Location::Home)
                })
                .collect();
            orders.sort_by(|a, b| {
                a.quoted_price
                    .total_cmp(&b.quoted_price)
                    .then_with(|| {
                        position
                            .distance(map.position(a.place))
                            .total_cmp(&position.distance(map.position(b.place)))
                    })
                    .then_with(|| a.place.0.cmp(&b.place.0))
                    .then_with(|| a.id.0.cmp(&b.id.0))
            });
            let Some(first) = orders.first() else {
                continue;
            };
            let place = first.place;
            if destination.is_some_and(|destination| destination != place) {
                continue;
            }
            let units = orders
                .iter()
                .take_while(|order| order.place == place)
                .fold(0_u64, |sum, order| sum.saturating_add(order.units))
                .min(wanted);
            let fill = market
                .fill_plan_at_quote(
                    buyer,
                    Some(place),
                    good,
                    units,
                    remaining_budget,
                    list.quote_ceiling(good),
                )
                .ok()?;
            if fill.units == 0 {
                continue;
            }
            market.apply_fill_budget(good, &fill);
            destination = Some(place);
            if let Some(remaining) = &mut remaining_budget {
                *remaining -= fill.coins;
            }
            basket.push((good, fill.units));
        }
        Some((
            destination?,
            list.with_units_from(ShoppingList::new(basket).ok()?),
        ))
    }

    pub(crate) fn estimated_purchase_cost_from(
        &self,
        buyer: impl Into<MarketParty>,
        map: &crate::locations::Map,
        mut position: crate::locations::Position,
        mut remaining: ShoppingList,
    ) -> Option<Coins> {
        let buyer = buyer.into();
        let mut market = self.clone();
        let mut coins: Coins = 0;
        while let Some((place, basket)) =
            market.next_purchase_stop(buyer, map, position, remaining, None)
        {
            for (good, units) in basket.items() {
                let fill = market
                    .fill_plan(buyer, Some(place), good, units, None)
                    .ok()?;
                market.apply_fill_budget(good, &fill);
                coins = coins.checked_add(fill.coins)?;
                for (id, units, _) in fill.fills {
                    market.orders.get_mut(&id)?.units -= units;
                }
            }
            remaining = ShoppingList::new(
                remaining
                    .items()
                    .map(|(good, units)| (good, units.saturating_sub(basket.units(good)))),
            )
            .ok()?;
            position = map.position(place);
        }
        for (good, units) in remaining.items() {
            coins = coins.checked_add(charge(self.prices.value(good, units)?).ok()?)?;
        }
        Some(coins)
    }

    pub fn estimated_purchase_cost(
        &self,
        buyer: impl Into<MarketParty>,
        good: Good,
        units: Quantity,
    ) -> Option<Coins> {
        let buyer = buyer.into();
        let fill = self.fill_plan(buyer, None, good, units, None).ok()?;
        let unavailable = units - fill.units;
        fill.coins
            .checked_add(charge(self.prices.value(good, unavailable)?).ok()?)
    }

    pub fn purchase_cost(
        &self,
        buyer: impl Into<MarketParty>,
        good: Good,
        requested: Quantity,
    ) -> Option<Coins> {
        let buyer = buyer.into();
        Some(
            self.fill_plan(buyer, None, good, requested, None)
                .ok()?
                .coins,
        )
    }

    fn fill_plan(
        &self,
        buyer: MarketParty,
        place: Option<PlaceId>,
        good: Good,
        requested: Quantity,
        budget: Option<Coins>,
    ) -> Result<FillPlan, SimulationError> {
        self.fill_plan_at_quote(buyer, place, good, requested, budget, f64::MAX)
    }

    fn fill_plan_at_quote(
        &self,
        buyer: MarketParty,
        place: Option<PlaceId>,
        good: Good,
        requested: Quantity,
        budget: Option<Coins>,
        ceiling: f64,
    ) -> Result<FillPlan, SimulationError> {
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| {
                self.purchasable(o)
                    && o.quoted_price <= ceiling
                    && o.seller != buyer
                    && o.good == good
                    && place.is_none_or(|id| o.place == id)
            })
            .collect();
        let mut first_orders = std::collections::HashMap::new();
        for order in &orders {
            first_orders
                .entry((order.place, order.seller, order.quoted_price.to_bits()))
                .and_modify(|first: &mut u64| *first = (*first).min(order.id.0))
                .or_insert(order.id.0);
        }
        orders.sort_by(|a, b| {
            a.quoted_price
                .total_cmp(&b.quoted_price)
                .then_with(|| {
                    first_orders[&(a.place, a.seller, a.quoted_price.to_bits())]
                        .cmp(&first_orders[&(b.place, b.seller, b.quoted_price.to_bits())])
                })
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        let mut plan = FillPlan {
            treasury_budget: self.treasury_budget.0,
            import_carry: self.import_tariff_carry[good as usize],
            ..FillPlan::default()
        };
        let mut seller_costs = std::collections::HashMap::<MarketParty, f64>::new();
        for order in orders {
            let remaining = requested - plan.units;
            if remaining == 0 {
                break;
            }
            let previous = seller_costs.get(&order.seller).copied().unwrap_or(0.0);
            let previous_charge = charge(previous)?;
            let available_coins = budget.unwrap_or(Coins::MAX).max(0) - plan.coins;
            let affordable = |units: Quantity| -> Option<Coins> {
                let quoted = units as f64 * order.quoted_price / good.units_per_price_unit() as f64;
                charge(previous + quoted).ok()?.checked_sub(previous_charge)
            };
            let mut low = 0;
            let mut high = remaining.min(order.units);
            if budget.is_none() && order.seller != MarketParty::Caravan {
                affordable(high)
                    .and_then(|cost| plan.coins.checked_add(cost))
                    .ok_or(SimulationError::WealthOverflow)?;
                low = high;
            }
            while low < high {
                let middle = low + (high - low).div_ceil(2);
                if affordable(middle).is_some_and(|cost| {
                    let mut carry = plan.import_carry;
                    cost <= available_coins
                        && (order.seller != MarketParty::Caravan
                            || tariff_share(cost, self.policy.import_rate(good), &mut carry)
                                .ok()
                                .and_then(|tariff| plan.treasury_budget.checked_add(tariff))
                                .is_some_and(|balance| balance >= 0))
                }) {
                    low = middle;
                } else {
                    high = middle - 1;
                }
            }
            if low == 0 {
                continue;
            }
            let quoted = low as f64 * order.quoted_price / good.units_per_price_unit() as f64;
            let cost = affordable(low).ok_or(SimulationError::WealthOverflow)?;
            if order.seller == MarketParty::Caravan {
                let tariff =
                    tariff_share(cost, self.policy.import_rate(good), &mut plan.import_carry)?;
                plan.treasury_budget = plan
                    .treasury_budget
                    .checked_add(tariff)
                    .ok_or(SimulationError::WealthOverflow)?;
            }
            seller_costs.insert(order.seller, previous + quoted);
            plan.units = plan
                .units
                .checked_add(low)
                .ok_or(SimulationError::WealthOverflow)?;
            plan.coins = plan
                .coins
                .checked_add(cost)
                .ok_or(SimulationError::WealthOverflow)?;
            plan.fills.push((order.id, low, cost));
        }
        Ok(plan)
    }
    pub(crate) fn list(
        &mut self,
        seller: impl Into<MarketParty>,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let seller = seller.into();
        if units == 0 {
            return Ok(());
        }
        let reference = self
            .prices
            .price(good)
            .ok_or(SimulationError::InvalidPrices)?;
        let price = if let Some(account) = seller.account() {
            let rate = f64::from(self.ensure_seller_adjustment_rate(account)) / 100.0;
            self.known_trial_price(account, place, good) * (1.0 + rate)
        } else {
            reference
        };
        self.list_at_quote(seller, place, good, units, price)?;
        Ok(())
    }

    pub(crate) fn list_at_quote(
        &mut self,
        seller: impl Into<MarketParty>,
        place: PlaceId,
        good: Good,
        units: Quantity,
        price: f64,
    ) -> Result<OrderId, SimulationError> {
        let seller = seller.into();
        if units == 0
            || !price.is_finite()
            || price <= 0.0
            || (seller != MarketParty::Caravan && price < MIN_QUOTED_PRICE)
        {
            return Err(SimulationError::InvalidPrices);
        }
        self.orders()
            .filter(|order| order.good == good)
            .map(|order| order.units)
            .try_fold(units, Quantity::checked_add)
            .ok_or(SimulationError::WealthOverflow)?;
        if !(units as f64 * price / good.units_per_price_unit() as f64).is_finite() {
            return Err(SimulationError::WealthOverflow);
        }
        let id = OrderId(self.next_order_id);
        self.next_order_id = self
            .next_order_id
            .checked_add(1)
            .ok_or(SimulationError::TimeOverflow)?;
        if let Some(account) = seller.account() {
            // UUID entropy gives forecasts the same stable policy as live execution.
            self.ensure_seller_adjustment_rate(account);
            self.seller_states
                .entry((account, good, place))
                .or_insert(SellerState {
                    established_price: None,
                    quoted_price: self.prices.price(good).unwrap(),
                    first_listed_ms: self.clock_ms,
                });
        }
        if seller.account().is_some() {
            self.batches.insert(
                id,
                ListingBatch {
                    quantity: crate::production::Recipe::ALL
                        .iter()
                        .flat_map(|recipe| recipe.outputs().iter())
                        .find_map(|&(output, quantity)| (output == good).then_some(quantity))
                        .unwrap_or(units)
                        .min(units),
                    listed_at_ms: self.clock_ms,
                    local_sales_by_price: Vec::new(),
                },
            );
        }
        self.orders.insert(
            id,
            SellOrder {
                id,
                seller,
                place,
                good,
                units,
                quoted_price: price,
            },
        );
        Ok(id)
    }
    pub(crate) fn withdraw(
        &mut self,
        owner: impl Into<MarketParty>,
        place: PlaceId,
        good: Good,
        requested: Quantity,
    ) -> Result<Quantity, SimulationError> {
        let owner = owner.into();
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| o.seller == owner && o.good == good && o.place == place)
            .cloned()
            .collect();
        orders.sort_by_key(|o| o.id.0);
        let mut withdrawn = 0_u64;
        for mut order in orders {
            let units = (requested - withdrawn).min(order.units);
            if units == 0 {
                break;
            }
            withdrawn = withdrawn
                .checked_add(units)
                .ok_or(SimulationError::WealthOverflow)?;
            self.record_trial_withdrawal(order.id, units);
            order.units -= units;
            if order.units == 0 {
                self.orders.remove(&order.id);
                self.containers.remove(&order.id);
                self.batches.remove(&order.id);
            } else {
                self.orders.insert(order.id, order);
            }
        }
        Ok(withdrawn)
    }
    #[cfg(test)]
    pub(crate) fn purchase(
        &mut self,
        buyer: impl Into<MarketParty>,
        place: PlaceId,
        item: (Good, Quantity),
        budget: Coins,
        time_ms: u64,
        record: bool,
    ) -> Result<Purchase, SimulationError> {
        self.purchase_at_quote(buyer, place, item, (budget, f64::MAX), time_ms, record)
    }

    pub(crate) fn purchase_at_quote(
        &mut self,
        buyer: impl Into<MarketParty>,
        place: PlaceId,
        item: (Good, Quantity),
        bounds: (Coins, f64),
        time_ms: u64,
        record: bool,
    ) -> Result<Purchase, SimulationError> {
        let buyer = buyer.into();
        let (good, requested) = item;
        let (budget, ceiling) = bounds;
        let plan =
            self.fill_plan_at_quote(buyer, Some(place), good, requested, Some(budget), ceiling)?;
        self.treasury_budget.0 = plan.treasury_budget;
        let mut payments = std::collections::BTreeMap::new();
        for (id, units, coins) in plan.fills {
            let mut order = self.orders.get(&id).unwrap().clone();
            if let Some(seller) = order.seller.account() {
                let payment = payments.entry(seller.0).or_insert(0_i64);
                *payment = payment
                    .checked_add(coins)
                    .ok_or(SimulationError::WealthOverflow)?;
            }
            let tariff_coins = if order.seller == MarketParty::Caravan {
                tariff_share(
                    coins,
                    self.policy.import_rate(good),
                    &mut self.import_tariff_carry[good as usize],
                )?
            } else {
                0
            };
            order.units -= units;
            if record {
                self.period_activity[good as usize].import_tariff_coins = self.period_activity
                    [good as usize]
                    .import_tariff_coins
                    .checked_add(tariff_coins)
                    .ok_or(SimulationError::WealthOverflow)?;
                let volume = &mut self.period_activity[good as usize];
                if order.seller != MarketParty::Caravan {
                    volume.local_quoted_value +=
                        units as f64 * order.quoted_price / good.units_per_price_unit() as f64;
                }
                volume.traded_units = volume
                    .traded_units
                    .checked_add(units)
                    .ok_or(SimulationError::WealthOverflow)?;
                volume.traded_coins = volume
                    .traded_coins
                    .checked_add(coins)
                    .ok_or(SimulationError::WealthOverflow)?;
                let (units_counter, coins_counter) = if order.seller == MarketParty::Caravan {
                    (
                        &mut volume.caravan_purchased_units,
                        &mut volume.caravan_purchased_coins,
                    )
                } else {
                    (
                        &mut volume.local_traded_units,
                        &mut volume.local_traded_coins,
                    )
                };
                *units_counter = units_counter
                    .checked_add(units)
                    .ok_or(SimulationError::WealthOverflow)?;
                *coins_counter = coins_counter
                    .checked_add(coins)
                    .ok_or(SimulationError::WealthOverflow)?;
                if let Some(request) = buyer.account().and_then(|id| self.requests.get_mut(&id)) {
                    request.units[good as usize] =
                        request.units[good as usize].saturating_sub(units);
                }
                self.trades.push_back(Trade {
                    order_id: order.id,
                    place: order.place,
                    time_ms,
                    buyer,
                    seller: order.seller,
                    good,
                    units,
                    coins,
                    quoted_price: order.quoted_price,
                    tariff_coins,
                    recipient_coins: coins
                        .checked_sub(tariff_coins)
                        .ok_or(SimulationError::WealthOverflow)?,
                });
                if buyer != MarketParty::Caravan
                    && !self.order_is_trial(order.id)
                    && let Some(account) = order.seller.account()
                    && let Some(batch) = self.batches.get_mut(&order.id)
                {
                    let sales = if let Some(index) = batch
                        .local_sales_by_price
                        .iter()
                        .position(|(price, _)| *price == order.quoted_price)
                    {
                        &mut batch.local_sales_by_price[index].1
                    } else {
                        batch.local_sales_by_price.push((order.quoted_price, 0));
                        &mut batch.local_sales_by_price.last_mut().unwrap().1
                    };
                    *sales = sales.saturating_add(units);
                    if sales.saturating_mul(2) >= batch.quantity {
                        self.establish_trial_price(account, order.place, good, order.quoted_price);
                    }
                }
                self.record_trial_sale(
                    order.id,
                    units,
                    coins
                        .checked_sub(tariff_coins)
                        .ok_or(SimulationError::WealthOverflow)?,
                    buyer != MarketParty::Caravan && order.seller != MarketParty::Caravan,
                    time_ms,
                );
            }
            if order.units == 0 {
                self.orders.remove(&id);
                self.containers.remove(&id);
                self.batches.remove(&id);
            } else {
                self.orders.insert(id, order);
            }
        }
        Ok(Purchase {
            units: plan.units,
            coins: plan.coins,
            payments: payments
                .into_iter()
                .map(|(id, coins)| (AgentId(id), coins))
                .collect(),
        })
    }
    pub fn caravan_units(&self, good: Good) -> Quantity {
        self.orders()
            .filter(|order| order.seller == MarketParty::Caravan && order.good == good)
            .map(|order| order.units)
            .sum()
    }

    #[cfg(test)]
    pub(crate) fn caravans(
        &mut self,
        place: PlaceId,
        time_ms: u64,
    ) -> Result<Vec<(AgentId, Coins)>, SimulationError> {
        let limits = Good::ALL.map(|_| Quantity::MAX);
        self.caravans_with_limits(place, time_ms, limits)
    }

    pub(crate) fn caravans_with_limits(
        &mut self,
        place: PlaceId,
        time_ms: u64,
        export_limits: [Quantity; Good::COUNT],
    ) -> Result<Vec<(AgentId, Coins)>, SimulationError> {
        self.remove_ineligible_caravan_orders();
        let mut payments = std::collections::BTreeMap::<uuid::Uuid, Coins>::new();
        for good in Good::ALL {
            let export_threshold = self.effective_export_threshold(good);
            if self.policy.exports[good as usize].allowed && export_threshold.is_some() {
                let mut sellers = std::collections::BTreeMap::<uuid::Uuid, Quantity>::new();
                for order in self.orders().filter(|order| {
                    order.good == good
                        && export_threshold.is_some_and(|threshold| order.quoted_price < threshold)
                }) {
                    if let Some(seller) = order.seller.account() {
                        *sellers.entry(seller.0).or_default() += order.units;
                    }
                }
                let total: Quantity = sellers.values().sum();
                let target = total.div_ceil(2).min(export_limits[good as usize]);
                let mut allocations: Vec<_> = sellers
                    .into_iter()
                    .map(|(seller, stock)| {
                        let numerator = u128::from(stock) * u128::from(target);
                        (
                            seller,
                            (numerator / u128::from(total)) as Quantity,
                            numerator % u128::from(total),
                        )
                    })
                    .collect();
                let assigned: Quantity = allocations.iter().map(|(_, units, _)| units).sum();
                allocations.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
                for allocation in allocations.iter_mut().take((target - assigned) as usize) {
                    allocation.1 += 1;
                }
                allocations.sort_by_key(|allocation| allocation.0);
                for (seller, mut remaining, _) in allocations {
                    let mut orders: Vec<_> = self
                        .orders()
                        .filter(|order| {
                            order.seller.account() == Some(AgentId(seller))
                                && order.good == good
                                && export_threshold
                                    .is_some_and(|threshold| order.quoted_price < threshold)
                        })
                        .cloned()
                        .collect();
                    orders.sort_by_key(|order| order.id.0);
                    let mut value = 0.0;
                    let mut charged = 0;
                    let mut received: Coins = 0;
                    for mut order in orders {
                        let rate = self.policy.export_rate(good);
                        let mut low = 0;
                        let mut high = remaining.min(order.units);
                        while low < high {
                            let middle = low + (high - low).div_ceil(2);
                            let net = charge(
                                value
                                    + middle as f64 * order.quoted_price
                                        / good.units_per_price_unit() as f64,
                            )?;
                            let coins = export_payment(
                                net,
                                charged,
                                received,
                                rate,
                                self.export_tariff_carry[good as usize],
                            )?;
                            let mut carry = self.export_tariff_carry[good as usize];
                            let tariff = tariff_share(coins, rate, &mut carry)?;
                            if self
                                .treasury_budget
                                .0
                                .checked_add(tariff)
                                .is_some_and(|balance| balance >= 0)
                            {
                                low = middle;
                            } else {
                                high = middle - 1;
                            }
                        }
                        let units = low;
                        if units == 0 {
                            continue;
                        }
                        value +=
                            units as f64 * order.quoted_price / good.units_per_price_unit() as f64;
                        let coins = export_payment(
                            charge(value)?,
                            charged,
                            received,
                            rate,
                            self.export_tariff_carry[good as usize],
                        )?;
                        charged = charged
                            .checked_add(coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        let tariff_coins = tariff_share(
                            coins,
                            self.policy.export_rate(good),
                            &mut self.export_tariff_carry[good as usize],
                        )?;
                        self.treasury_budget.0 = self
                            .treasury_budget
                            .0
                            .checked_add(tariff_coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        let recipient_coins = coins
                            .checked_sub(tariff_coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        received = received
                            .checked_add(recipient_coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        remaining -= units;
                        order.units -= units;
                        if order.units == 0 {
                            self.orders.remove(&order.id);
                            self.containers.remove(&order.id);
                            self.batches.remove(&order.id);
                        } else {
                            self.orders.insert(order.id, order.clone());
                        }
                        let activity = &mut self.period_activity[good as usize];
                        activity.traded_units = activity
                            .traded_units
                            .checked_add(units)
                            .ok_or(SimulationError::WealthOverflow)?;
                        activity.traded_coins = activity
                            .traded_coins
                            .checked_add(coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        activity.exported_receipts = activity
                            .exported_receipts
                            .checked_add(recipient_coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        activity.export_tariff_coins = activity
                            .export_tariff_coins
                            .checked_add(tariff_coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        activity.exported_units = activity
                            .exported_units
                            .checked_add(units)
                            .ok_or(SimulationError::WealthOverflow)?;
                        activity.exported_coins = activity
                            .exported_coins
                            .checked_add(coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        self.trades.push_back(Trade {
                            order_id: order.id,
                            place: order.place,
                            time_ms,
                            buyer: MarketParty::Caravan,
                            seller: order.seller,
                            good,
                            units,
                            coins,
                            quoted_price: order.quoted_price,
                            tariff_coins,
                            recipient_coins,
                        });
                        self.record_trial_sale(order.id, units, recipient_coins, false, time_ms);
                    }
                    let payment = payments.entry(seller).or_default();
                    *payment = payment
                        .checked_add(received)
                        .ok_or(SimulationError::WealthOverflow)?;
                }
            }
            if let Some(import_quote) = self.effective_import_threshold(good) {
                let wanted = self
                    .affordable_unmet_at_quote(good, import_quote)
                    .div_ceil(2);
                let units = wanted;
                if units == 0 {
                    continue;
                }
                self.orders()
                    .filter(|order| order.good == good)
                    .map(|order| order.units)
                    .try_fold(units, Quantity::checked_add)
                    .ok_or(SimulationError::WealthOverflow)?;
                let id = OrderId(self.next_order_id);
                self.next_order_id = self
                    .next_order_id
                    .checked_add(1)
                    .ok_or(SimulationError::TimeOverflow)?;
                self.orders.insert(
                    id,
                    SellOrder {
                        id,
                        seller: MarketParty::Caravan,
                        place,
                        good,
                        units,
                        quoted_price: self.effective_import_threshold(good).unwrap(),
                    },
                );
                self.period_activity[good as usize].imported_units = units;
                self.period_activity[good as usize].imported_quoted_value +=
                    units as f64 * import_quote / good.units_per_price_unit() as f64;
            }
        }
        Ok(payments
            .into_iter()
            .map(|(id, coins)| (AgentId(id), coins))
            .collect())
    }

    pub(crate) fn update(&mut self, time_ms: u64) -> Result<(), SimulationError> {
        if time_ms < UPDATE_TIME_MS {
            return Ok(());
        }
        let boundary = UPDATE_TIME_MS + (time_ms - UPDATE_TIME_MS) / DAY_MS * DAY_MS;
        if boundary <= self.period_start_ms {
            return Ok(());
        }
        let mut remaining = self.period_activity.map(|activity| activity.traded_units);
        let mut quoted_values = [0.0; Good::COUNT];
        // Period volumes distinguish trades on either side of a close sharing its timestamp.
        for trade in self.trades.iter().rev() {
            if remaining.iter().all(|units| *units == 0) {
                break;
            }
            let index = trade.good as usize;
            if remaining[index] == 0 {
                continue;
            }
            remaining[index] = remaining[index]
                .checked_sub(trade.units)
                .ok_or(SimulationError::InvalidQuantity)?;
            quoted_values[index] += trade.units as f64 * trade.quoted_price;
        }
        if remaining.iter().any(|units| *units != 0) {
            return Err(SimulationError::InvalidQuantity);
        }
        for good in Good::ALL {
            let traded = self.period_activity[good as usize];
            let before = self.prices.price(good).unwrap();
            let supply: Quantity = self
                .orders()
                .filter(|o| self.purchasable(o) && o.good == good)
                .map(|o| o.units)
                .sum();
            let unmet: Quantity = self
                .affordable_requests
                .values()
                .map(|list| list.units(good))
                .sum();
            if traded.traded_units == 0 && traded.imported_units == 0 && supply == 0 && unmet == 0 {
                continue;
            }
            let accepted_local_price = (traded.local_traded_units > 0).then(|| {
                (traded.local_quoted_value / traded.local_traded_units as f64
                    * good.units_per_price_unit() as f64)
                    .max(MIN_QUOTED_PRICE)
            });
            if let Some(accepted) = accepted_local_price {
                self.last_local_prices[good as usize] = Some((boundary, accepted));
            }
            let after = if traded.traded_units > 0 {
                let accepted = quoted_values[good as usize] / traded.traded_units as f64;
                (before * 0.75 + accepted * 0.25).max(MIN_QUOTED_PRICE)
            } else {
                before
            };
            self.prices = self.prices.with_price(good, after)?;
            self.history.push_back(DailyMarketActivity {
                local_quoted_value: traded.local_quoted_value,
                imported_quoted_value: traded.imported_quoted_value,
                accepted_local_price,
                start_ms: self.period_start_ms,
                end_ms: boundary,
                good,
                traded_units: traded.traded_units,
                traded_coins: traded.traded_coins,
                remaining_supply_units: supply,
                caravan_remaining_supply_units: self.caravan_units(good),
                local_traded_units: traded.local_traded_units,
                local_traded_coins: traded.local_traded_coins,
                exported_units: traded.exported_units,
                exported_coins: traded.exported_coins,
                exported_receipts: traded.exported_receipts,
                export_tariff_coins: traded.export_tariff_coins,
                import_tariff_coins: traded.import_tariff_coins,
                imported_units: traded.imported_units,
                caravan_purchased_units: traded.caravan_purchased_units,
                caravan_purchased_coins: traded.caravan_purchased_coins,

                unmet_demand_units: unmet,
                price_before: before,
                price_after: after,
            });
        }
        self.update_production_trials(boundary);
        let mut recent_movement =
            std::collections::HashMap::<(AgentId, Good, PlaceId), Quantity>::new();
        for trade in self
            .trades
            .iter()
            .rev()
            .take_while(|trade| trade.time_ms >= boundary.saturating_sub(2 * DAY_MS))
        {
            if let Some(account) = trade.seller.account() {
                let movement = recent_movement
                    .entry((account, trade.good, trade.place))
                    .or_default();
                *movement = movement.saturating_add(trade.units);
            }
        }
        let keys: Vec<_> = self.seller_states.keys().copied().collect();
        for (account, good, place) in keys {
            let key = (account, good, place);
            let stock: Quantity = self
                .orders()
                .filter(|order| {
                    order.seller.account() == Some(account)
                        && order.good == good
                        && order.place == place
                        && !self.order_is_trial(order.id)
                })
                .map(|order| order.units)
                .sum();
            if stock == 0 {
                continue;
            }
            let movement = recent_movement.get(&key).copied().unwrap_or(0);
            let markdown = if boundary
                >= self.seller_states[&key]
                    .first_listed_ms
                    .saturating_add(2 * DAY_MS)
            {
                f64::from(self.seller_rates[&account]) / 100.0
                    * (1.0 - movement as f64 / stock as f64).max(0.0)
            } else {
                0.0
            };
            let quote =
                (self.seller_states[&key].quoted_price * (1.0 - markdown)).max(MIN_QUOTED_PRICE);
            self.seller_states.get_mut(&key).unwrap().quoted_price = quote;
            for id in self.orders.keys().copied().collect::<Vec<_>>() {
                if self.order_is_trial(id) {
                    continue;
                }
                let order = self.orders.get_mut(&id).unwrap();
                if order.seller.account() == Some(account)
                    && order.good == good
                    && order.place == place
                    && self
                        .batches
                        .get(&id)
                        .is_some_and(|batch| batch.listed_at_ms < boundary)
                {
                    order.quoted_price = order.quoted_price.min((order.quoted_price + quote) / 2.0);
                }
            }
        }
        self.batches.retain(|id, _| self.orders.contains_key(id));
        self.clock_ms = boundary;
        self.period_activity = [GoodActivity::default(); Good::COUNT];
        self.period_start_ms = boundary;
        Ok(())
    }
}

fn gross_up(net: Coins, rate: i64) -> Result<Coins, SimulationError> {
    let denominator = 10_000_i128 - i128::from(rate);
    if denominator <= 0 {
        return Err(SimulationError::InvalidTaxRate);
    }
    let numerator = i128::from(net) * 10_000;
    Coins::try_from((numerator + denominator - 1) / denominator)
        .map_err(|_| SimulationError::WealthOverflow)
}

fn export_payment(
    asking: Coins,
    charged: Coins,
    received: Coins,
    rate: i64,
    carry: i16,
) -> Result<Coins, SimulationError> {
    if rate >= 0 {
        return Ok(gross_up(asking, rate)? - charged);
    }
    // Earlier subsidy rounding can already cover part of the next order's asking amount.
    let needed = asking.saturating_sub(received).max(0);
    // Below `needed`, coverage requires coins * (10_000 - rate) >= needed * 10_000 + carry.
    // Paying `needed` itself always covers the ask, even if positive carry delays the subsidy.
    let numerator = i128::from(needed) * 10_000 + i128::from(carry);
    let denominator = 10_000 - i128::from(rate);
    let subsidized = ((numerator + denominator - 1) / denominator).max(0);
    Ok(needed.min(subsidized as Coins))
}

// Signed carry preserves fractional taxes and subsidies across split orders and policy changes.
fn tariff_share(gross: Coins, rate: i64, carry: &mut i16) -> Result<Coins, SimulationError> {
    let numerator = i128::from(gross) * i128::from(rate) + i128::from(*carry);
    let tariff =
        Coins::try_from(numerator / 10_000).map_err(|_| SimulationError::WealthOverflow)?;
    *carry = (numerator % 10_000) as i16;
    Ok(tariff)
}

fn charge(cost: f64) -> Result<Coins, SimulationError> {
    if !cost.is_finite() || cost < 0.0 || cost.ceil() >= 9_223_372_036_854_775_808.0 {
        return Err(SimulationError::WealthOverflow);
    }
    Ok(cost.ceil() as Coins)
}

#[derive(Default)]
struct FillPlan {
    treasury_budget: Coins,
    import_carry: i16,
    units: Quantity,
    coins: Coins,
    fills: Vec<(OrderId, Quantity, Coins)>,
}

pub(crate) fn until_update(time_ms: u64) -> u64 {
    let time_of_day = time_ms % DAY_MS;
    if time_of_day < UPDATE_TIME_MS {
        UPDATE_TIME_MS - time_of_day
    } else {
        DAY_MS - time_of_day + UPDATE_TIME_MS
    }
}

impl Market {
    pub(crate) fn prune_history(&mut self, now: u64) {
        let trades_cutoff = now.saturating_sub(30 * DAY_MS);
        while self
            .trades
            .front()
            .is_some_and(|trade| trade.time_ms < trades_cutoff)
        {
            self.trades.pop_front();
        }
        let history_cutoff = now.saturating_sub(365 * DAY_MS);
        while self
            .history
            .front()
            .is_some_and(|day| day.start_ms < history_cutoff)
        {
            if let Some(day) = self.history.pop_front()
                && let Some(quote) = day.accepted_local_price
            {
                self.retained_local_prices[day.good as usize] = Some((day.end_ms, quote));
            }
        }
    }

    pub(crate) fn validate_save(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        self.validate_production_trial_state()?;
        self.policy
            .validate()
            .map_err(|_| SaveError::InvalidState("invalid caravan policy"))?;
        if self.containers.len() != self.orders.len()
            || self
                .containers
                .keys()
                .any(|id| !self.orders.contains_key(id))
        {
            return Err(SaveError::InvalidState("market container binding mismatch"));
        }
        if self
            .import_tariff_carry
            .into_iter()
            .chain(self.export_tariff_carry)
            .any(|carry| !(-9_999..=9_999).contains(&carry))
            || self.orders.iter().any(|(id, order)| {
                *id != order.id
                    || order.units == 0
                    || id.0 >= self.next_order_id
                    || !order.quoted_price.is_finite()
                    || order.quoted_price <= 0.0
                    || (order.seller != MarketParty::Caravan
                        && order.quoted_price < MIN_QUOTED_PRICE)
            })
        {
            return Err(SaveError::InvalidState(
                "invalid market order or tariff carry",
            ));
        }
        if self.batches.iter().any(|(id, batch)| {
            batch.quantity == 0
                || batch.listed_at_ms > self.clock_ms
                || self
                    .order(*id)
                    .is_none_or(|order| order.seller.account().is_none())
                || batch
                    .local_sales_by_price
                    .iter()
                    .any(|(price, _)| !price.is_finite() || *price < MIN_QUOTED_PRICE)
        }) || self
            .orders()
            .any(|order| order.seller.account().is_some() && !self.batches.contains_key(&order.id))
        {
            return Err(SaveError::InvalidState("invalid listing batch"));
        }
        if self
            .seller_rates
            .values()
            .any(|rate| !(1..=10).contains(rate))
            || self.seller_states.iter().any(|((account, _, _), state)| {
                !self.seller_rates.contains_key(account)
                    || !state.quoted_price.is_finite()
                    || state.quoted_price < MIN_QUOTED_PRICE
                    || state
                        .established_price
                        .is_some_and(|quote| !quote.is_finite() || quote < MIN_QUOTED_PRICE)
            })
            || self
                .requests
                .values()
                .chain(self.affordable_requests.values())
                .any(|list| !list.validate_bounds())
        {
            return Err(SaveError::InvalidState(
                "invalid seller pricing state or purchase bounds",
            ));
        }
        if self
            .last_local_prices
            .iter()
            .chain(self.retained_local_prices.iter())
            .flatten()
            .any(|(_, quote)| !quote.is_finite() || *quote < MIN_QUOTED_PRICE)
            || self.period_activity.iter().any(|activity| {
                !activity.local_quoted_value.is_finite()
                    || activity.local_quoted_value < 0.0
                    || !activity.imported_quoted_value.is_finite()
                    || activity.imported_quoted_value < 0.0
            })
            || self.history.iter().any(|day| {
                day.accepted_local_price
                    .is_some_and(|quote| !quote.is_finite() || quote < MIN_QUOTED_PRICE)
                    || !day.local_quoted_value.is_finite()
                    || day.local_quoted_value < 0.0
                    || !day.imported_quoted_value.is_finite()
                    || day.imported_quoted_value < 0.0
            })
            || self.demand_budgets.values().any(|budget| *budget < 0)
        {
            return Err(SaveError::InvalidState(
                "invalid accepted prices or demand budgets",
            ));
        }
        for good in Good::ALL {
            self.prices
                .with_price(good, self.prices.price(good).unwrap_or(f64::NAN))
                .map_err(|_| SaveError::InvalidState("invalid market price"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Market {
        fn list_reference(
            &mut self,
            seller: impl Into<MarketParty>,
            place: PlaceId,
            good: Good,
            units: Quantity,
        ) -> Result<(), SimulationError> {
            if units == 0 {
                return Ok(());
            }
            self.list_at_quote(seller, place, good, units, self.prices.price(good).unwrap())?;
            Ok(())
        }
    }

    const PLACE: PlaceId = PlaceId(uuid::Uuid::nil());

    #[test]
    fn affordability_reuses_unchanged_accounts_and_recalculates_changed_inputs() {
        let first = AgentId(uuid::Uuid::new_v4());
        let second = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        for buyer in [first, second] {
            market
                .set_request(buyer, ShoppingList::single(Good::Flour, 1000))
                .unwrap();
        }
        market.refresh_affordability(&[(first, 10), (second, 20)]);
        assert_eq!(market.affordability_cache.1, 2);
        let expected = market.affordable_requests.clone();
        market.affordable_requests.clear();
        market.refresh_affordability(&[(first, 10), (second, 20)]);
        assert_eq!(market.affordability_cache.1, 2);
        assert_eq!(market.affordable_requests, expected);

        market
            .set_request(first, ShoppingList::single(Good::Water, 500))
            .unwrap();
        market.refresh_affordability(&[(first, 10), (second, 20)]);
        assert_eq!(market.affordability_cache.1, 3);
        assert_eq!(market.affordable_request(first).units(Good::Flour), 0);
        market.refresh_affordability(&[(first, 0), (second, 20)]);
        assert_eq!(market.affordability_cache.1, 4);
        assert_eq!(market.affordable_request(first), ShoppingList::default());
        market.refresh_affordability(&[(second, 20)]);
        assert_eq!(market.affordability_cache.1, 4);
        assert!(!market.demand_budgets.contains_key(&first));
        assert!(
            !market
                .affordability_cache
                .0
                .as_ref()
                .unwrap()
                .accounts
                .contains_key(&first)
        );
        market.refresh_affordability(&[(first, 10), (second, 20)]);
        assert_eq!(market.affordability_cache.1, 5);
        market.set_request(first, ShoppingList::default()).unwrap();
        market.refresh_affordability(&[(first, 10), (second, 20)]);
        assert_eq!(market.affordability_cache.1, 5);
        assert_eq!(market.affordable_request(first), ShoppingList::default());
        market.refresh_affordability(&[(MarketParty::Household(second), 20)]);
        assert_eq!(market.affordability_cache.1, 6);
    }

    #[test]
    fn affordability_invalidates_for_every_shared_market_input() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .set_request(buyer, ShoppingList::single(Good::Flour, 1000))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2)]);
        let check = |market: &mut Market| {
            let calculations = market.affordability_cache.1;
            let mut uncached = market.clone();
            uncached.affordability_cache = AffordabilityCache::default();
            uncached.refresh_affordability(&[(buyer, 2)]);
            market.refresh_affordability(&[(buyer, 2)]);
            assert_eq!(market.affordability_cache.1, calculations + 1);
            assert_eq!(market.affordable_requests, uncached.affordable_requests);
            assert_eq!(market.demand_budgets, uncached.demand_budgets);
        };
        let order = market
            .list_at_quote(MarketParty::Caravan, PLACE, Good::Flour, 500, 1.0)
            .unwrap();
        check(&mut market);
        market.orders.get_mut(&order).unwrap().units = 100;
        check(&mut market);
        market.orders.get_mut(&order).unwrap().quoted_price = 100.0;
        check(&mut market);
        market.policy.import_tariff_basis_points = 10_000;
        check(&mut market);
        market.policy.import_tariff_basis_points = 0;
        check(&mut market);
        market.prices = market.prices.with_price(Good::Flour, 100.0).unwrap();
        check(&mut market);
        market.orders.remove(&order);
        check(&mut market);
    }

    #[test]
    fn affordability_cache_is_safe_across_clones_and_save_roundtrips() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .set_request(buyer, ShoppingList::single(Good::Flour, 1000))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2)]);
        let saved = ron::to_string(&market).unwrap();
        let mut restored: Market = ron::from_str(&saved).unwrap();
        assert_eq!(restored, market);
        assert!(restored.affordability_cache.0.is_none());
        restored.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(restored, market);
        assert_eq!(restored.affordability_cache.1, 1);
        assert_eq!(ron::to_string(&restored).unwrap(), saved);
        let mut speculative = market.clone();
        speculative.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(speculative.affordability_cache.1, 1);
        speculative.prices = speculative.prices.with_price(Good::Flour, 100.0).unwrap();
        speculative.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(speculative.affordability_cache.1, 2);
        speculative
            .list_at_quote(MarketParty::Caravan, PLACE, Good::Flour, 100, 1.0)
            .unwrap();
        speculative.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(speculative.affordability_cache.1, 3);
        market.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(market.affordability_cache.1, 1);
        assert_eq!(ron::to_string(&market).unwrap(), saved);
    }

    #[test]
    fn save_validation_rejects_missing_or_orphan_containers_and_empty_escrow() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        let id = market
            .list_at_quote(seller, PLACE, Good::Wheat, 10, 1.0)
            .unwrap();
        let object = crate::locations::ObjectId(uuid::Uuid::new_v4());
        market.bind_order(id, object);
        market.validate_save().unwrap();
        let mut invalid = market.clone();
        invalid.containers.remove(&id);
        assert!(invalid.validate_save().is_err());
        let mut invalid = market.clone();
        invalid.bind_order(OrderId(999), object);
        assert!(invalid.validate_save().is_err());
        market.orders.get_mut(&id).unwrap().units = 0;
        assert!(market.validate_save().is_err());
    }

    #[test]
    fn lower_local_sales_do_not_replace_the_established_baseline() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::BreadRoll, 100.0).unwrap();
        market.list(seller, PLACE, Good::BreadRoll, 1).unwrap();
        market.establish_trial_price(seller, PLACE, Good::BreadRoll, 110.0);
        market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 1, 80.0)
            .unwrap();
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 1), 100, 0, true)
            .unwrap();
        assert_eq!(
            market.known_trial_price(seller, PLACE, Good::BreadRoll),
            110.0
        );
        assert_eq!(
            market
                .seller_state(seller, PLACE, Good::BreadRoll)
                .unwrap()
                .quoted_price,
            110.0
        );
        let trial = market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 1, 60.0)
            .unwrap();
        market.mark_trial_order(trial, true);
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 1), 100, 0, true)
            .unwrap();
        assert_eq!(
            market.known_trial_price(seller, PLACE, Good::BreadRoll),
            110.0
        );
    }

    #[test]
    fn trial_ask_does_not_set_the_ordinary_seller_quote() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::BreadRoll, 10.0).unwrap();
        let trial = market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 10, 11.0)
            .unwrap();
        market.mark_trial_order(trial, true);
        market.list(seller, PLACE, Good::BreadRoll, 90).unwrap();
        assert_eq!(market.order(trial).unwrap().quoted_price, 11.0);
        assert_eq!(
            market
                .orders()
                .find(|order| order.id != trial)
                .unwrap()
                .quoted_price,
            10.1
        );
    }

    #[test]
    fn repeated_minimum_price_sales_close_with_valid_reference() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market
            .prices
            .with_price(Good::BreadRoll, MIN_QUOTED_PRICE)
            .unwrap();
        market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 29, MIN_QUOTED_PRICE)
            .unwrap();
        for _ in 0..29 {
            market
                .purchase(buyer, PLACE, (Good::BreadRoll, 1), 1, 0, true)
                .unwrap();
        }
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.prices.price(Good::BreadRoll), Some(MIN_QUOTED_PRICE));
        market.validate_save().unwrap();
    }

    #[test]
    fn imports_do_not_subtract_existing_caravan_supply_twice() {
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let good = Good::BreadRoll;
        let mut market = Market::default();
        market
            .list_at_quote(
                MarketParty::Caravan,
                PLACE,
                good,
                4,
                good.import_threshold(),
            )
            .unwrap();
        market
            .set_request(buyer, ShoppingList::single(good, 10))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        assert_eq!(
            market.affordable_unmet_at_quote(good, good.import_threshold()),
            6
        );
        market.caravans(PLACE, 0).unwrap();
        assert_eq!(market.caravan_units(good), 7);
    }

    #[test]
    fn ordinary_batches_do_not_compound_and_daily_asks_preserve_listing_age() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Berries, 10.0).unwrap();
        market.set_time(UPDATE_TIME_MS + 1);
        market.list(seller, PLACE, Good::Berries, 100).unwrap();
        market.list(seller, PLACE, Good::Berries, 100).unwrap();
        assert!(market.orders().all(|order| order.quoted_price == 10.1));
        market.update(UPDATE_TIME_MS + DAY_MS).unwrap();
        assert!(market.orders().all(|order| order.quoted_price == 10.05));
        market.update(UPDATE_TIME_MS + 2 * DAY_MS).unwrap();
        assert!(market.orders().all(|order| order.quoted_price == 10.025));
        market.update(UPDATE_TIME_MS + 3 * DAY_MS).unwrap();
        let state = market.seller_state(seller, PLACE, Good::Berries).unwrap();
        assert_eq!(state.quoted_price, 9.9);
        assert_eq!(state.first_listed_ms, UPDATE_TIME_MS + 1);
        market.list(seller, PLACE, Good::Berries, 100).unwrap();
        assert!(
            market
                .orders()
                .any(|order| (order.quoted_price - 9.999).abs() < 1e-12)
        );
    }

    #[test]
    fn ordinary_acceptance_counts_half_recipe_batch_at_each_exact_price() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::BreadRoll, 10.0).unwrap();
        let id = market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 100, 12.0)
            .unwrap();
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 4), 100, 0, true)
            .unwrap();
        assert_eq!(
            market.known_trial_price(seller, PLACE, Good::BreadRoll),
            10.0
        );
        market.set_order_price(id, 11.0).unwrap();
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 1), 100, 0, true)
            .unwrap();
        assert_eq!(
            market.known_trial_price(seller, PLACE, Good::BreadRoll),
            10.0
        );
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 4), 100, 0, true)
            .unwrap();
        assert_eq!(
            market.known_trial_price(seller, PLACE, Good::BreadRoll),
            11.0
        );
        market.set_order_price(id, 12.0).unwrap();
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 1), 100, 0, true)
            .unwrap();
        assert_eq!(
            market.known_trial_price(seller, PLACE, Good::BreadRoll),
            12.0
        );
    }

    #[test]
    fn reference_uses_local_quoted_volume_including_boundary_sales() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::BreadRoll, 10.0).unwrap();
        market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 1, 2.1)
            .unwrap();
        market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 3, 3.1)
            .unwrap();
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 4), 20, UPDATE_TIME_MS, true)
            .unwrap();
        market.update(UPDATE_TIME_MS).unwrap();
        assert!((market.prices.price(Good::BreadRoll).unwrap() - 8.2125).abs() < 1e-12);
        let day = market.history().back().unwrap();
        assert_eq!(day.local_traded_coins, 12);
        assert_eq!(day.accepted_local_price, Some(2.85));
    }

    #[test]
    fn reference_weights_all_trade_directions_and_survives_save_at_close() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let good = Good::BreadRoll;
        let mut market = Market::default();
        market.prices = market.prices.with_price(good, 10.0).unwrap();
        market
            .set_caravan_policy(CaravanPolicy {
                import_tariff_basis_points: 5000,
                export_tariff_basis_points: 3333,
                ..CaravanPolicy::default()
            })
            .unwrap();
        market.list_at_quote(seller, PLACE, good, 1, 2.1).unwrap();
        market
            .purchase(buyer, PLACE, (good, 1), 100, UPDATE_TIME_MS, true)
            .unwrap();
        market.list_at_quote(seller, PLACE, good, 4, 1.1).unwrap();
        market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(market.period_activity[good as usize].exported_units, 2);
        market.withdraw(seller, PLACE, good, 2).unwrap();
        market
            .list_at_quote(MarketParty::Caravan, PLACE, good, 3, 20.1)
            .unwrap();
        market
            .purchase(buyer, PLACE, (good, 3), 100, UPDATE_TIME_MS, true)
            .unwrap();
        let encoded = ron::to_string(&market).unwrap();
        let mut market: Market = ron::from_str(&encoded).unwrap();
        market.update(UPDATE_TIME_MS).unwrap();
        let expected = 7.5 + (2.1 + 2.0 * 1.1 + 3.0 * 20.1) / 6.0 * 0.25;
        assert!((market.prices.price(good).unwrap() - expected).abs() < 1e-12);
        assert_eq!(
            market.history().back().unwrap().accepted_local_price,
            Some(2.1)
        );
        assert_eq!(
            market.last_local_prices[good as usize],
            Some((UPDATE_TIME_MS, 2.1))
        );

        market
            .list_at_quote(MarketParty::Caravan, PLACE, good, 1, 30.1)
            .unwrap();
        market
            .purchase(buyer, PLACE, (good, 1), 100, UPDATE_TIME_MS, true)
            .unwrap();
        market.update(UPDATE_TIME_MS + DAY_MS).unwrap();
        assert!(
            (market.prices.price(good).unwrap() - (expected * 0.75 + 30.1 * 0.25)).abs() < 1e-12
        );
        let after = market.prices;
        market.update(UPDATE_TIME_MS + 2 * DAY_MS).unwrap();
        assert_eq!(market.prices, after);
    }

    #[test]
    fn planned_bounds_reject_higher_quotes_and_limit_the_budget() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = MarketParty::Citizen(AgentId(uuid::Uuid::from_u128(2)));
        let mut market = Market::default();
        let id = market
            .list_at_quote(seller, PLACE, Good::BreadRoll, 3, 2.0)
            .unwrap();
        let list = ShoppingList::single(Good::BreadRoll, 3)
            .with_purchase_bounds(&market, buyer, PLACE, 6)
            .unwrap();
        market.set_order_price(id, 3.0).unwrap();
        let fill = market
            .purchase_at_quote(
                buyer,
                PLACE,
                (Good::BreadRoll, 3),
                (list.spending_budget(), list.quote_ceiling(Good::BreadRoll)),
                0,
                true,
            )
            .unwrap();
        assert_eq!(fill.units, 0);
        market.set_order_price(id, 1.0).unwrap();
        let fill = market
            .purchase_at_quote(
                buyer,
                PLACE,
                (Good::BreadRoll, 3),
                (2, list.quote_ceiling(Good::BreadRoll)),
                0,
                true,
            )
            .unwrap();
        assert_eq!((fill.units, fill.coins), (2, 2));
    }

    #[test]
    fn caravans_filter_quotes_and_import_only_affordable_missing_supply() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        let good = Good::BreadRoll;
        let cheap = market
            .list_at_quote(seller, PLACE, good, 10, good.export_threshold() * 0.5)
            .unwrap();
        let expensive = market
            .list_at_quote(seller, PLACE, good, 10, good.export_threshold() * 2.0)
            .unwrap();
        market.caravans(PLACE, 0).unwrap();
        assert_eq!(market.order(cheap).unwrap().units, 5);
        assert_eq!(market.order(expensive).unwrap().units, 10);
        market
            .set_request(buyer, ShoppingList::single(good, 10))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        assert_eq!(
            market.affordable_unmet_at_quote(good, good.import_threshold()),
            0
        );
        let mut empty = Market::default();
        empty
            .set_request(buyer, ShoppingList::single(good, 10))
            .unwrap();
        let import_price = good.import_threshold();
        empty.refresh_affordability(&[(buyer, (import_price * 2.0).ceil() as Coins)]);
        assert_eq!(empty.affordable_unmet_at_quote(good, import_price), 2);
        empty.caravans(PLACE, 0).unwrap();
        assert_eq!(empty.caravan_units(good), 1);
    }

    #[test]
    fn household_trades_exclude_own_stock_and_round_by_owner() {
        let seller_id = AgentId(uuid::Uuid::from_u128(1));
        let buyer_id = AgentId(uuid::Uuid::from_u128(2));
        let seller = MarketParty::Household(seller_id);
        let buyer = MarketParty::Household(buyer_id);
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Berries, 5.0).unwrap();
        market
            .list_reference(seller, PLACE, Good::Berries, 100)
            .unwrap();
        market
            .list_reference(buyer, PLACE, Good::Berries, 500)
            .unwrap();
        market
            .list_reference(seller, PLACE, Good::Berries, 100)
            .unwrap();
        market
            .set_request(buyer_id, ShoppingList::single(Good::Berries, 200))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1)]);
        assert_eq!(
            market.affordable_request(buyer_id).units(Good::Berries),
            200
        );
        assert_eq!(market.available_units_at(buyer, PLACE, Good::Berries), 200);
        assert_eq!(
            market.purchase_cost_at(buyer, PLACE, Good::Berries, 200),
            Some(1)
        );
        let purchase = market
            .purchase(buyer, PLACE, (Good::Berries, 200), 1, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (200, 1));
        assert_eq!(purchase.payments, vec![(seller_id, 1)]);
        assert_eq!(market.requested(buyer_id).units(Good::Berries), 0);
        assert_eq!(market.listed_units(buyer, Good::Berries), 500);
        assert!(
            market
                .trades()
                .iter()
                .all(|trade| trade.buyer == buyer && trade.seller == seller)
        );
        assert_eq!(
            market.withdraw(buyer, PLACE, Good::Berries, 200).unwrap(),
            200
        );
    }

    #[test]
    fn caravan_exports_pay_household_accounts_and_preserve_party() {
        let id = AgentId(uuid::Uuid::from_u128(1));
        let seller = MarketParty::Household(id);
        let mut market = Market::default();
        market.prices = market
            .prices
            .with_price(Good::BreadRoll, Good::BreadRoll.export_threshold() * 0.5)
            .unwrap();
        market
            .list_reference(seller, PLACE, Good::BreadRoll, 4)
            .unwrap();
        let payments = market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].0, id);
        assert!(payments[0].1 > 0);
        assert_eq!(market.listed_units(seller, Good::BreadRoll), 2);
        assert!(
            market
                .trades()
                .iter()
                .all(|trade| trade.buyer == MarketParty::Caravan && trade.seller == seller)
        );
    }

    #[test]
    fn daily_pruning_accounts_for_new_trades_before_removing_old_ones() {
        use crate::locations::{Location, Map};
        use crate::{AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe};
        let boundary = UPDATE_TIME_MS + 31 * DAY_MS;
        let mut world = Universe::with_map(Map::default());
        world.current_time_ms = boundary - 2 * TRADE_DURATION_MS;
        world.market = Market::starting_at(world.current_time_ms);
        let (world, seller) = world
            .with_citizen(
                "Seller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_good(Good::BreadRoll, 10)
                    .unwrap(),
            )
            .unwrap();
        let (world, buyer) = world
            .with_citizen(
                "Buyer",
                Citizen::new(0.0).unwrap().with_coins(1000).unwrap(),
            )
            .unwrap();
        let market = world.map().public_place(Location::Market);
        let mut world = world
            .start_action(seller, CitizenAction::Travel(market))
            .unwrap()
            .start_action(seller, CitizenAction::List(Good::BreadRoll, 5))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        world.market.trades.push_back(Trade {
            order_id: OrderId(0),
            place: market,
            time_ms: 0,
            buyer: MarketParty::Citizen(buyer),
            seller: MarketParty::Citizen(seller),
            good: Good::BreadRoll,
            units: 1,
            coins: 1,
            quoted_price: 1.0,
            tariff_coins: 0,
            recipient_coins: 1,
        });
        let world = world
            .start_action(buyer, CitizenAction::Travel(market))
            .unwrap()
            .start_action(
                buyer,
                CitizenAction::Buy(ShoppingList::single(Good::BreadRoll, 2)),
            )
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(world.current_time_ms(), boundary);
        assert!(world.market.trades.iter().all(|trade| trade.time_ms > 0));
        assert_eq!(
            world
                .household_history(world.economic_id(buyer).unwrap())
                .unwrap()
                .completed
                .back()
                .unwrap()
                .bought[Good::BreadRoll as usize],
            2
        );
        assert_eq!(
            world
                .household_history(world.economic_id(seller).unwrap())
                .unwrap()
                .completed
                .back()
                .unwrap()
                .sold[Good::BreadRoll as usize],
            2
        );
        let AgentKind::Citizen(citizen) = &world.agents[&buyer].kind;
        assert_eq!(citizen.units(Good::BreadRoll), 2);
    }

    #[test]
    fn history_retention_keeps_thirty_days_of_trades_and_365_complete_market_days() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_reference(seller, PLACE, Good::BreadRoll, 1000)
            .unwrap();
        for day in 0..=366 {
            let boundary = UPDATE_TIME_MS + day * DAY_MS;
            market
                .purchase(buyer, PLACE, (Good::BreadRoll, 1), 1000, boundary, true)
                .unwrap();
            market.update(boundary).unwrap();
            market.prune_history(boundary);
        }
        let now = UPDATE_TIME_MS + 366 * DAY_MS;
        assert_eq!(market.trades.len(), 31);
        assert_eq!(market.trades.front().unwrap().time_ms, now - 30 * DAY_MS);
        assert_eq!(market.trades.back().unwrap().time_ms, now);
        assert_eq!(market.history.len(), 365);
        assert_eq!(market.history.front().unwrap().start_ms, now - 365 * DAY_MS);
        assert_eq!(market.history.back().unwrap().end_ms, now);
        assert!(market.listed_units(seller, Good::BreadRoll) > 0);
        assert_eq!(
            market.previous_period().unwrap().goods[Good::BreadRoll as usize].traded_units,
            1
        );
    }

    #[test]
    fn universal_goods_block_caravans_and_keep_local_orders_purchasable() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        for good in [Good::Berries, Good::Water] {
            let mut market = Market::default();
            assert!(!market.caravan_policy().exports[good as usize].allowed);
            assert_eq!(market.effective_export_threshold(good), None);
            assert_eq!(market.effective_import_threshold(good), None);
            market.prices = market
                .prices
                .with_price(good, good.core_price() / 10.0)
                .unwrap();
            market.list_reference(seller, PLACE, good, 1000).unwrap();
            let old_id = OrderId(market.next_order_id);
            market.orders.insert(
                old_id,
                SellOrder {
                    id: old_id,
                    seller: MarketParty::Caravan,
                    place: PLACE,
                    good,
                    units: 100,
                    quoted_price: good.import_threshold(),
                },
            );
            assert!(!market.purchasable(&market.orders[&old_id]));
            let mut policy = market.caravan_policy();
            policy.exports[good as usize].allowed = true;
            let mut daily = market.clone();
            assert!(daily.caravans(PLACE, 0).unwrap().is_empty());
            assert_eq!(daily.caravan_units(good), 0);
            assert_eq!(daily.listed_units(seller, good), 1000);
            market.set_caravan_policy(policy).unwrap();
            assert!(!market.caravan_policy().exports[good as usize].allowed);
            assert_eq!(market.caravan_units(good), 0);
            assert_eq!(market.listed_units(seller, good), 1000);
            let bought = market
                .purchase(buyer, PLACE, (good, 10), 100, 0, true)
                .unwrap();
            assert_eq!(bought.units, 10);
            assert_eq!(
                market.trades().back().unwrap().seller,
                MarketParty::Citizen(seller)
            );
            market.prices = market
                .prices
                .with_price(good, good.core_price() * 10.0)
                .unwrap();
            market
                .set_request(buyer, ShoppingList::single(good, 10_000))
                .unwrap();
            market.refresh_affordability(&[(buyer, 100_000)]);
            market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
            assert_eq!(market.caravan_units(good), 0);
            assert_eq!(
                market.current_period().goods[good as usize].imported_units,
                0
            );
        }
        assert!(Good::Fish.caravan_eligible());
    }

    #[test]
    fn expensive_caravan_stock_preserves_price_history_and_purchases() {
        let buyer = AgentId(uuid::Uuid::from_u128(1));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Thread, 10.0049).unwrap();
        let id = OrderId(0);
        market.orders.insert(
            id,
            SellOrder {
                id,
                seller: MarketParty::Caravan,
                place: PLACE,
                good: Good::Thread,
                units: 9600,
                quoted_price: Good::Thread.import_threshold(),
            },
        );
        market.next_order_id = 1;
        market.period_activity[Good::Thread as usize].imported_units = 9600;
        for day in 0..3 {
            market.update(UPDATE_TIME_MS + day * DAY_MS).unwrap();
            assert_eq!(market.prices.price(Good::Thread), Some(10.0049));
            let history = market.history().back().unwrap();
            assert_eq!(history.remaining_supply_units, 9600);
            assert_eq!(history.caravan_remaining_supply_units, 9600);
            assert_eq!(history.imported_units, if day == 0 { 9600 } else { 0 });
            assert_eq!(
                market.previous_period().unwrap().goods[Good::Thread as usize].listed_units,
                9600
            );
            assert_eq!(
                market.current_period().goods[Good::Thread as usize].listed_units,
                9600
            );
            assert_eq!(market.orders[&id].quoted_price, 175.5);
        }
        market
            .set_request(buyer, ShoppingList::single(Good::Thread, 1000))
            .unwrap();
        market.refresh_affordability(&[(buyer, 176)]);
        assert_eq!(market.affordable_request(buyer).units(Good::Thread), 1000);
        assert_eq!(market.available_units(buyer, Good::Thread), 9600);
        let purchase = market
            .purchase(buyer, PLACE, (Good::Thread, 1000), 176, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (1000, 176));
        assert!(purchase.payments.is_empty());
        market.refresh_affordability(&[(buyer, 0)]);
        market.update(UPDATE_TIME_MS + 3 * DAY_MS).unwrap();
        assert_eq!(
            market.prices.price(Good::Thread),
            Some(10.0049 * 0.75 + 175.5 * 0.25)
        );
        let history = market.history().back().unwrap();
        assert_eq!(
            (
                history.traded_units,
                history.caravan_purchased_units,
                history.remaining_supply_units
            ),
            (1000, 1000, 8600)
        );
    }

    #[test]
    fn import_delivery_without_pricing_activity_keeps_price_and_history() {
        let mut market = Market::default();
        market.period_activity[Good::Thread as usize].imported_units = 10;
        let before = market.prices;
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.prices, before);
        let history = market.history().back().unwrap();
        assert_eq!(history.good, Good::Thread);
        assert_eq!(history.imported_units, 10);
        assert_eq!(history.remaining_supply_units, 0);
        assert_eq!(history.price_before, history.price_after);
    }

    #[test]
    fn unfilled_local_and_caravan_quotes_and_unmet_demand_keep_reference() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let good = Good::BreadRoll;
        let mut market = Market::default();
        market.prices = market.prices.with_price(good, 10.0).unwrap();
        market.list_at_quote(seller, PLACE, good, 20, 5.0).unwrap();
        market
            .list_at_quote(MarketParty::Caravan, PLACE, good, 30, 20.0)
            .unwrap();
        market
            .set_request(buyer, ShoppingList::single(good, 10))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.prices.price(good), Some(10.0));
        let day = market.history().back().unwrap();
        assert_eq!(day.remaining_supply_units, 50);
        assert_eq!(day.unmet_demand_units, 10);
        assert_eq!(day.traded_units, 0);
    }

    #[test]
    fn caravan_purchases_move_the_reference_by_executed_quote() {
        for quote in [9.0, 10.0, f64::from_bits(10.0_f64.to_bits() + 1)] {
            let buyer = AgentId(uuid::Uuid::from_u128(1));
            let mut market = Market::default();
            market.prices = market.prices.with_price(Good::BreadRoll, 10.0).unwrap();
            for (id, units, quoted_price) in [(OrderId(0), 30, quote), (OrderId(1), 100, 20.0)] {
                market.orders.insert(
                    id,
                    SellOrder {
                        id,
                        seller: MarketParty::Caravan,
                        place: PLACE,
                        good: Good::BreadRoll,
                        units,
                        quoted_price,
                    },
                );
            }
            market.next_order_id = 2;
            market
                .purchase(buyer, PLACE, (Good::BreadRoll, 10), 1000, 0, true)
                .unwrap();
            market
                .set_request(buyer, ShoppingList::single(Good::BreadRoll, 10))
                .unwrap();
            market.refresh_affordability(&[(buyer, 1000)]);
            market.update(UPDATE_TIME_MS).unwrap();
            assert_eq!(
                market.prices.price(Good::BreadRoll),
                Some(7.5 + quote * 0.25)
            );
            let history = market.history().back().unwrap();
            assert_eq!(
                (
                    history.traded_units,
                    history.remaining_supply_units,
                    history.unmet_demand_units
                ),
                (10, 120, 10)
            );
            assert_eq!(history.caravan_purchased_units, 10);
        }
    }

    #[test]
    fn effective_policies_stack_offset_block_and_reprice_existing_imports() {
        let mut market = Market::default();
        let good = Good::BreadRoll;
        let buyer = AgentId(uuid::Uuid::new_v4());
        market
            .list_at_quote(MarketParty::Caravan, PLACE, good, 10, 100.0)
            .unwrap();
        let mut policy = market.caravan_policy();
        policy.import_tariff_basis_points = 15_000;
        policy.export_tariff_basis_points = -5_000;
        policy.exports[good as usize].import_tariff_basis_points = -17_500;
        policy.exports[good as usize].export_tariff_basis_points = 2_500;
        market.set_caravan_policy(policy).unwrap();
        assert_eq!(
            market.effective_import_threshold(good),
            Some(good.import_threshold() / 1.25)
        );
        assert_eq!(
            market.effective_export_threshold(good),
            Some(good.export_threshold() * 1.25)
        );
        assert_eq!(
            market.orders().next().unwrap().quoted_price,
            good.import_threshold() / 1.25
        );
        policy.exports[good as usize].imports_allowed = false;
        market.set_caravan_policy(policy).unwrap();
        assert_eq!(market.available_units(buyer, good), 0);
        assert_eq!(market.caravan_units(good), 10);
        assert_eq!(
            market
                .purchase(buyer, PLACE, (good, 10), 1000, 0, true)
                .unwrap()
                .units,
            0
        );
        policy.exports[good as usize].imports_allowed = true;
        for rate in [10_000, 15_000] {
            policy.import_tariff_basis_points = rate;
            policy.export_tariff_basis_points = rate;
            policy.exports[good as usize].import_tariff_basis_points = 0;
            policy.exports[good as usize].export_tariff_basis_points = 0;
            market.set_caravan_policy(policy).unwrap();
            assert_eq!(market.effective_import_threshold(good), None);
            assert_eq!(market.effective_export_threshold(good), None);
            assert_eq!(market.available_units(buyer, good), 0);
        }
    }

    #[test]
    fn imports_spend_shared_subsidy_budget_in_planning_and_settlement() {
        let mut market = Market::default();
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut policy = market.caravan_policy();
        policy.import_tariff_basis_points = -2_500;
        market.set_caravan_policy(policy).unwrap();
        market.set_treasury_budget(30);
        for good in [Good::BreadRoll, Good::BerryPie] {
            market
                .list_at_quote(MarketParty::Caravan, PLACE, good, 10, 40.0)
                .unwrap();
        }
        let list = ShoppingList::new([(Good::BreadRoll, 10), (Good::BerryPie, 10)]).unwrap();
        let planned = market
            .affordable_purchase_list_at(buyer.into(), Some(PLACE), list, 1000)
            .unwrap();
        assert_eq!(planned.units(Good::BreadRoll), 3);
        assert_eq!(planned.units(Good::BerryPie), 0);
        let purchase = market
            .purchase(buyer, PLACE, (Good::BreadRoll, 10), 1000, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (3, 120));
        let trade = market.trades().back().unwrap();
        assert_eq!((trade.tariff_coins, trade.recipient_coins), (-30, 150));
        assert_eq!(purchase.coins + 30, trade.recipient_coins);
        assert_eq!(market.treasury_budget.0, 0);
        assert_eq!(
            market
                .purchase(buyer, PLACE, (Good::BerryPie, 1), 1000, 0, true)
                .unwrap()
                .units,
            0
        );
    }

    #[test]
    fn estimated_purchase_cost_prices_treasury_limited_missing_units_at_reference() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::BreadRoll, 10.0).unwrap();
        let mut policy = market.caravan_policy();
        policy.import_tariff_basis_points = -2500;
        market.set_caravan_policy(policy).unwrap();
        market
            .list_at_quote(MarketParty::Caravan, PLACE, Good::BreadRoll, 10, 4.0)
            .unwrap();
        for (budget, filled, expected) in [(0, 0, 100), (3, 3, 82)] {
            market.set_treasury_budget(budget);
            let fill = market
                .fill_plan(buyer.into(), None, Good::BreadRoll, 10, None)
                .unwrap();
            assert_eq!(fill.units, filled);
            assert_eq!(
                market.estimated_purchase_cost(buyer, Good::BreadRoll, 10),
                Some(expected)
            );
        }
    }

    #[test]
    fn exhausted_subsidies_do_not_hide_a_fundable_local_purchase_stop() {
        use crate::locations::{Location, Map, Position};
        let map = Map::default();
        let mut market = Market::default();
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut policy = market.caravan_policy();
        policy.import_tariff_basis_points = -2500;
        market.set_caravan_policy(policy).unwrap();
        let imported = map.public_place(Location::Market);
        let local = map.public_place(Location::Forest);
        market
            .list_at_quote(MarketParty::Caravan, imported, Good::BreadRoll, 1, 4.0)
            .unwrap();
        market
            .list_at_quote(seller, local, Good::BreadRoll, 1, 5.0)
            .unwrap();
        assert_eq!(
            market.lowest_purchase_quote(buyer.into(), Good::BreadRoll, &map),
            Some(5.0)
        );
        assert_eq!(
            market
                .next_purchase_stop(
                    buyer.into(),
                    &map,
                    Position::default(),
                    ShoppingList::single(Good::BreadRoll, 1),
                    Some(10)
                )
                .unwrap()
                .0,
            local
        );
    }

    #[test]
    fn negative_quarter_import_and_export_preserve_share_of_payment_and_conservation() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        let mut policy = market.caravan_policy();
        policy.import_tariff_basis_points = -2_500;
        policy.export_tariff_basis_points = -2_500;
        market.set_caravan_policy(policy).unwrap();
        market.set_treasury_budget(40);
        market
            .list_at_quote(MarketParty::Caravan, PLACE, Good::BreadRoll, 1, 80.0)
            .unwrap();
        let purchase = market
            .purchase(buyer, PLACE, (Good::BreadRoll, 1), 100, 0, true)
            .unwrap();
        assert_eq!(purchase.coins, 80);
        assert_eq!(market.trades().back().unwrap().recipient_coins, 100);
        // Bulk stock permits an exact 100-coin settlement at an eligible per-kilogram quote.
        market
            .list_at_quote(seller, PLACE, Good::Flour, 200_000, 1.0)
            .unwrap();
        assert!(market.effective_export_threshold(Good::Flour).unwrap() > 1.0);
        let payments = market.caravans(PLACE, 0).unwrap();
        assert_eq!(payments, vec![(seller, 100)]);
        let export = market.trades().back().unwrap();
        assert_eq!(
            (export.coins, export.tariff_coins, export.recipient_coins),
            (80, -20, 100)
        );
        assert_eq!(market.treasury_budget.0, 0);
        assert_eq!(80 + 80 + 40, 100 + 100);
    }

    #[test]
    fn exports_partially_fill_using_one_treasury_across_goods_and_tax_settlement() {
        let mut market = Market::default();
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut policy = market.caravan_policy();
        policy.export_tariff_basis_points = -2500;
        market.set_caravan_policy(policy).unwrap();
        market.set_treasury_budget(6);
        for good in [Good::BreadRoll, Good::BerryPie] {
            market.list_at_quote(seller, PLACE, good, 40, 1.0).unwrap();
        }
        let payments = market.caravans(PLACE, 0).unwrap();
        assert_eq!(payments.iter().map(|(_, coins)| *coins).sum::<Coins>(), 33);
        assert_eq!(market.treasury_budget.0, 0);
        let trades: Vec<_> = market.trades().iter().cloned().collect();
        assert_eq!(trades.iter().map(|t| t.tariff_coins).sum::<Coins>(), -6);
        assert_eq!(trades.iter().map(|t| t.coins).sum::<Coins>(), 27);
        assert_eq!(trades.iter().map(|t| t.units).sum::<Quantity>(), 33);
        let mut town = crate::Universe::default();
        town.town_treasury = 6;
        town.tax_sales(&trades, 0).unwrap();
        assert_eq!(town.town_treasury, 0);
        assert_eq!(27 + 6, payments[0].1);
        market.caravans(PLACE, DAY_MS).unwrap();
        assert_eq!(market.treasury_budget.0, 0);
        assert!(
            market
                .trades()
                .iter()
                .skip(trades.len())
                .all(|trade| trade.tariff_coins == 0)
        );
    }

    #[test]
    fn signed_carry_survives_rate_changes_and_fragmented_subsidies() {
        let mut carry = 0;
        assert_eq!(tariff_share(1, -3333, &mut carry).unwrap(), 0);
        assert_eq!(carry, -3333);
        assert_eq!(tariff_share(1, 3333, &mut carry).unwrap(), 0);
        assert_eq!(carry, 0);
        let fragmented: Coins = (0..100)
            .map(|_| tariff_share(1, -3333, &mut carry).unwrap())
            .sum();
        let mut whole_carry = 0;
        assert_eq!(
            fragmented,
            tariff_share(100, -3333, &mut whole_carry).unwrap()
        );
        assert_eq!(carry, whole_carry);
        assert_eq!(fragmented, -33);
    }

    #[test]
    fn old_good_policy_defaults_to_imports_enabled_and_zero_rates() {
        let old: ExportPolicy = ron::from_str("(allowed:false,minimum_reserve:123)").unwrap();
        assert!(!old.allowed);
        assert!(old.imports_allowed);
        assert_eq!(old.import_tariff_basis_points, 0);
        assert_eq!(old.export_tariff_basis_points, 0);
    }

    #[test]
    fn tariffs_preserve_asking_receipts_and_carry_by_good_and_direction() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Flour, 2.0).unwrap();
        let policy = CaravanPolicy {
            export_tariff_basis_points: 3333,
            import_tariff_basis_points: 5000,
            ..CaravanPolicy::default()
        };
        market.set_caravan_policy(policy).unwrap();
        for day in 0..3 {
            market
                .list_reference(seller, PLACE, Good::Flour, 1)
                .unwrap();
            let payments = market
                .caravans(PLACE, UPDATE_TIME_MS + day * DAY_MS)
                .unwrap();
            assert!(payments[0].1 >= 1);
        }
        let exports: Vec<_> = market.trades().iter().collect();
        assert_eq!(exports.iter().map(|trade| trade.coins).sum::<Coins>(), 6);
        assert_eq!(
            exports
                .iter()
                .map(|trade| trade.tariff_coins)
                .sum::<Coins>(),
            1
        );
        assert_eq!(
            exports
                .iter()
                .map(|trade| trade.recipient_coins)
                .sum::<Coins>(),
            5
        );
        assert_eq!(market.export_tariff_carry[Good::Flour as usize], 9998);
        assert_eq!(market.export_tariff_carry[Good::BreadRoll as usize], 0);
        assert_eq!(market.import_tariff_carry[Good::Flour as usize], 0);
        let id = OrderId(market.next_order_id);
        market.orders.insert(
            id,
            SellOrder {
                id,
                seller: MarketParty::Caravan,
                place: PLACE,
                good: Good::Flour,
                units: 1000,
                quoted_price: 3.0,
            },
        );
        market
            .set_request(buyer, ShoppingList::single(Good::Flour, 1000))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(market.affordable_request(buyer).units(Good::Flour), 666);
        assert_eq!(market.purchase_cost(buyer, Good::Flour, 666), Some(2));
        let imported = market
            .purchase(buyer, PLACE, (Good::Flour, 1000), 2, 0, true)
            .unwrap();
        assert_eq!((imported.units, imported.coins), (666, 2));
        let trade = market.trades().back().unwrap();
        assert_eq!((trade.tariff_coins, trade.recipient_coins), (1, 1));
        assert_eq!(market.export_tariff_carry[Good::Flour as usize], 9998);
    }

    #[test]
    fn export_subsidy_payments_are_minimal_and_invariant_under_fragmentation() {
        for rate in [-25_000, -10_000, -3333, -2500, -1] {
            for initial_carry in [-9999, -1, 0, 1, 9998] {
                let mut carry = initial_carry;
                let mut charged = 0;
                let mut received = 0;
                for asking in 1..=40 {
                    let whole = export_payment(asking, 0, 0, rate, initial_carry).unwrap();
                    let brute = (0..=asking)
                        .find(|&coins| {
                            let mut trial = initial_carry;
                            coins - tariff_share(coins, rate, &mut trial).unwrap() >= asking
                        })
                        .unwrap();
                    assert_eq!(
                        whole, brute,
                        "rate={rate}, carry={initial_carry}, ask={asking}"
                    );
                    let coins = export_payment(asking, charged, received, rate, carry).unwrap();
                    let tariff = tariff_share(coins, rate, &mut carry).unwrap();
                    charged += coins;
                    received += coins - tariff;
                    let mut whole_carry = initial_carry;
                    let whole_tariff = tariff_share(whole, rate, &mut whole_carry).unwrap();
                    assert_eq!(
                        (charged, received, carry),
                        (whole, whole - whole_tariff, whole_carry)
                    );
                }
            }
        }
    }

    #[test]
    fn export_subsidies_cover_asking_after_positive_carry_with_partial_treasury() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let mut base = Market::default();
        base.prices = base.prices.with_price(Good::Flour, 2.0).unwrap();
        let mut policy = base.caravan_policy();
        policy.export_tariff_basis_points = 3333;
        base.set_caravan_policy(policy).unwrap();
        for _ in 0..3 {
            base.list_reference(seller, PLACE, Good::Flour, 1).unwrap();
            base.caravans(PLACE, 0).unwrap();
        }
        assert_eq!(base.export_tariff_carry[Good::Flour as usize], 9998);
        policy.export_tariff_basis_points = -2500;
        base.set_caravan_policy(policy).unwrap();
        base.trades.clear();
        base.period_activity = [GoodActivity::default(); Good::COUNT];
        for (stock, budget) in [(10_000, 10), (40_000, 0), (40_000, 1), (40_000, 3)] {
            let mut whole = base.clone();
            whole.set_treasury_budget(budget);
            let mut fragmented = whole.clone();
            whole
                .list_at_quote(seller, PLACE, Good::Flour, stock, 1.0)
                .unwrap();
            for _ in 0..stock / 1000 {
                fragmented
                    .list_at_quote(seller, PLACE, Good::Flour, 1000, 1.0)
                    .unwrap();
            }
            let payments = whole.caravans(PLACE, 0).unwrap();
            assert_eq!(payments, fragmented.caravans(PLACE, 0).unwrap());
            assert_eq!(
                whole.current_period().goods,
                fragmented.current_period().goods
            );
            assert_eq!(whole.export_tariff_carry, fragmented.export_tariff_carry);
            assert_eq!(whole.treasury_budget.0, fragmented.treasury_budget.0);
            let trade = whole.trades().back().unwrap();
            assert!(trade.recipient_coins >= charge(trade.units as f64 / 1000.0).unwrap());
            assert_eq!(trade.coins, trade.recipient_coins + trade.tariff_coins);
            assert_eq!(budget + trade.tariff_coins, whole.treasury_budget.0);
            assert!(whole.treasury_budget.0 >= 0);
            if stock == 10_000 {
                assert_eq!(
                    (trade.units, trade.coins, trade.recipient_coins),
                    (5000, 5, 5)
                );
            } else if budget < 3 {
                assert!(trade.units < stock / 2);
                assert_eq!(whole.treasury_budget.0, 0);
            }
        }
    }

    #[test]
    fn tariff_rounding_does_not_depend_on_export_order_fragmentation() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let mut whole = Market::default();
        whole.prices = whole.prices.with_price(Good::Flour, 2.0).unwrap();
        whole
            .set_caravan_policy(CaravanPolicy {
                export_tariff_basis_points: 3333,
                ..CaravanPolicy::default()
            })
            .unwrap();
        let mut fragmented = whole.clone();
        whole.list_reference(seller, PLACE, Good::Flour, 6).unwrap();
        for _ in 0..6 {
            fragmented
                .list_reference(seller, PLACE, Good::Flour, 1)
                .unwrap();
        }
        assert_eq!(
            whole.caravans(PLACE, 0).unwrap(),
            fragmented.caravans(PLACE, 0).unwrap()
        );
        assert_eq!(
            whole.current_period().goods,
            fragmented.current_period().goods
        );
        assert_eq!(whole.export_tariff_carry, fragmented.export_tariff_carry);
    }

    #[test]
    fn caravan_exports_allocate_between_sellers_before_orders_and_round_payment_once() {
        let first = AgentId(uuid::Uuid::from_u128(1));
        let second = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Flour, 2.0).unwrap();
        for _ in 0..3 {
            market.list_reference(first, PLACE, Good::Flour, 1).unwrap();
        }
        market
            .list_reference(second, PLACE, Good::Flour, 2)
            .unwrap();
        market.update(UPDATE_TIME_MS).unwrap();
        let payments = market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(market.listed_units(first, Good::Flour), 1);
        assert_eq!(market.listed_units(second, Good::Flour), 1);
        assert_eq!(payments, vec![(first, 1), (second, 1)]);
        let current = market.current_period().goods[Good::Flour as usize];
        assert_eq!((current.exported_units, current.exported_coins), (3, 2));
        assert_eq!(current.local_traded_units, 0);
        assert_eq!(
            market.previous_period().unwrap().goods[Good::Flour as usize].exported_units,
            0
        );
        assert!(
            market
                .trades()
                .iter()
                .all(|trade| trade.buyer == MarketParty::Caravan)
        );
    }

    #[test]
    fn equal_sellers_receive_the_same_share_regardless_of_listing_fragmentation() {
        let first = AgentId(uuid::Uuid::from_u128(1));
        let second = AgentId(uuid::Uuid::from_u128(2));
        let mut whole = Market::default();
        whole.prices = whole.prices.with_price(Good::Flour, 2.0).unwrap();
        whole.list_reference(second, PLACE, Good::Flour, 3).unwrap();
        whole.list_reference(first, PLACE, Good::Flour, 3).unwrap();
        let mut fragmented = Market {
            prices: whole.prices,
            ..Market::default()
        };
        for _ in 0..3 {
            fragmented
                .list_reference(second, PLACE, Good::Flour, 1)
                .unwrap();
        }
        fragmented
            .list_reference(first, PLACE, Good::Flour, 3)
            .unwrap();
        let whole_payments = whole.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        let fragmented_payments = fragmented.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(whole_payments, fragmented_payments);
        assert_eq!(fragmented.listed_units(first, Good::Flour), 1);
        assert_eq!(fragmented.listed_units(second, Good::Flour), 2);
        for seller in [first, second] {
            assert_eq!(
                whole.listed_units(seller, Good::Flour),
                fragmented.listed_units(seller, Good::Flour)
            );
        }
    }

    #[test]
    fn caravan_thresholds_are_strict_and_import_prices_survive_repricing() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut exact = Market::default();
        exact.prices = exact
            .prices
            .with_price(Good::BreadRoll, Good::BreadRoll.export_threshold())
            .unwrap();
        exact
            .list_reference(seller, PLACE, Good::BreadRoll, 9)
            .unwrap();
        assert!(exact.caravans(PLACE, UPDATE_TIME_MS).unwrap().is_empty());
        assert_eq!(exact.listed_units(seller, Good::BreadRoll), 9);
        exact.prices = exact
            .prices
            .with_price(Good::BreadRoll, Good::BreadRoll.import_threshold())
            .unwrap();
        exact.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(exact.caravan_units(Good::BreadRoll), 0);
        let mut market = Market::default();
        market.prices = market
            .prices
            .with_price(Good::BreadRoll, Good::BreadRoll.core_price() * 2.0)
            .unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::BreadRoll, 9))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        market.update(UPDATE_TIME_MS).unwrap();
        market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(market.caravan_units(Good::BreadRoll), 5);
        assert_eq!(
            market.current_period().goods[Good::BreadRoll as usize].imported_units,
            5
        );
        assert!(market.trades().is_empty());
        market.refresh_affordability(&[(buyer, 1000)]);
        market.update(UPDATE_TIME_MS + DAY_MS).unwrap();
        market.caravans(PLACE, UPDATE_TIME_MS + DAY_MS).unwrap();
        assert_eq!(market.caravan_units(Good::BreadRoll), 7);
        assert!(
            market
                .orders()
                .all(|order| order.quoted_price == Good::BreadRoll.import_threshold())
        );
        let purchase = market
            .purchase(
                buyer,
                PLACE,
                (Good::BreadRoll, 2),
                1000,
                UPDATE_TIME_MS + DAY_MS,
                true,
            )
            .unwrap();
        assert!(purchase.payments.is_empty());
        assert_eq!(
            purchase.coins,
            charge(Good::BreadRoll.import_threshold() * 2.0).unwrap()
        );
        let activity = market.current_period().goods[Good::BreadRoll as usize];
        assert_eq!(
            (
                activity.local_traded_units,
                activity.caravan_purchased_units,
                activity.imported_units
            ),
            (0, 2, 2)
        );
        let before = market.clone();
        market
            .purchase(buyer, PLACE, (Good::BreadRoll, 1), 1000, 0, false)
            .unwrap();
        assert_eq!(market.trades(), before.trades());
        assert_eq!(
            market.current_period().goods[Good::BreadRoll as usize].caravan_purchased_units,
            2
        );
    }

    #[test]
    fn local_settlement_leaves_remote_supply_and_owner_stock_untouched() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let remote = PlaceId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::BreadRoll, 15.0).unwrap();
        market
            .list_reference(seller, PLACE, Good::BreadRoll, 2)
            .unwrap();
        market
            .list_reference(seller, remote, Good::BreadRoll, 3)
            .unwrap();
        market
            .list_reference(buyer, PLACE, Good::BreadRoll, 7)
            .unwrap();
        assert_eq!(market.available_units(buyer, Good::BreadRoll), 5);
        assert_eq!(market.available_units_at(buyer, PLACE, Good::BreadRoll), 2);
        let purchase = market
            .purchase(buyer, PLACE, (Good::BreadRoll, 10), 1000, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (2, 30));
        assert_eq!(market.available_units_at(buyer, remote, Good::BreadRoll), 3);
        assert_eq!(market.listed_units(buyer, Good::BreadRoll), 7);
        assert_eq!(
            market.withdraw(seller, PLACE, Good::BreadRoll, 10).unwrap(),
            0
        );
        assert_eq!(
            market.current_period().goods[Good::BreadRoll as usize].traded_units,
            2
        );
    }

    #[test]
    fn later_start_has_no_closed_period_until_the_next_four_am_update() {
        let mut market = Market::starting_at(6 * 60 * 60 * 1000);
        let initial = market.current_period();
        assert_eq!(initial.start_ms, UPDATE_TIME_MS);
        assert_eq!(initial.end_ms, UPDATE_TIME_MS + DAY_MS);
        assert!(market.previous_period().is_none());
        market.update(initial.end_ms - 1).unwrap();
        assert!(market.previous_period().is_none());
        market.update(initial.end_ms).unwrap();
        assert_eq!(market.current_period().start_ms, initial.end_ms);
        assert_eq!(market.previous_period(), Some(initial));
        assert!(market.history().is_empty());
    }

    #[test]
    fn period_statistics_count_fills_once_and_close_at_four() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_reference(seller, PLACE, Good::BreadRoll, 10)
            .unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::BreadRoll, 7))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        market
            .purchase(
                buyer,
                PLACE,
                (Good::BreadRoll, 2),
                1000,
                UPDATE_TIME_MS,
                true,
            )
            .unwrap();
        market.refresh_affordability(&[(buyer, 970)]);
        let current = market.current_period();
        assert_eq!((current.start_ms, current.end_ms), (0, UPDATE_TIME_MS));
        assert_eq!(
            current.goods[Good::BreadRoll as usize],
            GoodActivity {
                local_quoted_value: 2.0 * Good::BreadRoll.core_price(),
                traded_units: 2,
                listed_units: 8,
                affordable_demand_units: 5,
                traded_coins: 9,
                local_traded_units: 2,
                local_traded_coins: 9,
                ..GoodActivity::default()
            }
        );
        assert!(market.previous_period().is_none());
        market.update(UPDATE_TIME_MS - 1).unwrap();
        assert!(market.previous_period().is_none());
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.previous_period().unwrap(), current);
        assert_eq!(
            market.current_period().goods[Good::BreadRoll as usize].traded_units,
            0
        );
        assert_eq!(market.current_period().start_ms, UPDATE_TIME_MS);
        assert_eq!(market.current_period().end_ms, UPDATE_TIME_MS + DAY_MS);
        assert_eq!(
            market.previous_period().unwrap().goods[Good::Water as usize],
            GoodActivity::default()
        );
    }

    #[test]
    fn inactive_latest_period_does_not_reuse_older_history() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_reference(seller, PLACE, Good::Water, 50)
            .unwrap();
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(
            market.previous_period().unwrap().goods[Good::Water as usize].listed_units,
            50
        );
        market.withdraw(seller, PLACE, Good::Water, 50).unwrap();
        market.update(UPDATE_TIME_MS + DAY_MS).unwrap();
        let previous = market.previous_period().unwrap();
        assert_eq!(
            (previous.start_ms, previous.end_ms),
            (UPDATE_TIME_MS, UPDATE_TIME_MS + DAY_MS)
        );
        assert_eq!(previous.goods, [GoodActivity::default(); Good::COUNT]);
        assert_eq!(market.history().len(), 1);
    }

    #[test]
    fn zero_purchases_preserve_stock_funds_requests_and_history() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_reference(seller, PLACE, Good::Berries, 35)
            .unwrap();
        let units = 0;
        market
            .set_request(buyer, ShoppingList::single(Good::Berries, units))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2)]);
        let before = market.clone();
        let purchase = market
            .purchase(buyer, PLACE, (Good::Berries, units), 200, 0, true)
            .unwrap();
        assert_eq!(purchase.units, 0);
        assert_eq!(purchase.coins, 0);
        assert!(purchase.payments.is_empty());
        assert_eq!(market, before);
    }
    #[test]
    fn seller_rounding_aggregates_orders_and_affordability_matches_settlement() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let other = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Berries, 5.0).unwrap();
        market
            .list_reference(seller, PLACE, Good::Berries, 100)
            .unwrap();
        market
            .list_reference(other, PLACE, Good::Berries, 100)
            .unwrap();
        market
            .list_reference(seller, PLACE, Good::Berries, 100)
            .unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::Berries, 300))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(market.affordable_request(buyer).units(Good::Berries), 300);
        assert_eq!(market.purchase_cost(buyer, Good::Berries, 300), Some(2));
        let purchase = market
            .purchase(buyer, PLACE, (Good::Berries, 300), 2, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (300, 2));
        assert_eq!(
            purchase
                .payments
                .iter()
                .map(|(_, coins)| *coins)
                .sum::<Coins>(),
            2
        );
        assert_eq!(
            market
                .trades()
                .iter()
                .map(|trade| trade.coins)
                .sum::<Coins>(),
            2
        );
        assert_eq!(market.available_units(buyer, Good::Berries), 0);
    }

    #[test]
    fn equal_price_listings_fill_one_seller_before_another() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let other = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Wheat, 12.871549).unwrap();
        market
            .list_reference(seller, PLACE, Good::Wheat, 11)
            .unwrap();
        market
            .list_reference(other, PLACE, Good::Wheat, 250)
            .unwrap();
        market
            .list_reference(seller, PLACE, Good::Wheat, 289)
            .unwrap();
        assert_eq!(
            market.purchase_cost_at(buyer, PLACE, Good::Wheat, 300),
            Some(4)
        );
        let purchase = market
            .purchase(buyer, PLACE, (Good::Wheat, 300), 4, 0, false)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (300, 4));
        assert_eq!(purchase.payments, vec![(seller, 4)]);
        assert_eq!(market.available_units(buyer, Good::Wheat), 250);
        let purchase = market
            .purchase(buyer, PLACE, (Good::Wheat, 300), 1, 0, false)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (77, 1));
    }

    #[test]
    fn explicit_purchase_bounds_survive_planning_and_limit_partial_stops() {
        use crate::locations::{Location, Map, Position};
        let map = Map::default();
        let place = map.public_place(Location::Market);
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_at_quote(buyer, place, Good::Wheat, 1000, 1.0)
            .unwrap();
        market
            .list_at_quote(seller, place, Good::Wheat, 50, 10.0)
            .unwrap();
        market
            .list_at_quote(seller, place, Good::Wheat, 1000, 12.0)
            .unwrap();
        assert_eq!(
            market.lowest_purchase_quote(buyer.into(), Good::Wheat, &map),
            Some(10.0)
        );
        assert_eq!(market.lowest_listed_quote(Good::Wheat, &map), Some(1.0));
        let mut ceilings = [0.0; Good::COUNT];
        ceilings[Good::Wheat as usize] = 11.0;
        let list = ShoppingList::single(Good::Wheat, 1000).with_quote_ceilings(ceilings, 100);
        let bounded = list
            .with_purchase_bounds(&market, buyer.into(), place, 100)
            .unwrap();
        assert_eq!(bounded, list);
        let (_, basket) = market
            .next_purchase_stop(buyer.into(), &map, Position::default(), list, Some(100))
            .unwrap();
        assert_eq!(basket.units(Good::Wheat), 50);
        assert_eq!(basket.quote_ceiling(Good::Wheat), 11.0);
        assert_eq!(basket.quote_ceiling(Good::Flour), 0.0);
        let saved = ron::to_string(&basket).unwrap();
        assert_eq!(ron::from_str::<ShoppingList>(&saved).unwrap(), basket);
        market
            .list_at_quote(seller, place, Good::Flour, 100, 1.0)
            .unwrap();
        assert!(
            market
                .next_purchase_stop(
                    buyer.into(),
                    &map,
                    Position::default(),
                    ShoppingList::single(Good::Flour, 100).with_quote_ceilings(ceilings, 100),
                    Some(100)
                )
                .is_none()
        );
        market
            .list_at_quote(seller, place, Good::Wheat, 100, 10.5)
            .unwrap();
        let (_, next) = market
            .next_purchase_stop(buyer.into(), &map, Position::default(), list, Some(100))
            .unwrap();
        assert_eq!(next.units(Good::Wheat), 150);
        assert_eq!(next.quote_ceiling(Good::Wheat), 11.0);
    }

    #[test]
    fn cheapest_stops_split_local_quotes_and_keep_other_goods_at_their_own_cheapest_site() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 1.0, y: 0.0 },
            Position { x: 20.0, y: 0.0 },
            Position { x: 40.0, y: 0.0 },
        )
        .unwrap();
        let near = map.public_place(Location::Forest);
        let far = map.public_place(Location::River);
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        for (place, good, units, quote) in [
            (near, Good::Wheat, 1000, 10.0),
            (near, Good::Wheat, 1000, 30.0),
            (far, Good::Wheat, 1000, 20.0),
            (near, Good::Water, 1000, 50.0),
            (far, Good::Water, 1000, 10.0),
        ] {
            market
                .list_at_quote(seller, place, good, units, quote)
                .unwrap();
        }
        let mut remaining = ShoppingList::new([(Good::Wheat, 2500), (Good::Water, 1000)]).unwrap();
        let estimated = market
            .estimated_purchase_cost_from(buyer, &map, Position::default(), remaining)
            .unwrap();
        let mut position = Position::default();
        let mut spent = 0;
        let mut stops = Vec::new();
        while let Some((place, basket)) =
            market.next_purchase_stop(buyer.into(), &map, position, remaining, None)
        {
            stops.push((place, basket));
            for (good, units) in basket.items() {
                spent += market
                    .purchase(buyer, place, (good, units), Coins::MAX, 0, false)
                    .unwrap()
                    .coins;
            }
            remaining = ShoppingList::new(
                remaining
                    .items()
                    .map(|(good, units)| (good, units - basket.units(good))),
            )
            .unwrap();
            position = map.position(place);
        }
        assert_eq!(
            stops,
            vec![
                (near, ShoppingList::single(Good::Wheat, 1000)),
                (
                    far,
                    ShoppingList::new([(Good::Wheat, 1000), (Good::Water, 1000)]).unwrap()
                ),
                (near, ShoppingList::single(Good::Wheat, 500)),
            ]
        );
        assert_eq!(spent, 55);
        assert_eq!(estimated, spent);
        assert_eq!(remaining, ShoppingList::default());
    }

    #[test]
    fn cheapest_stop_prefers_price_then_distance_and_ignores_ineligible_orders() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 1.0, y: 0.0 },
            Position { x: 20.0, y: 0.0 },
            Position::default(),
        )
        .unwrap();
        let near = map.public_place(Location::Forest);
        let far = map.public_place(Location::River);
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_at_quote(seller, near, Good::Wheat, 3600, 97.5)
            .unwrap();
        market
            .list_at_quote(seller, far, Good::Wheat, 12000, 45.2)
            .unwrap();
        market
            .list_at_quote(buyer, near, Good::Wheat, 1000, 1.0)
            .unwrap();
        let list = ShoppingList::single(Good::Wheat, 3600);
        assert_eq!(
            market.next_purchase_stop(buyer.into(), &map, Position::default(), list, None),
            Some((far, list))
        );
        market
            .list_at_quote(seller, near, Good::Wheat, 3600, 45.2)
            .unwrap();
        assert_eq!(
            market.next_purchase_stop(buyer.into(), &map, Position::default(), list, None),
            Some((near, list))
        );
        assert_eq!(
            market.next_purchase_stop(buyer.into(), &map, map.position(far), list, None),
            Some((far, list))
        );
    }

    #[test]
    fn cheaper_quotes_remain_ahead_of_grouped_seller_listings() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Wheat, 20.0).unwrap();
        market
            .list_reference(seller, PLACE, Good::Wheat, 11)
            .unwrap();
        let id = OrderId(market.next_order_id);
        market.next_order_id += 1;
        market.orders.insert(
            id,
            SellOrder {
                id,
                seller: MarketParty::Caravan,
                place: PLACE,
                good: Good::Wheat,
                units: 300,
                quoted_price: 10.0,
            },
        );
        market
            .list_reference(seller, PLACE, Good::Wheat, 289)
            .unwrap();
        let purchase = market
            .purchase(buyer, PLACE, (Good::Wheat, 300), 3, 0, false)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (300, 3));
        assert!(purchase.payments.is_empty());
        assert_eq!(market.available_units(buyer, Good::Wheat), 300);
    }

    #[test]
    fn integer_boundary_quotes_use_direct_ceil_and_never_fractional_goods() {
        for (price, expected) in [(10.0, 1), (f64::from_bits(10.0_f64.to_bits() + 1), 2)] {
            let seller = AgentId(uuid::Uuid::new_v4());
            let buyer = AgentId(uuid::Uuid::new_v4());
            let mut market = Market::default();
            market.prices = market.prices.with_price(Good::Berries, price).unwrap();
            market
                .list_reference(seller, PLACE, Good::Berries, 100)
                .unwrap();
            assert_eq!(
                market.purchase_cost(buyer, Good::Berries, 100),
                Some(expected)
            );
            let purchase = market
                .purchase(buyer, PLACE, (Good::Berries, 100), expected, 0, false)
                .unwrap();
            assert_eq!((purchase.units, purchase.coins), (100, expected));
        }
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_reference(seller, PLACE, Good::BreadRoll, 2)
            .unwrap();
        let before = market.clone();
        let purchase = market
            .purchase(buyer, PLACE, (Good::BreadRoll, 2), 2, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (0, 0));
        assert_eq!(market, before);
    }

    #[test]
    fn integer_stock_transfer_handles_large_values_and_retains_real_overflow_guard() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list_reference(seller, PLACE, Good::Berries, Quantity::MAX)
            .unwrap();
        let purchase = market
            .purchase(buyer, PLACE, (Good::Berries, 1), 1, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (1, 1));
        assert_eq!(
            market.listed_units(seller, Good::Berries),
            Quantity::MAX - 1
        );
        assert_eq!(market.withdraw(seller, PLACE, Good::Berries, 1).unwrap(), 1);
        assert_eq!(
            market.listed_units(seller, Good::Berries),
            Quantity::MAX - 2
        );
        assert_eq!(
            market.list_reference(seller, PLACE, Good::Berries, 3),
            Err(SimulationError::WealthOverflow)
        );
        market.prices = market.prices.with_price(Good::BreadRoll, f64::MAX).unwrap();
        market
            .list_reference(seller, PLACE, Good::BreadRoll, 1)
            .unwrap();
        assert_eq!(market.purchase_cost(buyer, Good::BreadRoll, 1), None);
    }

    #[test]
    fn empty_market_keeps_cash_affordable_unmet_demand() {
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Berries, 5.0).unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::Berries, 300))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1)]);
        assert_eq!(market.unmet_units(Good::Berries), 200);
        let purchase = market
            .purchase(buyer, PLACE, (Good::Berries, 300), 1, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (0, 0));
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.prices.price(Good::Berries), Some(5.0));
    }
}
