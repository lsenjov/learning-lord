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

fn listed_world(units: u64) -> (Universe, AgentId) {
    let (world, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::default().with_price(Good::Bread, 4.0).unwrap())
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, units)
                .unwrap(),
        )
        .unwrap();
    let world = world
        .start_action(seller, CitizenAction::List(Good::Bread, units))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    (world, seller)
}

#[test]
fn export_controls_limit_volume_without_consuming_reserve_stock() {
    for (allowed, reserve, exported) in [
        (false, 0, 0),
        (true, 10, 0),
        (true, 11, 0),
        (true, 8, 2),
        (true, 0, 5),
    ] {
        let (world, seller) = listed_world(10);
        let mut policy = world.caravan_policy();
        policy.exports[Good::Bread as usize] =
            learning_lord_simulation::marketplace::ExportPolicy {
                allowed,
                minimum_reserve: reserve,
            };
        let configured = world.with_caravan_policy(policy).unwrap();
        let after = configured
            .advance(UPDATE_TIME_MS - TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(
            after.market().listed_units(seller, Good::Bread),
            10 - exported
        );
        assert_eq!(after.town_stock(Good::Bread), 10 - exported);
        assert_eq!(
            world.caravan_policy().exports[Good::Bread as usize].minimum_reserve,
            0
        );
    }
}

#[test]
fn full_export_tariff_disables_exports() {
    let (world, seller) = listed_world(10);
    let mut policy = world.caravan_policy();
    policy.export_tariff_basis_points = 10_000;
    let disabled = world.with_caravan_policy(policy).unwrap();
    assert_eq!(
        disabled.market().effective_export_threshold(Good::Bread),
        None
    );
    let after = disabled
        .advance(UPDATE_TIME_MS - TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(after.market().listed_units(seller, Good::Bread), 10);
    assert_eq!(after.town_treasury(), 0);
}

#[test]
fn town_reserve_includes_carried_storage_and_town_goods_once() {
    use learning_lord_simulation::storage::GoodsOwner;
    let (world, seller) = listed_world(10);
    let (world, holder) = world
        .with_citizen(
            "Holder",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 3)
                .unwrap(),
        )
        .unwrap();
    let market = world.map().public_place(Location::Market);
    let world = world
        .with_stored_good(market, GoodsOwner::Agent(holder), Good::Bread, 4)
        .unwrap()
        .with_stored_good(market, GoodsOwner::Town, Good::Bread, 5)
        .unwrap();
    assert_eq!(world.town_stock(Good::Bread), 22);
    let mut policy = world.caravan_policy();
    policy.exports[Good::Bread as usize].minimum_reserve = 20;
    let after = world
        .with_caravan_policy(policy)
        .unwrap()
        .advance(UPDATE_TIME_MS - TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(after.town_stock(Good::Bread), 20);
    assert_eq!(after.market().listed_units(seller, Good::Bread), 8);
    assert_eq!(citizen(&after, holder).units(Good::Bread), 3);
    assert_eq!(after.storage(), world.storage());
}

#[test]
fn export_tariff_grosses_up_net_receipts_and_income_tax_uses_only_receipts() {
    let (world, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::default().with_price(Good::Bread, 4.0).unwrap())
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 10)
                .unwrap(),
        )
        .unwrap();
    let (world, bakery) = world.with_property(seller, Location::Bakery).unwrap();
    let (world, _) = world
        .with_tax_rule(
            bakery,
            "Income",
            TaxKind::Income {
                rates: [(Good::Bread, TaxRate::new(2000).unwrap())]
                    .into_iter()
                    .collect(),
            },
        )
        .unwrap();
    let world = world
        .start_action(seller, CitizenAction::List(Good::Bread, 10))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let mut policy = world.caravan_policy();
    policy.export_tariff_basis_points = 5000;
    let configured = world.with_caravan_policy(policy).unwrap();
    let after = configured
        .advance(UPDATE_TIME_MS - TRADE_DURATION_MS)
        .unwrap();
    let trade = &after.market().trades()[0];
    assert_eq!(
        (trade.coins, trade.recipient_coins, trade.tariff_coins),
        (36, 18, 18)
    );
    assert_eq!(citizen(&after, seller).coins(), 15);
    assert_eq!(after.town_treasury(), 21);
    assert_eq!(
        trade.coins,
        citizen(&after, seller).coins() + after.town_treasury()
    );
    assert_eq!(
        after.citizen_history(seller).unwrap().current.coins_earned,
        18
    );
    let activity = after.market().current_period().goods[Good::Bread as usize];
    assert_eq!(
        (
            activity.exported_coins,
            activity.exported_receipts,
            activity.export_tariff_coins
        ),
        (36, 18, 18)
    );
    assert_eq!(
        configured
            .advance(UPDATE_TIME_MS - TRADE_DURATION_MS + DAY_MS)
            .unwrap(),
        after.advance(DAY_MS).unwrap()
    );
}

