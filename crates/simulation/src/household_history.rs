use crate::{
    AgentId, AgentKind, Coins, Quantity, SimulationError, Universe, history::CitizenHistory,
    locations::PlaceId, marketplace::Good, storage::GoodsOwner, taxation::TaxAmount,
};
use imbl::{HashMap, Vector};

pub const COMPLETED_DAYS: usize = 365;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct MemberContribution {
    pub produced: [Quantity; Good::COUNT],
    pub consumed: [Quantity; Good::COUNT],
    pub production_ms: u64,
    pub travel_ms: u64,
    pub supply_ms: u64,
}
impl Default for MemberContribution {
    fn default() -> Self {
        Self {
            produced: [0; Good::COUNT],
            consumed: [0; Good::COUNT],
            production_ms: 0,
            travel_ms: 0,
            supply_ms: 0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct HouseholdStock {
    pub place: Option<PlaceId>,
    pub carrier: Option<AgentId>,
    pub good: Good,
    pub units: Quantity,
    pub listed_units: Quantity,
    pub tax_reserved_units: Quantity,
    pub work_reserved_units: Quantity,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct DailyHouseholdHistory {
    pub start_ms: u64,
    pub end_ms: u64,
    pub opening_coins: Coins,
    pub closing_coins: Coins,
    pub sales_receipts: Coins,
    pub purchases: Coins,
    pub taxes_paid: Coins,
    pub workshop_fees_paid: Coins,
    pub construction: crate::construction::ConstructionCash,
    pub produced: [Quantity; Good::COUNT],
    pub bought: [Quantity; Good::COUNT],
    pub sold: [Quantity; Good::COUNT],
    pub consumed: [Quantity; Good::COUNT],
    pub tax_collected: [Quantity; Good::COUNT],
    pub members: HashMap<AgentId, MemberContribution>,
    pub closing_stocks: Vec<HouseholdStock>,
}
impl DailyHouseholdHistory {
    fn new(now: u64, coins: Coins) -> Self {
        Self {
            start_ms: now,
            end_ms: now,
            opening_coins: coins,
            closing_coins: coins,
            sales_receipts: 0,
            purchases: 0,
            taxes_paid: 0,
            workshop_fees_paid: 0,
            construction: Default::default(),
            produced: [0; Good::COUNT],
            bought: [0; Good::COUNT],
            sold: [0; Good::COUNT],
            consumed: [0; Good::COUNT],
            tax_collected: [0; Good::COUNT],
            members: HashMap::new(),
            closing_stocks: Vec::new(),
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct HouseholdHistory {
    pub current: DailyHouseholdHistory,
    pub completed: Vector<DailyHouseholdHistory>,
}
impl HouseholdHistory {
    pub(crate) fn new(now: u64, coins: Coins) -> Self {
        Self {
            current: DailyHouseholdHistory::new(now, coins),
            completed: Vector::new(),
        }
    }
}
fn add(total: &mut u64, amount: u64) -> Result<(), SimulationError> {
    *total = total
        .checked_add(amount)
        .ok_or(SimulationError::InventoryOverflow)?;
    Ok(())
}
fn cash(total: &mut Coins, amount: Coins) -> Result<(), SimulationError> {
    *total = total
        .checked_add(amount)
        .ok_or(SimulationError::WealthOverflow)?;
    Ok(())
}

impl Universe {
    pub fn household_history(&self, id: AgentId) -> Option<&HouseholdHistory> {
        self.household_histories.get(&id)
    }

    pub(crate) fn record_household_contributions(
        &mut self,
        before: &HashMap<AgentId, CitizenHistory>,
    ) -> Result<(), SimulationError> {
        for (id, old) in before {
            let AgentKind::Citizen(citizen) = &self.agents[id].kind;
            let Some(household) = citizen.household_id() else {
                continue;
            };
            let new = &self.citizen_histories[id].current;
            let old = &old.current;
            let day = &mut self
                .household_histories
                .get_mut(&household)
                .unwrap()
                .current;
            let mut member = day.members.get(id).cloned().unwrap_or_default();
            for good in Good::ALL {
                let i = good as usize;
                let produced = new.produced[i] - old.produced[i];
                let consumed = new.consumed[i] - old.consumed[i];
                add(&mut day.produced[i], produced)?;
                add(&mut member.produced[i], produced)?;
                add(&mut day.consumed[i], consumed)?;
                add(&mut member.consumed[i], consumed)?;
            }
            add(
                &mut member.production_ms,
                new.activity.production_ms - old.activity.production_ms,
            )?;
            add(
                &mut member.travel_ms,
                new.activity.travel_ms - old.activity.travel_ms,
            )?;
            add(
                &mut member.supply_ms,
                new.activity.trade_ms - old.activity.trade_ms,
            )?;
            if member != MemberContribution::default() {
                day.members.insert(*id, member);
            }
        }
        Ok(())
    }

    pub(crate) fn record_household_events(
        &mut self,
        trades: usize,
        taxes: usize,
    ) -> Result<(), SimulationError> {
        for trade in self.market.trades().iter().skip(trades) {
            if let Some(day) = trade
                .buyer
                .account()
                .and_then(|id| self.household_histories.get_mut(&id))
                .map(|history| &mut history.current)
            {
                add(&mut day.bought[trade.good as usize], trade.units)?;
                cash(&mut day.purchases, trade.coins)?;
            }
            if let Some(day) = trade
                .seller
                .account()
                .and_then(|id| self.household_histories.get_mut(&id))
                .map(|history| &mut history.current)
            {
                add(&mut day.sold[trade.good as usize], trade.units)?;
                cash(&mut day.sales_receipts, trade.recipient_coins)?;
            }
        }
        for receipt in self.taxation.history.iter().skip(taxes) {
            let Some(history) = self.household_histories.get_mut(&receipt.payer) else {
                continue;
            };
            match receipt.amount {
                TaxAmount::Coins { paid, .. } => cash(&mut history.current.taxes_paid, paid)?,
                TaxAmount::Goods { collected, .. } => add(
                    &mut history.current.tax_collected[receipt.good.unwrap() as usize],
                    collected,
                )?,
                TaxAmount::Reservation { .. } => {}
            }
        }
        Ok(())
    }

    pub(crate) fn refresh_household_history(
        &mut self,
        now: u64,
        close: bool,
    ) -> Result<(), SimulationError> {
        let ids: Vec<_> = self.households.keys().copied().collect();
        for id in ids {
            if close
                && crate::production::work_period(now)
                    == crate::production::work_period(
                        self.household_histories[&id].current.start_ms,
                    )
            {
                continue;
            }
            let owner = GoodsOwner::Household(id);
            let mut stocks = Vec::new();
            for (key, units) in self.storage.stock() {
                if key.owner == owner {
                    stocks.push(HouseholdStock {
                        place: Some(key.place),
                        carrier: None,
                        good: key.good,
                        units: *units,
                        listed_units: 0,
                        tax_reserved_units: self.storage.reserved_units(key.place, owner, key.good),
                        work_reserved_units: self.storage.work_reserved_units(*key),
                    });
                }
            }
            for member in &self.households[&id].members {
                let AgentKind::Citizen(citizen) = &self.agents[member].kind;
                for good in Good::ALL {
                    let units = citizen.units(good);
                    if units != 0 {
                        stocks.push(HouseholdStock {
                            place: None,
                            carrier: Some(*member),
                            good,
                            units,
                            listed_units: 0,
                            tax_reserved_units: citizen.tax_reserved_units(good),
                            work_reserved_units: 0,
                        });
                    }
                }
            }
            for order in self
                .market
                .orders()
                .filter(|order| order.seller.account() == Some(id))
            {
                if let Some(stock) = stocks.iter_mut().find(|stock| {
                    stock.place == Some(order.place)
                        && stock.carrier.is_none()
                        && stock.good == order.good
                }) {
                    add(&mut stock.units, order.units)?;
                    add(&mut stock.listed_units, order.units)?;
                } else {
                    stocks.push(HouseholdStock {
                        place: Some(order.place),
                        carrier: None,
                        good: order.good,
                        units: order.units,
                        listed_units: order.units,
                        tax_reserved_units: 0,
                        work_reserved_units: 0,
                    });
                }
            }
            stocks.sort_by_key(|stock| {
                (
                    stock.place.map(|place| place.0),
                    stock.carrier.map(|carrier| carrier.0),
                    stock.good as usize,
                )
            });
            let history = self.household_histories.get_mut(&id).unwrap();
            history.current.end_ms = now;
            history.current.closing_coins = self.households[&id].coins;
            history.current.closing_stocks = stocks;
            if close
                && crate::production::work_period(now)
                    != crate::production::work_period(history.current.start_ms)
            {
                let next = DailyHouseholdHistory::new(now, history.current.closing_coins);
                history
                    .completed
                    .push_back(std::mem::replace(&mut history.current, next));
                if history.completed.len() > COMPLETED_DAYS {
                    history.completed.pop_front();
                }
            }
        }
        Ok(())
    }
}

impl Universe {
    pub(crate) fn validate_household_histories(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        let invalid = || SaveError::InvalidState("invalid household history");
        if self.household_histories.len() != self.households.len() {
            return Err(invalid());
        }
        for (id, household) in &self.households {
            let history = self.household_histories.get(id).ok_or_else(invalid)?;
            if history.completed.len() > COMPLETED_DAYS
                || history.current.end_ms != self.current_time_ms
                || history.current.closing_coins != household.coins
            {
                return Err(invalid());
            }
            let mut previous_end = None;
            let mut previous_coins = None;
            for day in history
                .completed
                .iter()
                .chain(std::iter::once(&history.current))
            {
                if day.start_ms > day.end_ms
                    || day.end_ms > self.current_time_ms
                    || day.end_ms - day.start_ms > crate::marketplace::DAY_MS
                    || previous_end.is_some_and(|end| end != day.start_ms)
                    || previous_coins.is_some_and(|coins| coins != day.opening_coins)
                    || [
                        day.opening_coins,
                        day.closing_coins,
                        day.sales_receipts,
                        day.purchases,
                        day.taxes_paid,
                        day.workshop_fees_paid,
                        day.construction.escrow_deposited,
                        day.construction.escrow_refunded,
                        day.construction.payments_received,
                        day.construction.payments_paid,
                    ]
                    .iter()
                    .any(|coins| *coins < 0)
                    || i128::from(day.opening_coins) + i128::from(day.sales_receipts)
                        - i128::from(day.purchases)
                        - i128::from(day.taxes_paid)
                        - i128::from(day.workshop_fees_paid)
                        - i128::from(day.construction.escrow_deposited)
                        + i128::from(day.construction.escrow_refunded)
                        + i128::from(day.construction.payments_received)
                        != i128::from(day.closing_coins)
                {
                    return Err(invalid());
                }
                if day != &history.current
                    && (crate::production::work_period(day.start_ms) + 1
                        != crate::production::work_period(day.end_ms)
                        || crate::production::period_start(crate::production::work_period(
                            day.end_ms,
                        )) != day.end_ms)
                {
                    return Err(invalid());
                }
                let mut produced = [0u128; Good::COUNT];
                let mut consumed = [0u128; Good::COUNT];
                for (member, contribution) in &day.members {
                    if !household.members.contains(member)
                        || contribution
                            .production_ms
                            .checked_add(contribution.travel_ms)
                            .and_then(|sum| sum.checked_add(contribution.supply_ms))
                            .is_none_or(|sum| sum > day.end_ms - day.start_ms)
                    {
                        return Err(invalid());
                    }
                    for good in Good::ALL {
                        produced[good as usize] += u128::from(contribution.produced[good as usize]);
                        consumed[good as usize] += u128::from(contribution.consumed[good as usize]);
                    }
                }
                for good in Good::ALL {
                    let i = good as usize;
                    if produced[i] != u128::from(day.produced[i])
                        || consumed[i] != u128::from(day.consumed[i])
                    {
                        return Err(invalid());
                    }
                }
                let mut keys = std::collections::HashSet::new();
                for stock in &day.closing_stocks {
                    if stock.units == 0
                        || stock.place.is_some() == stock.carrier.is_some()
                        || stock
                            .place
                            .is_some_and(|place| self.map.place(place).is_err())
                        || stock
                            .carrier
                            .is_some_and(|carrier| !household.members.contains(&carrier))
                        || stock.carrier.is_some() && stock.listed_units != 0
                        || stock
                            .listed_units
                            .checked_add(stock.tax_reserved_units)
                            .and_then(|sum| sum.checked_add(stock.work_reserved_units))
                            .is_none_or(|sum| sum > stock.units)
                        || !keys.insert((stock.place, stock.carrier, stock.good))
                    {
                        return Err(invalid());
                    }
                }
                previous_end = Some(day.end_ms);
                previous_coins = Some(day.closing_coins);
            }
            if crate::production::work_period(history.current.start_ms)
                != crate::production::work_period(self.current_time_ms)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ACTION_DURATION_MS, Citizen, CitizenAction, TRADE_DURATION_MS,
        locations::{Location, Map},
        marketplace::{DAY_MS, Prices, ShoppingList, UPDATE_TIME_MS},
        production::Recipe,
        taxation::{TaxKind, TaxRate},
    };

    fn household(world: Universe, name: &str, citizen: Citizen) -> (Universe, AgentId, AgentId) {
        let (world, member) = world.with_citizen(name, citizen).unwrap();
        let (world, second) = world
            .with_citizen("Other member", Citizen::new(0.0).unwrap())
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&member].kind;
        let (world, id) = world
            .with_household(name, citizen.home(), &[member, second])
            .unwrap();
        (world, member, id)
    }
    fn base(citizen: Citizen) -> (Universe, AgentId, AgentId) {
        household(Universe::with_map(Map::default()), "Family", citizen)
    }
    fn round_trip(world: &Universe) {
        assert_eq!(
            *world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn shared_trades_record_household_cash_once_and_income_tax_separately() {
        let world = Universe::with_map(Map::default()).with_prices(Prices::new(100.0).unwrap());
        let (world, seller, seller_household) = household(
            world,
            "Seller",
            Citizen::new(0.0).unwrap().with_berries(500).unwrap(),
        );
        let (world, buyer, buyer_household) = household(
            world,
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(100).unwrap(),
        );
        let (world, place) = world.with_property(seller, Location::Bakery).unwrap();
        let world = world.with_designated_workplace(seller, place).unwrap();
        let (world, _) = world
            .with_tax_rule(
                place,
                "Income",
                TaxKind::Income {
                    rates: [(Good::Berries, TaxRate::new(5000).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let world = world
            .start_action(seller, CitizenAction::List(Good::Berries, 500))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let listed = &world.household_history(seller_household).unwrap().current;
        assert_eq!(listed.sold[Good::Berries as usize], 0);
        assert_eq!(
            listed
                .closing_stocks
                .iter()
                .map(|stock| stock.listed_units)
                .sum::<u64>(),
            500
        );
        let travel = world
            .start_action(buyer, CitizenAction::Travel(place))
            .unwrap();
        let duration = match &travel.agents[&buyer].kind {
            AgentKind::Citizen(citizen) => citizen
                .active_action()
                .map_or(0, |active| active.remaining_ms()),
        };
        let world = travel.advance(duration).unwrap();
        let started = world
            .start_action(
                buyer,
                CitizenAction::BuyAt {
                    place,
                    list: ShoppingList::single(Good::Berries, 200),
                },
            )
            .unwrap();
        let whole = started.advance(TRADE_DURATION_MS).unwrap();
        let split = started
            .advance(TRADE_DURATION_MS / 2)
            .unwrap()
            .advance(TRADE_DURATION_MS / 2)
            .unwrap();
        for id in [seller_household, buyer_household] {
            assert_eq!(whole.household_history(id), split.household_history(id));
        }
        let seller_day = &whole.household_history(seller_household).unwrap().current;
        let buyer_day = &whole.household_history(buyer_household).unwrap().current;
        assert!(seller_day.sales_receipts > 0);
        assert_eq!(seller_day.sales_receipts, buyer_day.purchases);
        assert_eq!(seller_day.taxes_paid, seller_day.sales_receipts / 2);
        assert_eq!(
            seller_day.closing_coins,
            seller_day.sales_receipts - seller_day.taxes_paid
        );
        assert_eq!(buyer_day.closing_coins, 100 - buyer_day.purchases);
        assert_eq!(seller_day.sold, buyer_day.bought);
        assert_eq!(
            whole.citizen_history(seller).unwrap().current.coins_earned,
            0
        );
        round_trip(&whole);
    }

    #[test]
    fn production_consumption_and_internal_transfers_reuse_member_totals() {
        let (world, member, id) = base(Citizen::new(0.0).unwrap().with_berries(1000).unwrap());
        let home = world.households[&id].home;
        let world = world
            .deposit_goods(member, home, Good::Berries, 400)
            .unwrap();
        let started = world
            .start_action(
                member,
                CitizenAction::CollectStored {
                    place: home,
                    list: ShoppingList::single(Good::Berries, 200),
                },
            )
            .unwrap();
        let moved = started.advance(TRADE_DURATION_MS).unwrap();
        let day = &moved.household_history(id).unwrap().current;
        assert_eq!(day.produced, [0; Good::COUNT]);
        assert_eq!(day.consumed, [0; Good::COUNT]);
        assert_eq!(day.bought, [0; Good::COUNT]);
        assert_eq!(day.sold, [0; Good::COUNT]);
        assert_eq!(day.members[&member].supply_ms, TRADE_DURATION_MS);
        let started = moved
            .start_action(member, CitizenAction::Produce(Recipe::Forage))
            .unwrap();
        let whole = started.advance(ACTION_DURATION_MS).unwrap();
        let split = started
            .advance(ACTION_DURATION_MS / 2)
            .unwrap()
            .advance(ACTION_DURATION_MS / 2)
            .unwrap();
        assert_eq!(whole.household_history(id), split.household_history(id));
        let day = &whole.household_history(id).unwrap().current;
        assert_eq!(
            day.produced,
            whole.citizen_history(member).unwrap().current.produced
        );
        assert_eq!(day.members[&member].production_ms, ACTION_DURATION_MS);
        let eating = whole.start_action(member, CitizenAction::Eat).unwrap();
        let day = &eating.household_history(id).unwrap().current;
        assert!(day.consumed[Good::Berries as usize] > 0);
        assert_eq!(
            day.consumed,
            eating.citizen_history(member).unwrap().current.consumed
        );
        round_trip(&eating);
    }

    #[test]
    fn four_am_closes_contributions_before_new_day_and_keeps_365_complete_days() {
        let (world, member, id) = base(Citizen::new(0.0).unwrap());
        let world = world
            .advance(UPDATE_TIME_MS - ACTION_DURATION_MS)
            .unwrap()
            .start_action(member, CitizenAction::Produce(Recipe::Forage))
            .unwrap();
        let whole = world.advance(ACTION_DURATION_MS).unwrap();
        let split = world
            .advance(ACTION_DURATION_MS - 1)
            .unwrap()
            .advance(1)
            .unwrap();
        assert_eq!(whole.household_history(id), split.household_history(id));
        let history = whole.household_history(id).unwrap();
        assert!(history.completed[0].produced[Good::Berries as usize] > 0);
        assert_eq!(history.current.produced, [0; Good::COUNT]);
        assert_eq!(history.current.start_ms, UPDATE_TIME_MS);
        round_trip(&whole);
        // Close synthetic idle days directly to exercise retention without running planners.
        let mut world = whole;
        for day in 1..=370 {
            let now = UPDATE_TIME_MS + day * DAY_MS;
            world.current_time_ms = now;
            world.refresh_household_history(now, true).unwrap();
        }
        world
            .refresh_household_history(world.current_time_ms, false)
            .unwrap();
        let history = world.household_history(id).unwrap();
        assert_eq!(history.completed.len(), 365);
        assert_eq!(
            history.completed.front().unwrap().start_ms,
            UPDATE_TIME_MS + 5 * DAY_MS
        );
        world.validate_household_histories().unwrap();
    }

    #[test]
    fn stock_reservations_and_collected_socage_are_distinct_from_consumption() {
        let (world, member, id) = base(Citizen::new(0.0).unwrap());
        let home = world.households[&id].home;
        let world = world
            .with_stored_good(home, GoodsOwner::Household(id), Good::Berries, 100)
            .unwrap();
        let (mut world, _) = world
            .with_tax_rule(
                home,
                "Socage",
                TaxKind::Socage {
                    rates: [(Good::Berries, TaxRate::new(5000).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let before = world.taxation.history.len();
        world
            .tax_production(member, home, Good::Berries, 100, 0)
            .unwrap();
        world.record_household_events(0, before).unwrap();
        world.refresh_household_history(0, false).unwrap();
        let day = &world.household_history(id).unwrap().current;
        assert_eq!(day.tax_collected[Good::Berries as usize], 0);
        assert_eq!(day.closing_stocks[0].tax_reserved_units, 50);
        let before = world.taxation.history.len();
        world.settle_tax_reservations(0).unwrap();
        world.record_household_events(0, before).unwrap();
        world.refresh_household_history(0, false).unwrap();
        let day = &world.household_history(id).unwrap().current;
        assert_eq!(day.tax_collected[Good::Berries as usize], 50);
        assert_eq!(day.consumed[Good::Berries as usize], 0);
        assert_eq!(day.closing_stocks[0].units, 50);
        assert_eq!(day.closing_stocks[0].tax_reserved_units, 0);
        round_trip(&world);
    }

    #[test]
    fn stored_recipe_inputs_and_work_reservations_match_physical_stock() {
        let (world, member, id) = base(
            Citizen::new(0.0)
                .unwrap()
                .with_starting_role(crate::StartingRole::Baker),
        );
        let (mut world, bakery) = world.with_property(member, Location::Bakery).unwrap();
        for &(good, units) in Recipe::BakeRolls.inputs() {
            world = world
                .with_stored_good(bakery, GoodsOwner::Household(id), good, units)
                .unwrap();
        }
        let started = world
            .start_action(
                member,
                CitizenAction::ProduceAt {
                    recipe: Recipe::BakeRolls,
                    place: bakery,
                },
            )
            .unwrap();
        for &(good, units) in Recipe::BakeRolls.inputs() {
            let stock = started
                .household_history(id)
                .unwrap()
                .current
                .closing_stocks
                .iter()
                .find(|stock| stock.place == Some(bakery) && stock.good == good)
                .unwrap();
            assert_eq!(stock.work_reserved_units, units);
        }
        let duration = match &started.agents[&member].kind {
            AgentKind::Citizen(citizen) => citizen.active_action().unwrap().remaining_ms(),
        };
        let completed = started.advance(duration).unwrap();
        let day = &completed.household_history(id).unwrap().current;
        for &(good, units) in Recipe::BakeRolls.inputs() {
            assert_eq!(day.consumed[good as usize], units);
        }
        assert!(day.produced[Good::BreadRoll as usize] > 0);
        round_trip(&completed);
    }

    #[test]
    #[ignore = "writes a varied synthetic 365-day history for an explicit gzip size measurement"]
    fn representative_history_size_sample() {
        let (world, member, id) = base(Citizen::new(0.0).unwrap().with_coins(1000).unwrap());
        let home = world.households[&id].home;
        let second = AgentId(uuid::Uuid::new_v4());
        let mut history = HouseholdHistory::new(UPDATE_TIME_MS, 1000);
        let mut balance = 1000;
        for index in 0..365u64 {
            let mut day = DailyHouseholdHistory::new(UPDATE_TIME_MS + index * DAY_MS, balance);
            day.end_ms = day.start_ms + DAY_MS;
            day.sales_receipts = 20 + ((index * 37 + 7) % 101) as Coins;
            day.purchases = 15 + ((index * 19 + 5) % 50) as Coins;
            day.taxes_paid = day.sales_receipts / 10;
            balance += day.sales_receipts - day.purchases - day.taxes_paid;
            day.closing_coins = balance;
            for (offset, member) in [member, second].into_iter().enumerate() {
                let mut contribution = MemberContribution {
                    production_ms: (4 + index % 5) * 3_600_000,
                    travel_ms: (index % 7) * 600_000,
                    supply_ms: (index % 11) * 300_000,
                    ..Default::default()
                };
                for good in Good::ALL.into_iter().take(8) {
                    let i = good as usize;
                    contribution.produced[i] =
                        (index * 17 + i as u64 * 31 + offset as u64 * 73) % 1500;
                    contribution.consumed[i] =
                        (index * 23 + i as u64 * 13 + offset as u64 * 43) % 800;
                    day.produced[i] += contribution.produced[i];
                    day.consumed[i] += contribution.consumed[i];
                    day.bought[i] = (index * 11 + i as u64 * 7) % 600;
                    day.sold[i] = (index * 29 + i as u64 * 41) % 900;
                    day.tax_collected[i] = day.produced[i] / 10;
                    day.closing_stocks.push(HouseholdStock {
                        place: (offset == 0).then_some(home),
                        carrier: (offset == 1).then_some(member),
                        good,
                        units: 300 + (index * 47 + i as u64 * 29) % 2000,
                        listed_units: if offset == 0 { (index * 7) % 200 } else { 0 },
                        tax_reserved_units: index % 23,
                        work_reserved_units: 0,
                    });
                }
                day.members.insert(member, contribution);
            }
            history.completed.push_back(day);
        }
        history.current = DailyHouseholdHistory::new(UPDATE_TIME_MS + 365 * DAY_MS, balance);
        let encoded =
            ron::ser::to_string_pretty(&history, ron::ser::PrettyConfig::default()).unwrap();
        let path = std::env::temp_dir().join("learning-lord-household-history-size.ron");
        std::fs::write(&path, &encoded).unwrap();
        println!(
            "Synthetic sample: one household, two members, eight goods, sixteen nonzero stock entries/day, 365 varied complete days. Raw {} bytes; {}",
            encoded.len(),
            path.display()
        );
    }

    #[test]
    fn invalid_history_cash_members_stock_and_retention_are_rejected() {
        let (world, member, id) = base(Citizen::new(0.0).unwrap().with_berries(100).unwrap());
        for case in 0..6 {
            let mut invalid = world.clone();
            let history = invalid.household_histories.get_mut(&id).unwrap();
            match case {
                0 => history.current.sales_receipts = 1,
                1 => {
                    history
                        .current
                        .members
                        .insert(AgentId(uuid::Uuid::new_v4()), MemberContribution::default());
                }
                2 => history.current.closing_stocks[0].place = Some(world.households[&id].home),
                3 => history.current.closing_stocks[0].tax_reserved_units = 101,
                4 => {
                    history.current.members.insert(
                        member,
                        MemberContribution {
                            production_ms: u64::MAX,
                            ..Default::default()
                        },
                    );
                }
                _ => {
                    for _ in 0..366 {
                        history.completed.push_back(history.current.clone());
                    }
                }
            }
            assert!(
                Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err(),
                "case {case}"
            );
        }
    }
}
