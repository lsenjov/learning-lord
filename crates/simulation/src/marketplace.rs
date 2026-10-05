use crate::{AgentId, SimulationError};
use imbl::{HashMap, Vector};

pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub const UPDATE_TIME_MS: u64 = 4 * 60 * 60 * 1000;
pub const MAX_DAILY_PRICE_CHANGE: f64 = 0.10;
pub const MIN_COINS_PER_KG: f64 = 0.0001;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Good {
    Berries,
    Wheat,
    Flour,
    Wood,
    Water,
    Bread,
    BerryPie,
}

impl Good {
    pub const COUNT: usize = 7;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Berries,
        Self::Wheat,
        Self::Flour,
        Self::Wood,
        Self::Water,
        Self::Bread,
        Self::BerryPie,
    ];
    pub const FOOD: [Self; 3] = [Self::Berries, Self::Bread, Self::BerryPie];
    pub fn nutrition_per_gram(self) -> Option<f64> {
        match self {
            Self::Berries | Self::Bread => Some(0.5),
            Self::BerryPie => Some(0.6),
            _ => None,
        }
    }
    pub fn eating_ms_per_gram(self) -> Option<f64> {
        self.nutrition_per_gram().map(|_| 1000.0)
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
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShoppingList {
    grams: [f64; Good::COUNT],
}

impl Default for ShoppingList {
    fn default() -> Self {
        Self {
            grams: [0.0; Good::COUNT],
        }
    }
}

impl ShoppingList {
    pub fn new(items: impl IntoIterator<Item = (Good, f64)>) -> Result<Self, SimulationError> {
        let mut list = Self::default();
        for (good, grams) in items {
            validate_quantity(grams)?;
            list.grams[good as usize] += grams;
        }
        list.validate()?;
        Ok(list)
    }

    pub fn single(good: Good, grams: f64) -> Self {
        let mut list = Self::default();
        list.grams[good as usize] = grams;
        list
    }

    pub fn grams(self, good: Good) -> f64 {
        self.grams[good as usize]
    }

    /// Purchases use catalogue order when the budget cannot fill the entire list.
    pub fn items(self) -> impl Iterator<Item = (Good, f64)> {
        Good::ALL
            .into_iter()
            .map(move |good| (good, self.grams[good as usize]))
            .filter(|(_, grams)| *grams > 0.0)
    }

    pub(crate) fn validate(self) -> Result<(), SimulationError> {
        for grams in self.grams {
            validate_quantity(grams)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prices {
    coins_per_kg: [f64; Good::COUNT],
}
impl Default for Prices {
    fn default() -> Self {
        Self {
            coins_per_kg: [1.0, 0.4, 1.0, 0.5, 0.1, 1.5, 2.0],
        }
    }
}
impl Prices {
    pub fn new(berries: f64) -> Result<Self, SimulationError> {
        Self::default().with_price(Good::Berries, berries)
    }
    pub fn with_price(self, good: Good, price: f64) -> Result<Self, SimulationError> {
        if !price.is_finite() || price < MIN_COINS_PER_KG {
            return Err(SimulationError::InvalidPrices);
        }
        let mut prices = self;
        prices.coins_per_kg[good as usize] = price;
        Ok(prices)
    }
    pub fn coins_per_kg(self, good: Good) -> Option<f64> {
        Some(self.coins_per_kg[good as usize])
    }
    pub fn value(self, good: Good, grams: f64) -> Option<f64> {
        self.coins_per_kg(good).map(|price| grams / 1000.0 * price)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OrderId(pub u64);

#[derive(Clone, Debug, PartialEq)]
pub struct SellOrder {
    pub id: OrderId,
    pub seller: AgentId,
    pub good: Good,
    pub grams: f64,
    pub coins_per_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    pub time_ms: u64,
    pub buyer: AgentId,
    pub seller: AgentId,
    pub good: Good,
    pub grams: f64,
    pub coins: f64,
    pub coins_per_kg: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TradedVolume {
    grams: f64,
    coins: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DailyMarketActivity {
    pub start_ms: u64,
    pub end_ms: u64,
    pub good: Good,
    pub traded_grams: f64,
    pub traded_coins: f64,
    pub remaining_supply_grams: f64,
    pub unmet_demand_grams: f64,
    pub price_before: f64,
    pub price_after: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Market {
    pub prices: Prices,
    orders: HashMap<OrderId, SellOrder>,
    trades: Vector<Trade>,
    next_order_id: u64,
    requests: HashMap<AgentId, ShoppingList>,
    affordable_requests: HashMap<AgentId, ShoppingList>,
    history: Vector<DailyMarketActivity>,
    period_start_ms: u64,
    period_trades: [TradedVolume; Good::COUNT],
}

pub(crate) struct Purchase {
    pub grams: f64,
    pub coins: f64,
    pub payments: Vec<(AgentId, f64)>,
}

impl Market {
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
        request.validate()?;
        if request.items().next().is_none() {
            self.requests.remove(&agent);
        } else {
            self.requests.insert(agent, request);
        }
        Ok(())
    }

    pub(crate) fn refresh_affordability(&mut self, budgets: &[(AgentId, f64)]) {
        let mut affordable = HashMap::new();
        for &(agent, coins) in budgets {
            let request = self.requested(agent);
            let mut funds = coins.max(0.0);
            let mut items = Vec::new();
            for (good, requested) in request.items() {
                let mut remaining = requested;
                let mut grams = 0.0;
                let mut orders: Vec<_> = self
                    .orders()
                    .filter(|o| o.seller != agent && o.good == good)
                    .collect();
                orders.sort_by(|a, b| {
                    a.coins_per_kg
                        .total_cmp(&b.coins_per_kg)
                        .then_with(|| a.id.0.cmp(&b.id.0))
                });
                for order in orders {
                    let amount = remaining
                        .min(order.grams)
                        .min(funds / order.coins_per_kg * 1000.0);
                    let cost = (amount / 1000.0 * order.coins_per_kg).min(funds);
                    if amount <= 0.0 || cost <= 0.0 {
                        continue;
                    }
                    grams += amount;
                    remaining = (remaining - amount).max(0.0);
                    funds = (funds - cost).max(0.0);
                    if remaining <= 0.0 || funds <= 0.0 {
                        break;
                    }
                }
                if remaining > 0.0 && funds > 0.0 {
                    let price = self.prices.coins_per_kg(good).unwrap();
                    let amount = remaining.min(funds / price * 1000.0);
                    let cost = (amount / 1000.0 * price).min(funds);
                    if cost > 0.0 {
                        grams += amount;
                        funds = (funds - cost).max(0.0);
                    }
                }
                if grams > 0.0 {
                    items.push((good, grams.min(requested)));
                }
            }
            let mut list = ShoppingList::default();
            for (good, grams) in items {
                list.grams[good as usize] = grams;
            }
            if list.items().next().is_some() {
                affordable.insert(agent, list);
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
    pub fn listed_grams(&self, owner: AgentId, good: Good) -> f64 {
        self.orders()
            .filter(|o| o.seller == owner && o.good == good)
            .map(|o| o.grams)
            .sum()
    }
    pub fn available_grams(&self, buyer: AgentId, good: Good) -> f64 {
        self.orders()
            .filter(|o| o.seller != buyer && o.good == good)
            .map(|o| o.grams)
            .sum()
    }
    pub fn purchase_cost(&self, buyer: AgentId, good: Good, requested: f64) -> Option<f64> {
        if !requested.is_finite() || requested < 0.0 {
            return None;
        }
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| o.seller != buyer && o.good == good)
            .collect();
        orders.sort_by(|a, b| {
            a.coins_per_kg
                .total_cmp(&b.coins_per_kg)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        let mut remaining = requested;
        let mut cost = 0.0;
        for order in orders {
            let grams = remaining.min(order.grams);
            cost += grams / 1000.0 * order.coins_per_kg;
            remaining -= grams;
            if remaining <= 0.0 {
                break;
            }
        }
        cost.is_finite().then_some(cost)
    }
    pub(crate) fn list(
        &mut self,
        seller: AgentId,
        good: Good,
        grams: f64,
    ) -> Result<(), SimulationError> {
        validate_quantity(grams)?;
        if grams == 0.0 {
            return Ok(());
        }
        let price = self
            .prices
            .coins_per_kg(good)
            .ok_or(SimulationError::InvalidPrices)?;
        if !(self.listed_grams(seller, good) + grams).is_finite()
            || !self.prices.value(good, grams).is_some_and(f64::is_finite)
        {
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
                seller,
                good,
                grams,
                coins_per_kg: price,
            },
        );
        Ok(())
    }
    pub(crate) fn withdraw(
        &mut self,
        owner: AgentId,
        good: Good,
        requested: f64,
    ) -> Result<f64, SimulationError> {
        validate_quantity(requested)?;
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| o.seller == owner && o.good == good)
            .cloned()
            .collect();
        orders.sort_by_key(|o| o.id.0);
        let mut remaining = requested;
        let mut withdrawn = 0.0;
        for mut order in orders {
            let grams = remaining.min(order.grams);
            if grams <= 0.0 {
                break;
            }
            if order.grams - grams == order.grams || !(withdrawn + grams).is_finite() {
                return Err(SimulationError::WealthOverflow);
            }
            remaining -= grams;
            withdrawn += grams;
            order.grams -= grams;
            if order.grams <= 0.0 {
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
        good: Good,
        requested: f64,
        budget: f64,
        time_ms: u64,
        record: bool,
    ) -> Result<Purchase, SimulationError> {
        validate_quantity(requested)?;
        let mut orders: Vec<_> = self
            .orders()
            .filter(|o| o.seller != buyer && o.good == good)
            .cloned()
            .collect();
        orders.sort_by(|a, b| {
            a.coins_per_kg
                .total_cmp(&b.coins_per_kg)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        let mut result = Purchase {
            grams: 0.0,
            coins: 0.0,
            payments: Vec::new(),
        };
        let mut remaining = requested;
        let mut funds = budget.max(0.0);
        for mut order in orders {
            let grams = remaining
                .min(order.grams)
                .min(funds / order.coins_per_kg * 1000.0);
            let cost = (grams / 1000.0 * order.coins_per_kg).min(funds);
            if grams <= 0.0 || cost <= 0.0 {
                continue;
            }
            if order.grams - grams == order.grams
                || !(result.grams + grams).is_finite()
                || !(result.coins + cost).is_finite()
            {
                return Err(SimulationError::WealthOverflow);
            }
            result.grams += grams;
            result.coins += cost;
            result.payments.push((order.seller, cost));
            remaining -= grams;
            funds -= cost;
            order.grams -= grams;
            if record {
                let volume = &mut self.period_trades[good as usize];
                volume.grams += grams;
                volume.coins += cost;
                if !volume.grams.is_finite() || !volume.coins.is_finite() {
                    return Err(SimulationError::WealthOverflow);
                }
                if let Some(request) = self.requests.get_mut(&buyer) {
                    request.grams[good as usize] = (request.grams[good as usize] - grams).max(0.0);
                }
                self.trades.push_back(Trade {
                    time_ms,
                    buyer,
                    seller: order.seller,
                    good,
                    grams,
                    coins: cost,
                    coins_per_kg: order.coins_per_kg,
                });
            }
            if order.grams <= 0.0 {
                self.orders.remove(&order.id);
            } else {
                self.orders.insert(order.id, order);
            }
            if remaining <= 0.0 || funds <= 0.0 {
                break;
            }
        }
        Ok(result)
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
            let traded = self.period_trades[good as usize];
            let supply: f64 = self
                .orders()
                .filter(|o| o.good == good)
                .map(|o| o.grams)
                .sum();
            let unmet: f64 = self
                .affordable_requests
                .values()
                .map(|list| list.grams(good))
                .sum();
            if !supply.is_finite() || !unmet.is_finite() {
                return Err(SimulationError::WealthOverflow);
            }
            if traded.grams == 0.0 && supply == 0.0 && unmet == 0.0 {
                continue;
            }
            // Scale before adding so finite large volumes do not overflow the imbalance ratio.
            let scale = traded.grams.max(supply).max(unmet);
            let demand = traded.grams / scale + unmet / scale;
            let available = traded.grams / scale + supply / scale;
            let imbalance = (demand - available) / (demand + available);
            let before = self.prices.coins_per_kg(good).unwrap();
            let after = (before * (1.0 + MAX_DAILY_PRICE_CHANGE * imbalance))
                .clamp(MIN_COINS_PER_KG, f64::MAX);
            self.prices = self.prices.with_price(good, after)?;
            self.history.push_back(DailyMarketActivity {
                start_ms: self.period_start_ms,
                end_ms: boundary,
                good,
                traded_grams: traded.grams,
                traded_coins: traded.coins,
                remaining_supply_grams: supply,
                unmet_demand_grams: unmet,
                price_before: before,
                price_after: after,
            });
        }
        self.period_trades = [TradedVolume::default(); Good::COUNT];
        self.period_start_ms = boundary;
        Ok(())
    }
}

pub(crate) fn validate_quantity(grams: f64) -> Result<(), SimulationError> {
    if !grams.is_finite() || grams < 0.0 {
        return Err(SimulationError::InvalidQuantity);
    }
    Ok(())
}

pub(crate) fn until_update(time_ms: u64) -> u64 {
    let time_of_day = time_ms % DAY_MS;
    if time_of_day < UPDATE_TIME_MS {
        UPDATE_TIME_MS - time_of_day
    } else {
        DAY_MS - time_of_day + UPDATE_TIME_MS
    }
}
