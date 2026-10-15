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
}

impl Default for ExportPolicy {
    fn default() -> Self {
        Self {
            allowed: true,
            minimum_reserve: 0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaravanPolicy {
    pub exports: [ExportPolicy; Good::COUNT],
    pub import_tariff_basis_points: u16,
    pub export_tariff_basis_points: u16,
}

impl Default for CaravanPolicy {
    fn default() -> Self {
        Self {
            exports: Good::ALL.map(|good| ExportPolicy {
                allowed: good.caravan_eligible(),
                ..ExportPolicy::default()
            }),
            import_tariff_basis_points: 0,
            export_tariff_basis_points: 0,
        }
    }
}

impl CaravanPolicy {
    pub fn validate(self) -> Result<(), SimulationError> {
        if self.import_tariff_basis_points > 10_000 || self.export_tariff_basis_points > 10_000 {
            return Err(SimulationError::InvalidTaxRate);
        }
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
        let mut cost: Coins = 0;
        let mut quote_ceilings = [0.0; Good::COUNT];
        for (good, units) in self.items() {
            let fill = market.fill_plan(buyer, Some(place), good, units, None)?;
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Market {
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
    import_tariff_carry: [u16; Good::COUNT],
    export_tariff_carry: [u16; Good::COUNT],
    orders: HashMap<OrderId, SellOrder>,
    trades: Vector<Trade>,
    next_order_id: u64,
    requests: HashMap<AgentId, ShoppingList>,
    affordable_requests: HashMap<AgentId, ShoppingList>,
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
        let rate = account
            .0
            .as_bytes()
            .iter()
            .copied()
            .find(|byte| *byte < 250)
            .unwrap_or(0)
            % 10
            + 1;
        *self.seller_rates.entry(account).or_insert(rate)
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
                    let Ok(fill) =
                        self.fill_plan_at_quote(party, None, good, requested, Some(funds), ceiling)
                    else {
                        continue;
                    };
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
                            .is_ok_and(|cost| cost <= funds)
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
        (good.caravan_eligible() && self.policy.export_tariff_basis_points < 10_000).then(|| {
            good.export_threshold()
                * (1.0 - f64::from(self.policy.export_tariff_basis_points) / 10_000.0)
        })
    }

    pub fn effective_import_threshold(&self, good: Good) -> Option<f64> {
        (good.caravan_eligible() && self.policy.import_tariff_basis_points < 10_000).then(|| {
            good.import_threshold()
                / (1.0 - f64::from(self.policy.import_tariff_basis_points) / 10_000.0)
        })
    }

    pub(crate) fn set_caravan_policy(
        &mut self,
        mut policy: CaravanPolicy,
    ) -> Result<(), SimulationError> {
        policy.validate()?;
        for good in Good::ALL {
            policy.exports[good as usize].allowed &= good.caravan_eligible();
        }
        self.policy = policy;
        self.remove_ineligible_caravan_orders();
        for id in self.orders.keys().copied().collect::<Vec<_>>() {
            let good = self.orders[&id].good;
            if self.orders[&id].seller == MarketParty::Caravan {
                self.orders.get_mut(&id).unwrap().quoted_price =
                    if policy.import_tariff_basis_points == 10_000 {
                        good.import_threshold()
                    } else {
                        self.effective_import_threshold(good).unwrap()
                    };
            }
        }
        Ok(())
    }

    fn remove_ineligible_caravan_orders(&mut self) {
        self.orders.retain(|_, order| {
            order.seller != MarketParty::Caravan || order.good.caravan_eligible()
        });
    }

    fn purchasable(&self, order: &SellOrder) -> bool {
        order.seller != MarketParty::Caravan
            || (order.good.caravan_eligible() && self.policy.import_tariff_basis_points < 10_000)
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
        for &(buyer, budget) in budgets {
            let buyer = buyer.into();
            let Some(account) = buyer.account() else {
                continue;
            };
            let mut funds = budget.max(0);
            let mut items = Vec::new();
            for (good, requested) in self.requested(account).items() {
                if let Ok(fill) = self.fill_plan(buyer, None, good, requested, Some(funds)) {
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
            if let Ok(list) = ShoppingList::new(items)
                && list.items().next().is_some()
            {
                affordable.insert(account, list);
            }
        }
        self.affordable_requests = affordable;
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
        let mut remaining = budget.max(0);
        let mut fills = Vec::new();
        for (good, units) in list.items() {
            let fill = self.fill_plan(buyer, place, good, units, Some(remaining))?;
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

    pub(crate) fn nearest_purchase_place(
        &self,
        buyer: impl Into<MarketParty>,
        map: &crate::locations::Map,
        position: crate::locations::Position,
        list: ShoppingList,
        visited: &[PlaceId],
    ) -> Option<PlaceId> {
        let buyer = buyer.into();
        self.orders()
            .filter(|order| {
                order.units > 0
                    && list.units(order.good) > 0
                    && order.seller != buyer
                    && self.purchasable(order)
            })
            .filter_map(|order| map.places().get(&order.place))
            .filter(|place| place.kind != crate::locations::Location::Home)
            .filter(|place| !visited.contains(&place.id))
            .min_by(|a, b| {
                position
                    .distance(a.position)
                    .total_cmp(&position.distance(b.position))
                    .then_with(|| a.id.0.cmp(&b.id.0))
            })
            .map(|place| place.id)
    }

    pub(crate) fn estimated_purchase_cost_from(
        &self,
        buyer: impl Into<MarketParty>,
        map: &crate::locations::Map,
        mut position: crate::locations::Position,
        mut remaining: ShoppingList,
    ) -> Option<Coins> {
        let buyer = buyer.into();
        let mut visited = Vec::new();
        let mut coins: Coins = 0;
        while remaining.items().next().is_some() {
            let Some(place) =
                self.nearest_purchase_place(buyer, map, position, remaining, &visited)
            else {
                break;
            };
            let mut shortages = Vec::new();
            for (good, units) in remaining.items() {
                let fill = self.fill_plan(buyer, Some(place), good, units, None).ok()?;
                coins = coins.checked_add(fill.coins)?;
                shortages.push((good, units - fill.units));
            }
            remaining = ShoppingList::new(shortages).ok()?;
            position = map.position(place);
            visited.push(place);
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
        let available = self.available_units(buyer, good).min(units);
        let actual = self.purchase_cost(buyer, good, available)?;
        let unavailable = units - available;
        actual.checked_add(charge(self.prices.value(good, unavailable)?).ok()?)
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
        let mut plan = FillPlan::default();
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
            if budget.is_none() {
                affordable(high)
                    .and_then(|cost| plan.coins.checked_add(cost))
                    .ok_or(SimulationError::WealthOverflow)?;
                low = high;
            }
            while low < high {
                let middle = low + (high - low).div_ceil(2);
                if affordable(middle).is_some_and(|cost| cost <= available_coins) {
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
        if units == 0 || !price.is_finite() || price < MIN_QUOTED_PRICE {
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
            order.units -= units;
            if order.units == 0 {
                self.orders.remove(&order.id);
                self.batches.remove(&order.id);
            } else {
                self.orders.insert(order.id, order);
            }
        }
        Ok(withdrawn)
    }
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
                    self.policy.import_tariff_basis_points,
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
                    recipient_coins: coins - tariff_coins,
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
                    coins - tariff_coins,
                    buyer != MarketParty::Caravan && order.seller != MarketParty::Caravan,
                    time_ms,
                );
            }
            if order.units == 0 {
                self.orders.remove(&id);
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
                        let units = remaining.min(order.units);
                        if units == 0 {
                            break;
                        }
                        value +=
                            units as f64 * order.quoted_price / good.units_per_price_unit() as f64;
                        let cumulative =
                            gross_up(charge(value)?, self.policy.export_tariff_basis_points)?;
                        let coins = cumulative - charged;
                        charged = cumulative;
                        let tariff_coins = tariff_share(
                            coins,
                            self.policy.export_tariff_basis_points,
                            &mut self.export_tariff_carry[good as usize],
                        )?;
                        let recipient_coins = coins - tariff_coins;
                        received = received
                            .checked_add(recipient_coins)
                            .ok_or(SimulationError::WealthOverflow)?;
                        remaining -= units;
                        order.units -= units;
                        if order.units == 0 {
                            self.orders.remove(&order.id);
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
            let after = accepted_local_price.map_or(before, |accepted| {
                (before * 0.75 + accepted * 0.25).max(MIN_QUOTED_PRICE)
            });
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

fn gross_up(net: Coins, rate: u16) -> Result<Coins, SimulationError> {
    let denominator = 10_000_u128 - u128::from(rate);
    if denominator == 0 {
        return Err(SimulationError::InvalidTaxRate);
    }
    let gross = (u128::try_from(net).map_err(|_| SimulationError::InvalidCoins)? * 10_000)
        .div_ceil(denominator);
    Coins::try_from(gross).map_err(|_| SimulationError::WealthOverflow)
}

// Carry follows the good and direction, so splitting orders cannot discard fractional tax.
fn tariff_share(gross: Coins, rate: u16, carry: &mut u16) -> Result<Coins, SimulationError> {
    let numerator = u128::try_from(gross).map_err(|_| SimulationError::InvalidCoins)?
        * u128::from(rate)
        + u128::from(*carry);
    let tariff =
        Coins::try_from(numerator / 10_000).map_err(|_| SimulationError::WealthOverflow)?;
    *carry = (numerator % 10_000) as u16;
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
        if self
            .import_tariff_carry
            .into_iter()
            .chain(self.export_tariff_carry)
            .any(|carry| carry >= 10_000)
            || self.orders.iter().any(|(id, order)| {
                *id != order.id
                    || id.0 >= self.next_order_id
                    || !order.quoted_price.is_finite()
                    || order.quoted_price < MIN_QUOTED_PRICE
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
                .citizen_history(buyer)
                .unwrap()
                .completed
                .back()
                .unwrap()
                .bought[Good::BreadRoll as usize],
            2
        );
        assert_eq!(
            world
                .citizen_history(seller)
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
        assert_eq!(market.prices.price(Good::Thread), Some(10.0049));
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
    fn caravan_quotes_and_unmet_demand_do_not_move_the_reference() {
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
            assert_eq!(market.prices.price(Good::BreadRoll), Some(10.0));
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
    fn geographic_estimate_follows_the_next_nearest_stop_for_a_basket() {
        use crate::locations::{Location, Map, Position};
        let origin = Position::default();
        let map = Map::new(
            Position { x: 10.0, y: 0.0 },
            Position { x: -11.0, y: 0.0 },
            Position { x: 20.0, y: 0.0 },
        )
        .unwrap();
        let forest = map
            .places()
            .values()
            .find(|p| p.kind == Location::Forest)
            .unwrap()
            .id;
        let river = map
            .places()
            .values()
            .find(|p| p.kind == Location::River)
            .unwrap()
            .id;
        let town = map
            .places()
            .values()
            .find(|p| p.kind == Location::Market)
            .unwrap()
            .id;
        let buyer = AgentId(uuid::Uuid::new_v4());
        let seller = AgentId(uuid::Uuid::new_v4());
        let other = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Wheat, 12.871549).unwrap();
        market
            .list_reference(other, river, Good::Wheat, 10)
            .unwrap();
        market
            .list_reference(seller, forest, Good::Water, 1)
            .unwrap();
        market
            .list_reference(seller, town, Good::Wheat, 11)
            .unwrap();
        market
            .list_reference(other, town, Good::Wheat, 250)
            .unwrap();
        market
            .list_reference(seller, town, Good::Wheat, 289)
            .unwrap();
        let list = ShoppingList::new([(Good::Water, 1), (Good::Wheat, 300)]).unwrap();
        assert_eq!(
            market.nearest_purchase_place(buyer, &map, origin, list, &[]),
            Some(forest)
        );
        assert_eq!(
            market.nearest_purchase_place(
                buyer,
                &map,
                map.position(forest),
                ShoppingList::single(Good::Wheat, 300),
                &[forest]
            ),
            Some(town)
        );
        let water_cost = market
            .purchase_cost_at(buyer, forest, Good::Water, 1)
            .unwrap();
        assert_eq!(
            market.estimated_purchase_cost_from(buyer, &map, origin, list),
            Some(water_cost + 4)
        );
        assert_eq!(market.available_units_at(buyer, town, Good::Wheat), 550);
        let water = market
            .purchase(buyer, forest, (Good::Water, 1), 100, 0, false)
            .unwrap();
        let wheat = market
            .purchase(buyer, town, (Good::Wheat, 300), 100 - water.coins, 0, false)
            .unwrap();
        assert_eq!(water.coins + wheat.coins, water_cost + 4);
        let remaining = ShoppingList::single(Good::Wheat, 400);
        assert_eq!(
            market.estimated_purchase_cost_from(buyer, &map, origin, remaining),
            Some(1 + 4 + 2)
        );
    }

    #[test]
    fn nearest_purchase_place_filters_orders_and_excluded_places() {
        use crate::locations::{Location, Map, Position};
        let buyer = AgentId(uuid::Uuid::from_u128(1));
        let seller = MarketParty::Citizen(AgentId(uuid::Uuid::from_u128(2)));
        let map = Map::new(
            Position { x: 1.0, y: 0.0 },
            Position { x: 2.0, y: 0.0 },
            Position { x: 3.0, y: 0.0 },
        )
        .unwrap();
        let forest = map.public_place(Location::Forest);
        let river = map.public_place(Location::River);
        let (map, home) = map
            .with_place(Location::Home, Position::default(), Some(buyer), "home")
            .unwrap();
        let orphan = PlaceId(uuid::Uuid::from_u128(99));
        let list = ShoppingList::single(Good::Wheat, 1);
        for (place, owner, good, units, tariff) in [
            (forest, MarketParty::Citizen(buyer), Good::Wheat, 1, 0),
            (forest, seller, Good::Wheat, 0, 0),
            (forest, seller, Good::Wood, 1, 0),
            (forest, MarketParty::Caravan, Good::Wheat, 1, 10_000),
            (home, seller, Good::Wheat, 1, 0),
            (orphan, seller, Good::Wheat, 1, 0),
        ] {
            let mut market = Market::default();
            market.policy.import_tariff_basis_points = tariff;
            market
                .list_reference(seller, river, Good::Wheat, 1)
                .unwrap();
            let id = OrderId(market.next_order_id);
            market.orders.insert(
                id,
                SellOrder {
                    id,
                    seller: owner,
                    place,
                    good,
                    units,
                    quoted_price: 1.0,
                },
            );
            assert_eq!(
                market.nearest_purchase_place(buyer, &map, Position::default(), list, &[]),
                Some(river)
            );
        }
        let mut market = Market::default();
        market
            .list_reference(seller, forest, Good::Wheat, 1)
            .unwrap();
        market
            .list_reference(seller, river, Good::Wheat, 1)
            .unwrap();
        assert_eq!(
            market.nearest_purchase_place(buyer, &map, Position::default(), list, &[forest]),
            Some(river)
        );
        assert_eq!(
            market.nearest_purchase_place(buyer, &map, Position::default(), list, &[forest, river]),
            None
        );
        assert_eq!(
            market.nearest_purchase_place(
                buyer,
                &map,
                Position::default(),
                ShoppingList::default(),
                &[]
            ),
            None
        );
    }

    #[test]
    fn nearest_purchase_place_breaks_distance_ties_by_place_id() {
        use crate::locations::{Map, Position};
        let map = Map::default();
        let mut places: Vec<_> = map.places().keys().copied().collect();
        places.sort_by_key(|place| place.0);
        let buyer = AgentId(uuid::Uuid::from_u128(1));
        let seller = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        for place in places.iter().rev() {
            market
                .list_reference(seller, *place, Good::Wheat, 1)
                .unwrap();
            market
                .list_reference(seller, *place, Good::Wood, 1)
                .unwrap();
        }
        assert_eq!(
            market.nearest_purchase_place(
                buyer,
                &map,
                Position::default(),
                ShoppingList::new([(Good::Wheat, 1), (Good::Wood, 1)]).unwrap(),
                &[],
            ),
            Some(places[0])
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
