use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe,
    locations::{Location, Map},
    marketplace::{DAY_MS, Good, MarketParty, Prices, ShoppingList, UPDATE_TIME_MS},
    taxation::{TaxKind, TaxRate},
};

fn citizen(world: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen
}

#[test]
fn daily_exports_credit_seller_tax_origin_and_open_new_history_without_changing_source() {
    let map = Map::default();
    let prices = Prices::default().with_price(Good::Bread, 5.0).unwrap();
    let (world, seller) = Universe::with_map(map)
        .with_prices(prices)
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 9)
                .unwrap(),
        )
        .unwrap();
    let (world, market) = world.with_property(seller, Location::Bakery).unwrap();
    let (world, _) = world
        .with_tax_rule(
            market,
            "Income",
            TaxKind::Income {
                rates: [(Good::Bread, TaxRate::new(2000).unwrap())]
                    .into_iter()
                    .collect(),
            },
        )
        .unwrap();
    let listed = world
        .start_action(seller, CitizenAction::List(Good::Bread, 9))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let before = listed
        .advance(UPDATE_TIME_MS - TRADE_DURATION_MS - 1)
        .unwrap();
    assert!(before.market().trades().is_empty());
    let after = before.advance(1).unwrap();
    assert_eq!(after.agents().len(), 1);
    assert_eq!(after.market().listed_units(seller, Good::Bread), 4);
    assert_eq!(citizen(&after, seller).coins(), 19);
    assert_eq!(after.town_treasury(), 4);
    let trade = &after.market().trades()[0];
    assert_eq!(
        (trade.place, trade.buyer, trade.coins, trade.units),
        (market, MarketParty::Caravan, 23, 5)
    );
    assert_eq!(
        after.citizen_history(seller).unwrap().current.start_ms,
        UPDATE_TIME_MS
    );
    assert_eq!(
        after.citizen_history(seller).unwrap().current.sold[Good::Bread as usize],
        5
    );
    assert_eq!(
        after
            .citizen_history(seller)
            .unwrap()
            .completed
            .back()
            .unwrap()
            .sold[Good::Bread as usize],
        0
    );
    assert_eq!(
        after.market().current_period().goods[Good::Bread as usize].exported_units,
        5
    );
    assert_eq!(after.advance(0).unwrap(), after);
    assert_eq!(citizen(&before, seller).coins(), 0);
    assert!(before.market().trades().is_empty());
    let long = before.advance(DAY_MS * 2 + 1).unwrap();
    let stepped = after.advance(DAY_MS).unwrap().advance(DAY_MS).unwrap();
    assert_eq!(long.market(), stepped.market());
    assert_eq!(
        citizen(&long, seller).coins(),
        citizen(&stepped, seller).coins()
    );
    assert_eq!(long.town_treasury(), stepped.town_treasury());
    assert!(
        long.market()
            .history()
            .iter()
            .all(|activity| activity.unmet_demand_units == 0)
    );
}

#[test]
fn imports_deliver_at_market_purchase_cash_leaves_town_and_forecast_is_immutable() {
    let price = Good::Bread.core_price() * 2.0;
    let (world, buyer) = Universe::with_map(Map::default())
        .with_prices(Prices::default().with_price(Good::Bread, price).unwrap())
        .with_citizen(
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(1000).unwrap(),
        )
        .unwrap();
    let requested = world
        .with_purchase_request(buyer, ShoppingList::single(Good::Bread, 9))
        .unwrap();
    let imported = requested.advance(UPDATE_TIME_MS).unwrap();
    assert_eq!(imported.market().caravan_units(Good::Bread), 5);
    assert!(imported.market().trades().is_empty());
    assert_eq!(imported.agents().len(), 1);
    assert_eq!(
        imported.storage().units(
            imported.map().public_place(Location::Market),
            learning_lord_simulation::storage::GoodsOwner::Town,
            Good::Bread
        ),
        0
    );
    let started = imported
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Bread, 2)),
        )
        .unwrap();
    let predicted = citizen(&started, buyer).advance(TRADE_DURATION_MS).unwrap();
    assert_eq!(predicted.units(Good::Bread), 2);
    assert_eq!(imported.market().caravan_units(Good::Bread), 5);
    let purchased = started.advance(TRADE_DURATION_MS).unwrap();
    assert_eq!(citizen(&purchased, buyer).units(Good::Bread), 2);
    assert_eq!(purchased.agents().len(), 1);
    assert_eq!(purchased.town_treasury(), 0);
    assert_eq!(
        citizen(&purchased, buyer).coins(),
        1000 - (Good::Bread.import_threshold() * 2.0).ceil() as i64
    );
    assert_eq!(purchased.market().trades()[0].seller, MarketParty::Caravan);
    let activity = purchased.market().current_period().goods[Good::Bread as usize];
    assert_eq!(
        (
            activity.imported_units,
            activity.caravan_purchased_units,
            activity.local_traded_units
        ),
        (5, 2, 0)
    );
    assert_eq!(
        purchased.citizen_history(buyer).unwrap().current.bought[Good::Bread as usize],
        2
    );
    assert_eq!(requested.market().caravan_units(Good::Bread), 0);
}

#[test]
fn empty_world_daily_progression_matches_smaller_advances() {
    let world = Universe::with_map(Map::default());
    let direct = world.advance(UPDATE_TIME_MS + 3 * DAY_MS).unwrap();
    let stepped = world
        .advance(UPDATE_TIME_MS)
        .unwrap()
        .advance(DAY_MS)
        .unwrap()
        .advance(DAY_MS)
        .unwrap()
        .advance(DAY_MS)
        .unwrap();
    assert_eq!(direct, stepped);
}

#[test]
fn failed_or_cancelled_daily_settlement_preserves_the_authoritative_snapshot() {
    let (world, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::default().with_price(Good::Bread, 5.0).unwrap())
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 1)
                .unwrap()
                .with_coins(i64::MAX)
                .unwrap(),
        )
        .unwrap();
    let listed = world
        .start_action(seller, CitizenAction::List(Good::Bread, 1))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let saved = listed.clone();
    assert_eq!(
        listed.advance(UPDATE_TIME_MS - TRADE_DURATION_MS),
        Err(learning_lord_simulation::SimulationError::WealthOverflow)
    );
    assert_eq!(listed, saved);
    let (world, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::default().with_price(Good::Bread, 5.0).unwrap())
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 1)
                .unwrap(),
        )
        .unwrap();
    let listed = world
        .start_action(seller, CitizenAction::List(Good::Bread, 1))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let saved = listed.clone();
    let mut checks = 0;
    let result = listed
        .advance_with_planner(
            &mut learning_lord_simulation::PlanningRuntime::default(),
            UPDATE_TIME_MS + DAY_MS,
            || {
                checks += 1;
                checks >= 3
            },
        )
        .unwrap();
    assert!(result.is_none());
    assert_eq!(listed, saved);
    assert!(listed.market().trades().is_empty());
}
