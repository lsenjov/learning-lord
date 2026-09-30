use learning_lord_simulation::planning::plan;
use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, TRADE_DURATION_MS};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}

#[test]
fn purchases_top_up_at_completion_without_borrowing() {
    for (berries, coins, expected_berries, expected_coins) in [
        (0.0, 1.0, 100.0, 0.9),
        (40.0, 1.0, 100.0, 0.94),
        (40.0, 0.02, 60.0, 0.0),
        (0.0, 0.1, 100.0, 0.0),
    ] {
        let source = Citizen::new(10.0)
            .unwrap()
            .with_berries(berries)
            .unwrap()
            .with_coins(coins)
            .unwrap();
        let started = source.start_action(CitizenAction::BuyBerries).unwrap();
        assert_eq!(
            started.active_action().unwrap().duration_ms(),
            TRADE_DURATION_MS
        );
        let partial = started.advance(TRADE_DURATION_MS - 1).unwrap();
        assert_eq!(partial.berries_grams(), berries);
        assert_eq!(partial.coins(), coins);
        let complete = partial.advance(1).unwrap();
        close(complete.berries_grams(), expected_berries);
        close(complete.coins(), expected_coins);
        close(complete.wealth().unwrap(), source.wealth().unwrap());
        assert!(complete.hunger() > source.hunger());
        assert!(complete.tiredness() > source.tiredness());
        let direct = started.advance(TRADE_DURATION_MS).unwrap();
        assert_eq!(complete.coins(), direct.coins());
        assert_eq!(complete.berries_grams(), direct.berries_grams());
        assert_eq!(source.coins(), coins);
    }
}

#[test]
fn selling_transfers_all_pebbles_and_preserves_other_inventory() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_pebbles(125.5)
        .unwrap()
        .with_berries(12.0)
        .unwrap()
        .with_coins(-0.1)
        .unwrap();
    let started = source.start_action(CitizenAction::SellPebbles).unwrap();
    let partial = started.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(partial.pebbles_grams(), 125.5);
    assert_eq!(partial.coins(), -0.1);
    let complete = partial.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(complete.pebbles_grams(), 0.0);
    close(complete.coins(), 0.151);
    assert_eq!(complete.berries_grams(), 12.0);
    close(complete.wealth().unwrap(), source.wealth().unwrap());
    assert_eq!(source.pebbles_grams(), 125.5);
    assert_eq!(partial.with_pebbles(1.0), Err(SimulationError::CitizenBusy));
    let huge = source
        .with_coins(f64::MAX)
        .unwrap()
        .with_pebbles(f64::MAX)
        .unwrap()
        .start_action(CitizenAction::SellPebbles)
        .unwrap();
    assert_eq!(
        huge.advance(TRADE_DURATION_MS),
        Err(SimulationError::WealthOverflow)
    );
    assert_eq!(huge.coins(), f64::MAX);
}

#[test]
fn empty_trades_are_skipped() {
    for (berries, coins) in [(0.0, 0.0), (0.0, -1.0), (100.0, 1.0), (150.0, 1.0)] {
        let source = Citizen::new(0.0)
            .unwrap()
            .with_berries(berries)
            .unwrap()
            .with_coins(coins)
            .unwrap();
        for action in [CitizenAction::BuyBerries, CitizenAction::SellPebbles] {
            assert_eq!(source.action_duration_ms(action), 0);
            assert_eq!(source.start_action(action).unwrap(), source);
        }
    }
}

#[test]
fn a_hungry_citizen_can_trade_pebbles_for_a_meal() {
    let source = Citizen::with_needs(80.0, -100.0)
        .unwrap()
        .with_pebbles(50.0)
        .unwrap();
    let chosen = plan(&source).unwrap();
    assert_eq!(
        &chosen.actions()[..3],
        &[
            CitizenAction::SellPebbles,
            CitizenAction::BuyBerries,
            CitizenAction::Eat
        ]
    );
    let executing = source.start_planning().unwrap();
    let eating = executing.advance(2 * TRADE_DURATION_MS).unwrap();
    assert_eq!(eating.active_action().unwrap().action(), CitizenAction::Eat);
    close(eating.berries_grams(), 100.0);
    close(eating.coins(), 0.0);
    assert_eq!(eating.pebbles_grams(), 0.0);
    assert_eq!(source.berries_grams(), 0.0);
}
