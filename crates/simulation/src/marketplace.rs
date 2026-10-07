use crate::{AgentId, Coins, PlaceId, Quantity, SimulationError};
use imbl::{HashMap, Vector};

pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub const UPDATE_TIME_MS: u64 = 4 * 60 * 60 * 1000;
pub const MAX_DAILY_PRICE_CHANGE: f64 = 0.10;
pub const MIN_QUOTED_PRICE: f64 = 0.01;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Good {
    Berries,
    Wheat,
    Flour,
    Wood,
    Water,
    Bread,
    BerryPie,
    Flax,
    Thread,
    Cloth,
    FlaxBlock,
    FlaxGarment,
}

impl Good {
    pub const COUNT: usize = 12;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Berries,
        Self::Wheat,
        Self::Flour,
        Self::Wood,
        Self::Water,
        Self::Bread,
        Self::BerryPie,
        Self::Flax,
        Self::Thread,
        Self::Cloth,
        Self::FlaxBlock,
        Self::FlaxGarment,
    ];
    pub const FOOD: [Self; 3] = [Self::Berries, Self::Bread, Self::BerryPie];
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
            Self::Bread => Some(50.0),
            Self::BerryPie => Some(60.0),
            _ => None,
        }
    }
    pub fn eating_ms_per_unit(self) -> Option<u64> {
        self.nutrition_per_unit()
            .map(|_| self.weight_grams() * 1000)
    }
    pub fn weight_grams(self) -> u64 {
        match self {
            Self::Bread => 100,
            Self::BerryPie => 125,
            Self::FlaxBlock => 25,
            Self::FlaxGarment => 200,
            _ => 1,
        }
    }
    pub fn unit_name(self) -> &'static str {
        match self {
            Self::Bread => "loaves",
            Self::BerryPie => "pies",
            Self::FlaxBlock => "blocks",
            Self::FlaxGarment => "garments",
            _ => "g",
        }
    }
    pub fn price_unit_name(self) -> &'static str {
        match self {
            Self::Bread => "loaf",
            Self::BerryPie => "pie",
            Self::FlaxBlock | Self::FlaxGarment => "each",
            _ => "kg",
        }
    }
    pub fn units_per_price_unit(self) -> u64 {
        match self {
            Self::Bread | Self::BerryPie | Self::FlaxBlock | Self::FlaxGarment => 1,
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
            Self::Bread => "Bread",
            Self::BerryPie => "Berry pie",
            Self::Flax => "Flax",
            Self::Thread => "Thread",
            Self::Cloth => "Cloth",
            Self::FlaxBlock => "Flax block",
            Self::FlaxGarment => "Flax garment",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaravanPolicy {
    pub exports: [ExportPolicy; Good::COUNT],
    pub import_tariff_basis_points: u16,
    pub export_tariff_basis_points: u16,
}

impl Default for CaravanPolicy {
    fn default() -> Self {
        Self {
            exports: [ExportPolicy::default(); Good::COUNT],
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShoppingList {
    units: [Quantity; Good::COUNT],
}

impl Default for ShoppingList {
    fn default() -> Self {
        Self {
            units: [0; Good::COUNT],
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

#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OrderId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MarketParty {
    Citizen(AgentId),
    Caravan,
}

impl MarketParty {
    pub fn citizen(self) -> Option<AgentId> {
        match self {
            Self::Citizen(id) => Some(id),
            Self::Caravan => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SellOrder {
    pub id: OrderId,
    pub seller: MarketParty,
    pub place: PlaceId,
    pub good: Good,
    pub units: Quantity,
    pub quoted_price: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
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

#[derive(Clone, Debug, PartialEq)]
pub struct DailyMarketActivity {
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GoodActivity {
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

#[derive(Clone, Debug, PartialEq)]
pub struct MarketPeriod {
    pub start_ms: u64,
    pub end_ms: u64,
    pub goods: [GoodActivity; Good::COUNT],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Market {
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
    pub fn caravan_policy(&self) -> CaravanPolicy {
        self.policy
    }

    pub fn effective_export_threshold(&self, good: Good) -> Option<f64> {
        (self.policy.export_tariff_basis_points < 10_000).then(|| {
            good.export_threshold()
                * (1.0 - f64::from(self.policy.export_tariff_basis_points) / 10_000.0)
        })
    }

    pub fn effective_import_threshold(&self, good: Good) -> Option<f64> {
        (self.policy.import_tariff_basis_points < 10_000).then(|| {
            good.import_threshold()
                / (1.0 - f64::from(self.policy.import_tariff_basis_points) / 10_000.0)
        })
    }

    pub(crate) fn set_caravan_policy(
        &mut self,
        policy: CaravanPolicy,
    ) -> Result<(), SimulationError> {
        policy.validate()?;
        self.policy = policy;
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

    fn purchasable(&self, order: &SellOrder) -> bool {
        order.seller != MarketParty::Caravan || self.policy.import_tariff_basis_points < 10_000
    }

    pub(crate) fn starting_at(time_ms: u64) -> Self {
        let period_start_ms =
            crate::production::period_start(crate::production::work_period(time_ms));
        Self {
            period_start_ms,
            first_period_start_ms: period_start_ms,
            started_at_ms: time_ms,
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

    pub(crate) fn refresh_affordability(&mut self, budgets: &[(AgentId, Coins)]) {
        let mut affordable = HashMap::new();
        for &(buyer, budget) in budgets {
            let mut funds = budget.max(0);
            let mut items = Vec::new();
            for (good, requested) in self.requested(buyer).items() {
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
                affordable.insert(buyer, list);
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
    pub fn listed_units(&self, owner: AgentId, good: Good) -> Quantity {
        self.orders()
            .filter(|o| o.seller == MarketParty::Citizen(owner) && o.good == good)
            .map(|o| o.units)
            .sum()
    }
    pub fn available_units(&self, buyer: AgentId, good: Good) -> Quantity {
        self.orders()
            .filter(|o| {
                self.purchasable(o) && o.seller != MarketParty::Citizen(buyer) && o.good == good
            })
            .map(|o| o.units)
            .sum()
    }
    pub fn available_units_at(&self, buyer: AgentId, place: PlaceId, good: Good) -> Quantity {
        self.orders()
            .filter(|order| {
                self.purchasable(order)
                    && order.seller != MarketParty::Citizen(buyer)
                    && order.place == place
                    && order.good == good
            })
            .map(|order| order.units)
            .sum()
    }

    pub fn purchase_cost_at(
        &self,
        buyer: AgentId,
        place: PlaceId,
        good: Good,
        requested: Quantity,
    ) -> Option<Coins> {
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

    pub fn estimated_purchase_cost(
        &self,
        buyer: AgentId,
        good: Good,
        units: Quantity,
    ) -> Option<Coins> {
        let available = self.available_units(buyer, good).min(units);
        let actual = self.purchase_cost(buyer, good, available)?;
        let unavailable = units - available;
        actual.checked_add(charge(self.prices.value(good, unavailable)?).ok()?)
    }

    pub fn purchase_cost(&self, buyer: AgentId, good: Good, requested: Quantity) -> Option<Coins> {
        Some(
            self.fill_plan(buyer, None, good, requested, None)
                .ok()?
                .coins,
        )
    }

    fn fill_plan(
        &self,
        buyer: AgentId,
        place: Option<PlaceId>,
        good: Good,
        requested: Quantity,
        budget: Option<Coins>,
    ) -> Result<FillPlan, SimulationError> {
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| {
                self.purchasable(o)
                    && o.seller != MarketParty::Citizen(buyer)
                    && o.good == good
                    && place.is_none_or(|id| o.place == id)
            })
            .collect();
        orders.sort_by(|a, b| {
            a.quoted_price
                .total_cmp(&b.quoted_price)
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
        seller: AgentId,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        if units == 0 {
            return Ok(());
        }
        self.orders()
            .filter(|order| order.good == good)
            .map(|order| order.units)
            .try_fold(units, Quantity::checked_add)
            .ok_or(SimulationError::WealthOverflow)?;
        let price = self
            .prices
            .price(good)
            .ok_or(SimulationError::InvalidPrices)?;
        if !self.prices.value(good, units).is_some_and(f64::is_finite) {
            return Err(SimulationError::WealthOverflow);
        }
        let id = OrderId(self.next_order_id);
        self.next_order_id = self
            .next_order_id
            .checked_add(1)
            .ok_or(SimulationError::TimeOverflow)?;
        self.orders.insert(
            id,
            SellOrder {
                id,
                seller: MarketParty::Citizen(seller),
                place,
                good,
                units,
                quoted_price: price,
            },
        );
        Ok(())
    }
    pub(crate) fn withdraw(
        &mut self,
        owner: AgentId,
        place: PlaceId,
        good: Good,
        requested: Quantity,
    ) -> Result<Quantity, SimulationError> {
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| {
                o.seller == MarketParty::Citizen(owner) && o.good == good && o.place == place
            })
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
            } else {
                self.orders.insert(order.id, order);
            }
        }
        Ok(withdrawn)
    }
    pub(crate) fn purchase(
        &mut self,
        buyer: AgentId,
        place: PlaceId,
        item: (Good, Quantity),
        budget: Coins,
        time_ms: u64,
        record: bool,
    ) -> Result<Purchase, SimulationError> {
        let (good, requested) = item;
        let plan = self.fill_plan(buyer, Some(place), good, requested, Some(budget))?;
        let mut payments = std::collections::BTreeMap::new();
        for (id, units, coins) in plan.fills {
            let mut order = self.orders.get(&id).unwrap().clone();
            if let Some(seller) = order.seller.citizen() {
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
                if let Some(request) = self.requests.get_mut(&buyer) {
                    request.units[good as usize] =
                        request.units[good as usize].saturating_sub(units);
                }
                self.trades.push_back(Trade {
                    place: order.place,
                    time_ms,
                    buyer: MarketParty::Citizen(buyer),
                    seller: order.seller,
                    good,
                    units,
                    coins,
                    quoted_price: order.quoted_price,
                    tariff_coins,
                    recipient_coins: coins - tariff_coins,
                });
            }
            if order.units == 0 {
                self.orders.remove(&id);
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
        let closed = self.previous_period();
        let mut payments = std::collections::BTreeMap::<uuid::Uuid, Coins>::new();
        for good in Good::ALL {
            let price = self.prices.price(good).unwrap();
            if self.policy.exports[good as usize].allowed
                && self
                    .effective_export_threshold(good)
                    .is_some_and(|threshold| price < threshold)
            {
                let mut sellers = std::collections::BTreeMap::<uuid::Uuid, Quantity>::new();
                for order in self.orders().filter(|order| order.good == good) {
                    if let Some(seller) = order.seller.citizen() {
                        *sellers.entry(seller.0).or_default() += order.units;
                    }
                }
                let total: Quantity = sellers.values().sum();
                if total == 0 {
                    continue;
                }
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
                            order.seller == MarketParty::Citizen(AgentId(seller))
                                && order.good == good
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
                    }
                    let payment = payments.entry(seller).or_default();
                    *payment = payment
                        .checked_add(received)
                        .ok_or(SimulationError::WealthOverflow)?;
                }
            } else if self
                .effective_import_threshold(good)
                .is_some_and(|threshold| price > threshold)
            {
                let wanted = closed
                    .as_ref()
                    .map_or(0, |period| {
                        period.goods[good as usize].affordable_demand_units
                    })
                    .div_ceil(2);
                let units = wanted.saturating_sub(self.caravan_units(good));
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
            // Scale before adding so finite large volumes do not overflow the imbalance ratio.
            let scale = traded.traded_units.max(supply).max(unmet) as f64;
            let demand = traded.traded_units as f64 / scale + unmet as f64 / scale;
            let available = traded.traded_units as f64 / scale + supply as f64 / scale;
            let imbalance = (demand - available) / (demand + available);
            let before = self.prices.price(good).unwrap();
            let after = (before * (1.0 + MAX_DAILY_PRICE_CHANGE * imbalance))
                .clamp(MIN_QUOTED_PRICE, f64::MAX);
            self.prices = self.prices.with_price(good, after)?;
            self.history.push_back(DailyMarketActivity {
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
        for id in self.orders.keys().copied().collect::<Vec<_>>() {
            let order = self.orders.get_mut(&id).unwrap();
            if order.seller.citizen().is_some() {
                order.quoted_price = self.prices.price(order.good).unwrap();
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const PLACE: PlaceId = PlaceId(uuid::Uuid::nil());

    #[test]
    fn tariffs_preserve_asking_receipts_and_carry_by_good_and_direction() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let buyer = AgentId(uuid::Uuid::from_u128(2));
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Berries, 2.0).unwrap();
        let policy = CaravanPolicy {
            export_tariff_basis_points: 3333,
            import_tariff_basis_points: 5000,
            ..CaravanPolicy::default()
        };
        market.set_caravan_policy(policy).unwrap();
        for day in 0..3 {
            market.list(seller, PLACE, Good::Berries, 1).unwrap();
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
        assert_eq!(market.export_tariff_carry[Good::Berries as usize], 9998);
        assert_eq!(market.export_tariff_carry[Good::Bread as usize], 0);
        assert_eq!(market.import_tariff_carry[Good::Berries as usize], 0);
        let id = OrderId(market.next_order_id);
        market.orders.insert(
            id,
            SellOrder {
                id,
                seller: MarketParty::Caravan,
                place: PLACE,
                good: Good::Berries,
                units: 1000,
                quoted_price: 3.0,
            },
        );
        market
            .set_request(buyer, ShoppingList::single(Good::Berries, 1000))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2)]);
        assert_eq!(market.affordable_request(buyer).units(Good::Berries), 666);
        assert_eq!(market.purchase_cost(buyer, Good::Berries, 666), Some(2));
        let imported = market
            .purchase(buyer, PLACE, (Good::Berries, 1000), 2, 0, true)
            .unwrap();
        assert_eq!((imported.units, imported.coins), (666, 2));
        let trade = market.trades().back().unwrap();
        assert_eq!((trade.tariff_coins, trade.recipient_coins), (1, 1));
        assert_eq!(market.export_tariff_carry[Good::Berries as usize], 9998);
    }

    #[test]
    fn tariff_rounding_does_not_depend_on_export_order_fragmentation() {
        let seller = AgentId(uuid::Uuid::from_u128(1));
        let mut whole = Market::default();
        whole.prices = whole.prices.with_price(Good::Berries, 2.0).unwrap();
        whole
            .set_caravan_policy(CaravanPolicy {
                export_tariff_basis_points: 3333,
                ..CaravanPolicy::default()
            })
            .unwrap();
        let mut fragmented = whole.clone();
        whole.list(seller, PLACE, Good::Berries, 6).unwrap();
        for _ in 0..6 {
            fragmented.list(seller, PLACE, Good::Berries, 1).unwrap();
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
        market.prices = market.prices.with_price(Good::Berries, 2.0).unwrap();
        for _ in 0..3 {
            market.list(first, PLACE, Good::Berries, 1).unwrap();
        }
        market.list(second, PLACE, Good::Berries, 2).unwrap();
        market.update(UPDATE_TIME_MS).unwrap();
        let payments = market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(market.listed_units(first, Good::Berries), 1);
        assert_eq!(market.listed_units(second, Good::Berries), 1);
        assert_eq!(payments, vec![(first, 1), (second, 1)]);
        let current = market.current_period().goods[Good::Berries as usize];
        assert_eq!((current.exported_units, current.exported_coins), (3, 2));
        assert_eq!(current.local_traded_units, 0);
        assert_eq!(
            market.previous_period().unwrap().goods[Good::Berries as usize].exported_units,
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
        whole.prices = whole.prices.with_price(Good::Berries, 2.0).unwrap();
        whole.list(second, PLACE, Good::Berries, 3).unwrap();
        whole.list(first, PLACE, Good::Berries, 3).unwrap();
        let mut fragmented = Market {
            prices: whole.prices,
            ..Market::default()
        };
        for _ in 0..3 {
            fragmented.list(second, PLACE, Good::Berries, 1).unwrap();
        }
        fragmented.list(first, PLACE, Good::Berries, 3).unwrap();
        let whole_payments = whole.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        let fragmented_payments = fragmented.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(whole_payments, fragmented_payments);
        assert_eq!(fragmented.listed_units(first, Good::Berries), 1);
        assert_eq!(fragmented.listed_units(second, Good::Berries), 2);
        for seller in [first, second] {
            assert_eq!(
                whole.listed_units(seller, Good::Berries),
                fragmented.listed_units(seller, Good::Berries)
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
            .with_price(Good::Bread, Good::Bread.export_threshold())
            .unwrap();
        exact.list(seller, PLACE, Good::Bread, 9).unwrap();
        assert!(exact.caravans(PLACE, UPDATE_TIME_MS).unwrap().is_empty());
        assert_eq!(exact.listed_units(seller, Good::Bread), 9);
        exact.prices = exact
            .prices
            .with_price(Good::Bread, Good::Bread.import_threshold())
            .unwrap();
        exact.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(exact.caravan_units(Good::Bread), 0);
        let mut market = Market::default();
        market.prices = market
            .prices
            .with_price(Good::Bread, Good::Bread.core_price() * 2.0)
            .unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::Bread, 9))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        market.update(UPDATE_TIME_MS).unwrap();
        market.caravans(PLACE, UPDATE_TIME_MS).unwrap();
        assert_eq!(market.caravan_units(Good::Bread), 5);
        assert_eq!(
            market.current_period().goods[Good::Bread as usize].imported_units,
            5
        );
        assert!(market.trades().is_empty());
        market.refresh_affordability(&[(buyer, 1000)]);
        market.update(UPDATE_TIME_MS + DAY_MS).unwrap();
        market.caravans(PLACE, UPDATE_TIME_MS + DAY_MS).unwrap();
        assert_eq!(market.caravan_units(Good::Bread), 5);
        assert!(
            market
                .orders()
                .all(|order| order.quoted_price == Good::Bread.import_threshold())
        );
        let purchase = market
            .purchase(
                buyer,
                PLACE,
                (Good::Bread, 2),
                1000,
                UPDATE_TIME_MS + DAY_MS,
                true,
            )
            .unwrap();
        assert!(purchase.payments.is_empty());
        assert_eq!(
            purchase.coins,
            charge(Good::Bread.import_threshold() * 2.0).unwrap()
        );
        let activity = market.current_period().goods[Good::Bread as usize];
        assert_eq!(
            (
                activity.local_traded_units,
                activity.caravan_purchased_units,
                activity.imported_units
            ),
            (0, 2, 0)
        );
        let before = market.clone();
        market
            .purchase(buyer, PLACE, (Good::Bread, 1), 1000, 0, false)
            .unwrap();
        assert_eq!(market.trades(), before.trades());
        assert_eq!(
            market.current_period().goods[Good::Bread as usize].caravan_purchased_units,
            2
        );
    }

    #[test]
    fn local_settlement_leaves_remote_supply_and_owner_stock_untouched() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let remote = PlaceId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market.prices = market.prices.with_price(Good::Bread, 15.0).unwrap();
        market.list(seller, PLACE, Good::Bread, 2).unwrap();
        market.list(seller, remote, Good::Bread, 3).unwrap();
        market.list(buyer, PLACE, Good::Bread, 7).unwrap();
        assert_eq!(market.available_units(buyer, Good::Bread), 5);
        assert_eq!(market.available_units_at(buyer, PLACE, Good::Bread), 2);
        let purchase = market
            .purchase(buyer, PLACE, (Good::Bread, 10), 1000, 0, true)
            .unwrap();
        assert_eq!((purchase.units, purchase.coins), (2, 30));
        assert_eq!(market.available_units_at(buyer, remote, Good::Bread), 3);
        assert_eq!(market.listed_units(buyer, Good::Bread), 7);
        assert_eq!(market.withdraw(seller, PLACE, Good::Bread, 10).unwrap(), 0);
        assert_eq!(
            market.current_period().goods[Good::Bread as usize].traded_units,
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
        market.list(seller, PLACE, Good::Bread, 10).unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::Bread, 7))
            .unwrap();
        market.refresh_affordability(&[(buyer, 1000)]);
        market
            .purchase(buyer, PLACE, (Good::Bread, 2), 1000, UPDATE_TIME_MS, true)
            .unwrap();
        market.refresh_affordability(&[(buyer, 970)]);
        let current = market.current_period();
        assert_eq!((current.start_ms, current.end_ms), (0, UPDATE_TIME_MS));
        assert_eq!(
            current.goods[Good::Bread as usize],
            GoodActivity {
                traded_units: 2,
                listed_units: 8,
                affordable_demand_units: 5,
                traded_coins: 41,
                local_traded_units: 2,
                local_traded_coins: 41,
                ..GoodActivity::default()
            }
        );
        assert!(market.previous_period().is_none());
        market.update(UPDATE_TIME_MS - 1).unwrap();
        assert!(market.previous_period().is_none());
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.previous_period().unwrap(), current);
        assert_eq!(
            market.current_period().goods[Good::Bread as usize].traded_units,
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
        market.list(seller, PLACE, Good::Water, 50).unwrap();
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
        market.list(seller, PLACE, Good::Berries, 35).unwrap();
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
        market.list(seller, PLACE, Good::Berries, 100).unwrap();
        market.list(other, PLACE, Good::Berries, 100).unwrap();
        market.list(seller, PLACE, Good::Berries, 100).unwrap();
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
    fn integer_boundary_quotes_use_direct_ceil_and_never_fractional_goods() {
        for (price, expected) in [(10.0, 1), (f64::from_bits(10.0_f64.to_bits() + 1), 2)] {
            let seller = AgentId(uuid::Uuid::new_v4());
            let buyer = AgentId(uuid::Uuid::new_v4());
            let mut market = Market::default();
            market.prices = market.prices.with_price(Good::Berries, price).unwrap();
            market.list(seller, PLACE, Good::Berries, 100).unwrap();
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
        market.list(seller, PLACE, Good::Bread, 2).unwrap();
        let before = market.clone();
        let purchase = market
            .purchase(buyer, PLACE, (Good::Bread, 2), 14, 0, true)
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
            .list(seller, PLACE, Good::Berries, Quantity::MAX)
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
            market.list(seller, PLACE, Good::Berries, 3),
            Err(SimulationError::WealthOverflow)
        );
        market.prices = market.prices.with_price(Good::Bread, f64::MAX).unwrap();
        market.list(seller, PLACE, Good::Bread, 1).unwrap();
        assert_eq!(market.purchase_cost(buyer, Good::Bread, 1), None);
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
        assert!(market.prices.price(Good::Berries).unwrap() > 5.0);
    }
}
