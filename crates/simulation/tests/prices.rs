use learning_lord_simulation::marketplace::{DAY_MS, Good, Prices, UPDATE_TIME_MS};
use learning_lord_simulation::{
    ACTION_DURATION_MS, AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe,
};

fn citizen(universe: &Universe) -> &Citizen {
    let AgentKind::Citizen(citizen) = &universe.agents().values().next().unwrap().kind;
    citizen
}

#[test]
fn prices_are_randomized_at_creation_and_at_four_each_day() {
    let source = Universe::with_map(learning_lord_simulation::locations::Map::default());
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(Prices::new(invalid, 1.0).is_err());
        assert!(Prices::new(1.0, invalid).is_err());
    }
    let mut current = source.clone();
    for day in 0..100 {
        let prices = current.prices();
        assert!((1.0..=2.0).contains(&prices.coins_per_kg(Good::Berries)));
        assert!((1.0..=4.0).contains(&prices.coins_per_kg(Good::Pebbles)));
        let duration = if day == 0 { UPDATE_TIME_MS } else { DAY_MS };
        let before = current.advance(duration - 1).unwrap();
        assert_eq!(before.prices(), prices);
        current = before.advance(1).unwrap();
        assert_ne!(current.prices(), prices);
    }
    assert_eq!(
        source.advance(UPDATE_TIME_MS + 99 * DAY_MS).unwrap(),
        current
    );
    assert_ne!(
        Universe::with_map(learning_lord_simulation::locations::Map::default()).prices(),
        source.prices()
    );
    assert_eq!(source.current_time_ms(), 0);
}

#[test]
fn all_citizens_share_prices_and_existing_trades_keep_their_quotes() {
    let initial = Prices::new(1.0, 2.0).unwrap();
    let universe = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_prices(initial)
        .advance(UPDATE_TIME_MS - 60_000)
        .unwrap();
    let (universe, buyer) =
        universe.with_citizen("buyer", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap());
    let (universe, seller) = universe.with_citizen(
        "seller",
        Citizen::new(0.0).unwrap().with_pebbles(100.0).unwrap(),
    );
    let started = universe
        .start_action(buyer, CitizenAction::BuyBerries)
        .unwrap()
        .start_action(seller, CitizenAction::SellPebbles)
        .unwrap();
    let boundary = started.advance(60_000).unwrap();
    for agent in boundary.agents().values() {
        let AgentKind::Citizen(citizen) = &agent.kind;
        assert_eq!(citizen.prices(), boundary.prices());
        assert_eq!(
            citizen.active_action().unwrap().remaining_ms(),
            TRADE_DURATION_MS - 60_000
        );
    }
    let complete = boundary.advance(TRADE_DURATION_MS - 60_000).unwrap();
    let AgentKind::Citizen(bought) = &complete.agents()[&buyer].kind;
    let AgentKind::Citizen(sold) = &complete.agents()[&seller].kind;
    assert_eq!(bought.berries_grams(), 100.0);
    assert_eq!(bought.coins(), 0.9);
    assert_eq!(sold.coins(), 0.2);
    assert_eq!(sold.pebbles_grams(), 0.0);
    assert_eq!(complete, started.advance(TRADE_DURATION_MS).unwrap());
    assert_eq!(started.prices(), initial);
}

#[test]
fn prices_revalue_stock_without_changing_inventory_or_forcing_replanning() {
    let source = Citizen::with_needs(-50.0, -100.0)
        .unwrap()
        .with_berries(50.0)
        .unwrap()
        .with_pebbles(50.0)
        .unwrap();
    let (universe, id) = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_citizen("Ada", source);
    let universe = universe
        .advance(UPDATE_TIME_MS - 1)
        .unwrap()
        .start_planning(id)
        .unwrap();
    let planned = citizen(&universe).active_plan().unwrap().plan().clone();
    let after = universe.advance(1).unwrap();
    let updated = citizen(&after);
    assert_eq!(updated.active_plan().unwrap().plan(), &planned);
    assert_eq!(updated.active_plan().unwrap().elapsed_ms(), 1);
    let expected = updated.coins()
        + after.prices().value(Good::Berries, updated.berries_grams())
        + after.prices().value(Good::Pebbles, updated.pebbles_grams());
    assert_eq!(updated.wealth(), Ok(expected));
    assert_eq!(universe.advance(0).unwrap(), universe);
}

#[test]
fn pebble_yield_is_half_the_berry_yield_from_the_same_random_state() {
    let source = Citizen::new(0.0).unwrap();
    let foraged = source
        .start_action(CitizenAction::Forage)
        .unwrap()
        .advance(ACTION_DURATION_MS)
        .unwrap();
    let rocks = source
        .start_action(CitizenAction::FindRocks)
        .unwrap()
        .advance(ACTION_DURATION_MS)
        .unwrap();
    assert_eq!(rocks.pebbles_grams(), foraged.berries_grams() / 2.0);
    assert!((2.5..=7.5).contains(&rocks.pebbles_grams()));
}

#[test]
fn predictions_hold_prices_fixed_beyond_the_daily_update() {
    let source = Citizen::with_needs(-50.0, 50.0)
        .unwrap()
        .with_pebbles(100.0)
        .unwrap();
    let (universe, id) = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_citizen("Ada", source);
    let source = citizen(&universe);
    let chosen = learning_lord_simulation::planning::plan(source).unwrap();
    assert_eq!(chosen.actions(), &[CitizenAction::Sleep]);
    let predicted = source
        .start_action(CitizenAction::Sleep)
        .unwrap()
        .advance(8 * 3_600_000)
        .unwrap();
    let started = source.start_action(CitizenAction::Sleep).unwrap();
    let expected = (1..=480)
        .map(|minute| {
            started
                .advance(minute * 60_000)
                .unwrap()
                .personal_wellbeing()
                .unwrap()
        })
        .sum::<f64>()
        / 480.0;
    assert!((chosen.average_wellbeing() - expected).abs() < 1e-10);
    assert_eq!(predicted.prices(), universe.prices());
    let actual = universe
        .start_planning(id)
        .unwrap()
        .advance(8 * 3_600_000)
        .unwrap();
    assert_eq!(
        actual.prices(),
        universe.advance(8 * 3_600_000).unwrap().prices()
    );
    assert_ne!(actual.prices(), universe.prices());
}
