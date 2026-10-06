use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::{DAY_MS, Good, Prices, UPDATE_TIME_MS};
use learning_lord_simulation::{AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe};

fn citizen(universe: &Universe, id: learning_lord_simulation::AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
    citizen
}

#[test]
fn inactive_prices_validate_and_remain_stable_across_daily_boundaries() {
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(Prices::new(invalid).is_err());
    }
    let source = Universe::with_map(Map::default());
    assert_eq!(source.prices(), Prices::default());
    let mut current = source.clone();
    for day in 0..100 {
        let duration = if day == 0 { UPDATE_TIME_MS } else { DAY_MS };
        current = current.advance(duration).unwrap();
        assert_eq!(current.prices(), source.prices());
    }
    assert_eq!(
        source.advance(UPDATE_TIME_MS + 99 * DAY_MS).unwrap(),
        current
    );
    assert_eq!(source.current_time_ms(), 0);
}

#[test]
fn explicit_setup_prices_share_citizen_context_without_running_the_daily_update() {
    let (universe, seller) = Universe::with_map(Map::default())
        .with_citizen(
            "Seller",
            Citizen::new(0.0).unwrap().with_berries(100.0).unwrap(),
        )
        .unwrap();
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let listed = universe
        .start_action(seller, CitizenAction::List(Good::Berries, 100.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let repriced = listed.with_prices(Prices::new(2.0).unwrap());
    for agent in repriced.agents().values() {
        let AgentKind::Citizen(c) = &agent.kind;
        assert_eq!(c.prices(), repriced.prices());
    }
    assert_eq!(repriced.market().orders().next().unwrap().coins_per_kg, 1.0);
    let bought = repriced
        .start_action(
            buyer,
            CitizenAction::BuyFood(learning_lord_simulation::marketplace::Good::Berries),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&bought, buyer).berries_grams(), 100.0);
    assert_eq!(citizen(&bought, buyer).coins(), 0.9);
    assert_eq!(citizen(&bought, seller).coins(), 0.1);
    assert_eq!(listed.prices(), Prices::default());
}

#[test]
fn prices_revalue_inventory_and_listed_goods_without_forcing_replanning() {
    let source = Citizen::with_needs(-50.0, -100.0)
        .unwrap()
        .with_berries(200.0)
        .unwrap();
    let (universe, id) = Universe::with_map(Map::default())
        .with_citizen("Ada", source)
        .unwrap();
    let listed = universe
        .start_action(id, CitizenAction::List(Good::Berries, 100.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap()
        .start_planning(id)
        .unwrap();
    let updated = listed.with_prices(Prices::new(2.0).unwrap());
    assert_eq!(
        citizen(&listed, id).active_plan(),
        citizen(&updated, id).active_plan()
    );
    assert_eq!(
        citizen(&listed, id).grams(Good::Berries),
        citizen(&updated, id).grams(Good::Berries)
    );
    assert_eq!(citizen(&updated, id).wealth(), Ok(0.4));
    assert_eq!(updated.advance(0).unwrap(), updated);
}

#[test]
fn predictions_hold_prices_fixed_beyond_the_daily_update() {
    let source = Citizen::with_needs(-50.0, 50.0)
        .unwrap()
        .with_berries(200.0)
        .unwrap();
    let (universe, id) = Universe::with_map(Map::default())
        .with_citizen("Ada", source)
        .unwrap();
    let source = citizen(&universe, id);
    let chosen = learning_lord_simulation::planning::plan(source).unwrap();
    assert_eq!(chosen.actions(), &[CitizenAction::Sleep]);
    let predicted = source
        .start_action(CitizenAction::Sleep)
        .unwrap()
        .advance(8 * 3_600_000)
        .unwrap();
    let expected =
        (source.personal_wellbeing().unwrap() + predicted.personal_wellbeing().unwrap()) * 0.5;
    assert!((chosen.average_wellbeing() - expected).abs() < 1e-10);
    assert_eq!(predicted.prices(), universe.prices());
    let actual = universe
        .start_planning(id)
        .unwrap()
        .advance(8 * 3_600_000)
        .unwrap();
    assert_eq!(actual.prices(), universe.prices());
}