#[test]
fn existing_imports_reprice_and_disable_immediately_without_mutating_forecasts() {
    let (world, buyer) = Universe::with_map(Map::default())
        .with_prices(
            Prices::default()
                .with_price(Good::Bread, Good::Bread.core_price() * 4.0)
                .unwrap(),
        )
        .with_citizen(
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(1000).unwrap(),
        )
        .unwrap();
    let imported = world
        .with_purchase_request(buyer, ShoppingList::single(Good::Bread, 9))
        .unwrap()
        .advance(UPDATE_TIME_MS)
        .unwrap();
    let market = imported.map().public_place(Location::Market);
    let mut policy = imported.caravan_policy();
    policy.import_tariff_basis_points = 5000;
    let taxed = imported.with_caravan_policy(policy).unwrap();
    let gross = (Good::Bread.import_threshold() * 4.0).ceil() as i64;
    assert_eq!(
        taxed
            .market()
            .purchase_cost_at(buyer, market, Good::Bread, 2),
        Some(gross)
    );
    let started = taxed
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Bread, 2)),
        )
        .unwrap();
    let predicted = citizen(&started, buyer).advance(TRADE_DURATION_MS).unwrap();
    let purchased = started.advance(TRADE_DURATION_MS).unwrap();
    assert_eq!(predicted.coins(), citizen(&purchased, buyer).coins());
    let trade = &purchased.market().trades()[0];
    assert_eq!(trade.coins, gross);
    assert_eq!(trade.tariff_coins, gross / 2);
    assert_eq!(purchased.town_treasury(), gross / 2);
    assert_eq!(trade.recipient_coins + trade.tariff_coins, gross);
    assert!(purchased.tax_history().is_empty());
    assert_eq!(taxed.town_treasury(), 0);
    assert_eq!(imported.market().caravan_units(Good::Bread), 5);
    policy.import_tariff_basis_points = 10_000;
    let disabled = taxed.with_caravan_policy(policy).unwrap();
    assert_eq!(disabled.market().available_units(buyer, Good::Bread), 0);
    assert_eq!(
        disabled
            .market()
            .purchase_cost_at(buyer, market, Good::Bread, 2),
        Some(0)
    );
    assert_eq!(
        disabled.market().effective_import_threshold(Good::Bread),
        None
    );
    assert_eq!(
        disabled.market().current_period().goods[Good::Bread as usize].listed_units,
        0
    );
    assert_eq!(disabled.market().caravan_units(Good::Bread), 5);
    assert!(
        disabled
            .market()
            .orders()
            .all(|order| order.quoted_price.is_finite())
    );
    let attempted = disabled
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Bread, 2)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&attempted, buyer).units(Good::Bread), 0);
    assert_eq!(attempted.town_treasury(), 0);
    policy.import_tariff_basis_points = 0;
    let restored = disabled.with_caravan_policy(policy).unwrap();
    assert_eq!(restored.market().available_units(buyer, Good::Bread), 5);
    assert_eq!(
        restored
            .market()
            .purchase_cost_at(buyer, market, Good::Bread, 2),
        Some((Good::Bread.import_threshold() * 2.0).ceil() as i64)
    );
    policy.import_tariff_basis_points = 10_001;
    assert_eq!(
        restored.with_caravan_policy(policy),
        Err(learning_lord_simulation::SimulationError::InvalidTaxRate)
    );
}
