use learning_lord_simulation::marketplace::{Good, Prices};
use learning_lord_simulation::planning::plan;
use learning_lord_simulation::{ACTION_DURATION_MS, Citizen, CitizenAction, SimulationError};

#[test]
fn wealth_values_grams_and_coins_and_preserves_need_penalties() {
    let source = Citizen::with_needs(20.0, 10.0).unwrap();
    assert_eq!(source.coins(), 0.0);
    assert_eq!(source.pebbles_grams(), 0.0);
    assert_eq!(Prices::default().coins_per_kg(Good::Berries), 1.0);
    assert_eq!(Prices::default().coins_per_kg(Good::Pebbles), 2.0);
    let rich = source
        .with_berries(500.0)
        .unwrap()
        .with_pebbles(250.0)
        .unwrap()
        .with_coins(2.5)
        .unwrap();
    assert_eq!(rich.wealth(), Ok(3.5));
    assert_eq!(rich.personal_wellbeing(), Ok(5.0));
    assert_eq!(source.wealth(), Ok(0.0));
    assert_eq!(rich.with_coins(-2.5).unwrap().wealth(), Ok(-1.5));
    let half = rich
        .start_action(CitizenAction::Eat)
        .unwrap()
        .advance(50_000)
        .unwrap();
    assert!((half.wealth().unwrap() - 3.45).abs() < 1e-12);
}

#[test]
fn wealth_setup_rejects_invalid_values_and_busy_citizens() {
    let source = Citizen::new(0.0).unwrap();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            source.with_coins(invalid),
            Err(SimulationError::InvalidCoins)
        );
        assert_eq!(
            source.with_pebbles(invalid),
            Err(SimulationError::InvalidPebbles)
        );
    }
    assert_eq!(
        source.with_pebbles(-1.0),
        Err(SimulationError::InvalidPebbles)
    );
    let busy = source.start_action(CitizenAction::FindRocks).unwrap();
    assert_eq!(busy.with_coins(1.0), Err(SimulationError::CitizenBusy));
    assert_eq!(busy.with_pebbles(1.0), Err(SimulationError::CitizenBusy));
    assert_eq!(
        source.with_coins(f64::MAX).unwrap().personal_wellbeing(),
        Err(SimulationError::WellbeingOverflow)
    );
    assert_eq!(
        source
            .with_coins(f64::MAX)
            .unwrap()
            .with_pebbles(f64::MAX)
            .unwrap()
            .wealth(),
        Err(SimulationError::WealthOverflow)
    );
}

#[test]
fn rocks_arrive_on_completion_and_prediction_preserves_randomness() {
    let source = Citizen::with_needs(-50.0, -100.0).unwrap();
    let original = source.clone();
    let planned = plan(&source).unwrap();
    assert!(
        planned
            .actions()
            .iter()
            .any(|a| matches!(a, CitizenAction::Forage | CitizenAction::FindRocks))
    );
    assert!(planned.average_wellbeing() >= 0.45 - 1e-10);
    assert_eq!(source, original);
    let started = source.start_action(CitizenAction::FindRocks).unwrap();
    let partial = started.advance(ACTION_DURATION_MS - 1).unwrap();
    assert_eq!(partial.pebbles_grams(), 0.0);
    let complete = partial.advance(1).unwrap();
    assert!((2.5..=7.5).contains(&complete.pebbles_grams()));
    assert_eq!(complete.berries_grams(), 0.0);
    assert_eq!(complete.coins(), 0.0);
    assert_eq!(
        complete.pebbles_grams(),
        original
            .start_action(CitizenAction::FindRocks)
            .unwrap()
            .advance(ACTION_DURATION_MS)
            .unwrap()
            .pebbles_grams()
    );
}
