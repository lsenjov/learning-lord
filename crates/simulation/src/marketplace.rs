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
            Self::Berries => Some(crate::BERRY_NUTRITION_PER_GRAM),
            Self::Bread => Some(0.5),
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
            coins_per_kg: [0.05, 0.4, 1.0, 0.5, 0.1, 1.5, 2.0],
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GoodActivity {
    pub traded_grams: f64,
    pub listed_grams: f64,
    pub affordable_demand_grams: f64,
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
    orders: HashMap<OrderId, SellOrder>,
    trades: Vector<Trade>,
    next_order_id: u64,
    requests: HashMap<AgentId, ShoppingList>,
    affordable_requests: HashMap<AgentId, ShoppingList>,
    history: Vector<DailyMarketActivity>,
    period_start_ms: u64,
    first_period_start_ms: u64,
    period_trades: [TradedVolume; Good::COUNT],
}

pub(crate) struct Purchase {
    pub grams: f64,
    pub coins: f64,
    pub payments: Vec<(AgentId, f64)>,
}

impl Market {
    pub(crate) fn starting_at(time_ms: u64) -> Self {
        let period_start_ms =
            crate::production::period_start(crate::production::work_period(time_ms));
        Self {
            period_start_ms,
            first_period_start_ms: period_start_ms,
            ..Self::default()
        }
    }

    /// Current activity; the end is the next scheduled 04:00 close.
    pub fn current_period(&self) -> MarketPeriod {
        let mut goods = [GoodActivity::default(); Good::COUNT];
        for good in Good::ALL {
            goods[good as usize].traded_grams = self.period_trades[good as usize].grams;
        }
        for order in self.orders() {
            goods[order.good as usize].listed_grams += order.grams;
        }
        for request in self.affordable_requests.values() {
            for (good, grams) in request.items() {
                goods[good as usize].affordable_demand_grams += grams;
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
                traded_grams: activity.traded_grams,
                listed_grams: activity.remaining_supply_grams,
                affordable_demand_grams: activity.unmet_demand_grams,
            };
        }
        Some(MarketPeriod {
            start_ms: self.period_start_ms.saturating_sub(DAY_MS),
            end_ms: self.period_start_ms,
            goods,
        })
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
    pub fn unmet_grams(&self, good: Good) -> f64 {
        self.affordable_requests
            .values()
            .map(|list| list.grams(good))
            .sum()
    }

    pub fn estimated_purchase_cost(&self, buyer: AgentId, good: Good, grams: f64) -> Option<f64> {
        if !grams.is_finite() || grams < 0.0 {
            return None;
        }
        let available = self.available_grams(buyer, good).min(grams);
        let cost = self.purchase_cost(buyer, good, available)?
            + self.prices.value(good, (grams - available).max(0.0))?;
        cost.is_finite().then_some(cost)
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
            if order.grams - grams == order.grams {
                // Sub-ULP requests cannot transfer stock, so leave their funds and history untouched.
                continue;
            }
            if !(result.grams + grams).is_finite() || !(result.coins + cost).is_finite() {
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
        for id in self.orders.keys().copied().collect::<Vec<_>>() {
            let order = self.orders.get_mut(&id).unwrap();
            order.coins_per_kg = self.prices.coins_per_kg(order.good).unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

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
        market.list(seller, Good::Bread, 1_000.0).unwrap();
        market
            .set_request(buyer, ShoppingList::single(Good::Bread, 700.0))
            .unwrap();
        market.refresh_affordability(&[(buyer, 10.0)]);
        market
            .purchase(buyer, Good::Bread, 200.0, 10.0, UPDATE_TIME_MS, true)
            .unwrap();
        market.refresh_affordability(&[(buyer, 9.7)]);
        let current = market.current_period();
        assert_eq!((current.start_ms, current.end_ms), (0, UPDATE_TIME_MS));
        assert_eq!(
            current.goods[Good::Bread as usize],
            GoodActivity {
                traded_grams: 200.0,
                listed_grams: 800.0,
                affordable_demand_grams: 500.0,
            }
        );
        assert!(market.previous_period().is_none());
        market.update(UPDATE_TIME_MS - 1).unwrap();
        assert!(market.previous_period().is_none());
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(market.previous_period().unwrap(), current);
        assert_eq!(
            market.current_period().goods[Good::Bread as usize].traded_grams,
            0.0
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
        market.list(seller, Good::Water, 50.0).unwrap();
        market.update(UPDATE_TIME_MS).unwrap();
        assert_eq!(
            market.previous_period().unwrap().goods[Good::Water as usize].listed_grams,
            50.0
        );
        market.withdraw(seller, Good::Water, 50.0).unwrap();
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
    fn sub_ulp_purchases_preserve_stock_funds_requests_and_history() {
        let seller = AgentId(uuid::Uuid::new_v4());
        let buyer = AgentId(uuid::Uuid::new_v4());
        let mut market = Market::default();
        market
            .list(seller, Good::Berries, 35.48387096774195)
            .unwrap();
        let grams = 0.000000000000003552713678800501;
        market
            .set_request(buyer, ShoppingList::single(Good::Berries, grams))
            .unwrap();
        market.refresh_affordability(&[(buyer, 2.0)]);
        let before = market.clone();
        let purchase = market
            .purchase(buyer, Good::Berries, grams, 2.0, 0, true)
            .unwrap();
        assert_eq!(purchase.grams, 0.0);
        assert_eq!(purchase.coins, 0.0);
        assert!(purchase.payments.is_empty());
        assert_eq!(market, before);
    }
}
