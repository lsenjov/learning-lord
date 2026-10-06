use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::{
    DAY_MS, Good, MIN_QUOTED_PRICE, Prices, ShoppingList, UPDATE_TIME_MS,
};
use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe,
};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn citizen(u: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(c) = &u.agents()[&id].kind;
    c
}
fn buyer(coins: i64) -> (Universe, AgentId) {
    Universe::with_map(Map::default())
        .with_prices(Prices::new(100.0).unwrap())
        .with_citizen(
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(coins).unwrap(),
        )
        .unwrap()
}
fn supply(grams: u64) -> (Universe, AgentId) {
    let (u, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::new(100.0).unwrap())
        .with_citizen(
            "Seller",
            Citizen::new(0.0).unwrap().with_berries(grams).unwrap(),
        )
        .unwrap();
    (
        u.start_action(seller, CitizenAction::List(Good::Berries, grams))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap(),
        seller,
    )
}

#[test]
fn affordable_shortages_raise_prices_at_four_and_repeated_requests_replace_intent() {
    let (source, id) = buyer(100);
    let list = ShoppingList::single(Good::Berries, 100);
    let mut u = source.with_purchase_request(id, list).unwrap();
    for _ in 0..20 {
        u = u.with_purchase_request(id, list).unwrap();
    }
    assert_eq!(u.market().requested(id).units(Good::Berries), 100);
    assert_eq!(u.market().affordable_request(id).units(Good::Berries), 100);
    let before = u.advance(UPDATE_TIME_MS - 1).unwrap();
    assert_eq!(before.prices(), Prices::new(100.0).unwrap());
    assert!(before.market().history().is_empty());
    let after = before.advance(1).unwrap();
    close(
        after.prices().price(Good::Berries).unwrap(),
        110.00000000000001,
    );
    let day = &after.market().history()[0];
    assert_eq!(day.start_ms, 0);
    assert_eq!(day.end_ms, UPDATE_TIME_MS);
    assert_eq!(day.unmet_demand_units, 100);
    assert_eq!(day.remaining_supply_units, 0);
    assert_eq!(day.traded_units, 0);
    let next = after.advance(DAY_MS).unwrap();
    close(next.prices().price(Good::Berries).unwrap(), 121.0);
    assert_eq!(next.market().history()[1].start_ms, UPDATE_TIME_MS);
    assert_eq!(next.market().history()[1].end_ms, UPDATE_TIME_MS + DAY_MS);
    assert!(source.market().history().is_empty());
    assert!(source.market().requested(id).items().next().is_none());
}

#[test]
fn unfunded_requests_do_not_drive_prices_and_cancellation_removes_demand() {
    let (source, id) = buyer(0);
    let requested = source
        .with_purchase_request(id, ShoppingList::single(Good::Berries, 100))
        .unwrap();
    assert_eq!(requested.market().requested(id).units(Good::Berries), 100);
    assert_eq!(
        requested
            .market()
            .affordable_request(id)
            .units(Good::Berries),
        0
    );
    let after = requested.advance(UPDATE_TIME_MS + DAY_MS).unwrap();
    assert_eq!(after.prices(), source.prices());
    assert!(after.market().history().is_empty());
    let (funded, id) = buyer(100);
    let requested = funded
        .with_purchase_request(id, ShoppingList::single(Good::Berries, 100))
        .unwrap();
    let cleared = requested
        .with_purchase_request(id, ShoppingList::default())
        .unwrap()
        .advance(UPDATE_TIME_MS)
        .unwrap();
    assert!(cleared.market().requested(id).items().next().is_none());
    assert!(cleared.market().history().is_empty());
}

#[test]
fn unsold_supply_lowers_prices_gradually_and_respects_the_floor() {
    let (u, seller) = supply(100);
    let after = u.advance(UPDATE_TIME_MS - u.current_time_ms()).unwrap();
    close(after.prices().price(Good::Berries).unwrap(), 90.0);
    assert_eq!(after.market().history()[0].remaining_supply_units, 100);
    assert_eq!(after.market().orders().next().unwrap().quoted_price, 90.0);
    assert_eq!(citizen(&after, seller).units(Good::Berries), 0);
    let floor = u
        .with_prices(Prices::new(MIN_QUOTED_PRICE).unwrap())
        .advance(UPDATE_TIME_MS - u.current_time_ms())
        .unwrap();
    assert_eq!(
        floor.prices().price(Good::Berries).unwrap(),
        MIN_QUOTED_PRICE
    );
    assert!(Prices::new(MIN_QUOTED_PRICE / 2.0).is_err());
}

#[test]
fn actual_fills_reduce_requests_and_balanced_trades_record_volume_without_price_changes() {
    let (u, seller) = supply(100);
    let (u, id) = u
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let started = u
        .start_action(
            id,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 100)),
        )
        .unwrap();
    assert_eq!(started.market().requested(id).units(Good::Berries), 100);
    let bought = started.advance(TRADE_DURATION_MS).unwrap();
    assert_eq!(bought.market().requested(id).units(Good::Berries), 0);
    assert_eq!(citizen(&bought, seller).coins(), 10);
    let after = bought
        .advance(UPDATE_TIME_MS - bought.current_time_ms())
        .unwrap();
    let day = &after.market().history()[0];
    assert_eq!(day.traded_units, 100);
    assert_eq!(day.traded_coins, 10);
    assert_eq!(day.unmet_demand_units, 0);
    assert_eq!(day.remaining_supply_units, 0);
    assert_eq!(day.price_before, day.price_after);
    let next = after.advance(DAY_MS).unwrap();
    assert_eq!(next.market().history().len(), 1);
    assert_eq!(next.prices(), after.prices());
}

#[test]
fn one_coin_budget_is_shared_across_goods_and_available_asks_override_reference_prices() {
    let (source, id) = buyer(12);
    let request =
        ShoppingList::new([(Good::Flour, 100), (Good::Wood, 100), (Good::Water, 100)]).unwrap();
    let u = source.with_purchase_request(id, request).unwrap();
    assert_eq!(u.market().affordable_request(id).units(Good::Flour), 100);
    assert_eq!(u.market().affordable_request(id).units(Good::Wood), 40);
    assert_eq!(u.market().affordable_request(id).units(Good::Water), 0);
    let (u, _) = supply(100);
    let (u, id) = u
        .with_prices(Prices::new(200.0).unwrap())
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(10).unwrap())
        .unwrap();
    let requested = u
        .with_purchase_request(id, ShoppingList::single(Good::Berries, 100))
        .unwrap();
    assert_eq!(
        requested
            .market()
            .affordable_request(id)
            .units(Good::Berries),
        100
    );
}

#[test]
fn seller_income_funds_an_existing_request_without_accumulating_retries() {
    let (u, seller) = supply(100);
    let u = u
        .with_purchase_request(seller, ShoppingList::single(Good::Flour, 100))
        .unwrap();
    assert_eq!(u.market().affordable_request(seller).units(Good::Flour), 0);
    let (u, id) = u
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let after = u
        .start_action(
            id,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 100)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(
        after.market().affordable_request(seller).units(Good::Flour),
        100
    );
    assert_eq!(after.market().requested(seller).units(Good::Flour), 100);
}

#[test]
fn boundary_completions_are_in_the_closing_interval_and_tick_partitioning_preserves_history() {
    let (u, seller) = supply(150);
    let original_order = u.market().orders().next().unwrap().clone();
    let (u, id) = u
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let u = u
        .advance(UPDATE_TIME_MS - u.current_time_ms() - TRADE_DURATION_MS)
        .unwrap()
        .start_action(
            id,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 100)),
        )
        .unwrap();
    let direct = u.advance(TRADE_DURATION_MS).unwrap();
    let split = u
        .advance(TRADE_DURATION_MS - 1)
        .unwrap()
        .advance(1)
        .unwrap();
    assert_eq!(direct.market(), split.market());
    let day = &direct.market().history()[0];
    assert_eq!(day.traded_units, 100);
    assert_eq!(day.remaining_supply_units, 50);
    assert_eq!(direct.market().trades()[0].time_ms, UPDATE_TIME_MS);
    close(day.price_after, 98.0);
    assert_eq!(direct.market().trades()[0].coins, 10);
    let remaining = direct.market().orders().next().unwrap();
    assert_eq!(remaining.id, original_order.id);
    assert_eq!(remaining.seller, seller);
    assert_eq!(remaining.good, original_order.good);
    assert_eq!(remaining.units, 50);
    close(remaining.quoted_price, 98.0);
    let after = direct
        .start_action(
            id,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 50)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(after.market().trades()[1].coins, 5);
    assert_eq!(after.market().listed_units(seller, Good::Berries), 0);
}

#[test]
fn prediction_does_not_register_demands_or_publish_history() {
    let (u, _) = supply(100);
    let (u, id) = u
        .with_citizen(
            "Hungry",
            Citizen::with_needs(80.0, -100.0)
                .unwrap()
                .with_coins(100)
                .unwrap(),
        )
        .unwrap();
    let before = u.clone();
    let plan = learning_lord_simulation::planning::plan(citizen(&u, id)).unwrap();
    assert!(!plan.actions().is_empty());
    let isolated = citizen(&u, id)
        .start_action(CitizenAction::BuyFood(Good::Berries))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert!(isolated.market().requested(id).items().next().is_none());
    assert!(isolated.market().trades().is_empty());
    assert!(isolated.market().history().is_empty());
    assert_eq!(u, before);
}

#[test]
fn purchases_crossing_four_settle_at_updated_live_order_prices() {
    let (source, seller) = supply(150);
    let (source, buyer) = source
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let source = source
        .advance(UPDATE_TIME_MS - source.current_time_ms() - TRADE_DURATION_MS / 2)
        .unwrap()
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 100)),
        )
        .unwrap();
    let boundary = source.advance(TRADE_DURATION_MS / 2).unwrap();
    close(
        boundary.market().orders().next().unwrap().quoted_price,
        98.0,
    );
    assert!(boundary.market().trades().is_empty());
    assert_eq!(boundary.market().listed_units(seller, Good::Berries), 150);
    let completed = boundary.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(completed.market().trades()[0].units, 100);
    assert_eq!(completed.market().trades()[0].coins, 10);
    assert_eq!(citizen(&completed, buyer).coins(), 90);
    assert_eq!(citizen(&completed, seller).coins(), 10);
    assert_eq!(completed, source.advance(TRADE_DURATION_MS).unwrap());
}
