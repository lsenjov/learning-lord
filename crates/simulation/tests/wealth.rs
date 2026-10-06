use learning_lord_simulation::marketplace::{Good, Prices};
use learning_lord_simulation::planning::plan;
use learning_lord_simulation::{ACTION_DURATION_MS, Citizen, CitizenAction, SimulationError};

#[test]
fn wealth_values_grams_and_coins_and_preserves_need_penalties() {
    let source = Citizen::with_needs(20.0, 10.0)
        .unwrap()
        .with_prices(Prices::new(100.0).unwrap());
    assert_eq!(source.coins(), 0);
    assert_eq!(Prices::default().price(Good::Berries).unwrap(), 5.0);
    let rich = source.with_berries(500).unwrap().with_coins(250).unwrap();
    assert_eq!(rich.wealth(), Ok(300.0));
    assert_eq!(
        rich.personal_wellbeing(),
        Ok(rich.food_reserve_wellbeing() - 20.0)
    );
    assert_eq!(source.wealth(), Ok(0.0));
    assert_eq!(rich.with_coins(-250).unwrap().wealth(), Ok(-200.0));
    let half = rich
        .start_action(CitizenAction::Eat)
        .unwrap()
        .advance(50_000)
        .unwrap();
    assert!((half.wealth().unwrap() - 295.0).abs() < 1e-12);
}

#[test]
fn integer_coin_boundaries_and_busy_setup_are_supported() {
    let source = Citizen::new(0.0).unwrap();
    for coins in [i64::MIN, 0, i64::MAX] {
        assert_eq!(source.with_coins(coins).unwrap().coins(), coins);
    }
    let busy = source.start_action(CitizenAction::Forage).unwrap();
    assert_eq!(busy.with_coins(100), Err(SimulationError::CitizenBusy));
    assert_eq!(busy.with_berries(1), Err(SimulationError::CitizenBusy));
    assert_eq!(
        source
            .with_prices(Prices::new(f64::MAX).unwrap())
            .with_berries(u64::MAX)
            .unwrap()
            .wealth(),
        Err(SimulationError::WealthOverflow)
    );
}

#[test]
fn berries_arrive_on_completion_and_prediction_preserves_randomness() {
    let source = Citizen::with_needs(-50.0, -100.0).unwrap();
    let original = source.clone();
    let planned = plan(&source).unwrap();
    assert!(
        planned
            .actions()
            .iter()
            .any(|a| matches!(a, CitizenAction::Forage))
    );
    assert!(planned.average_wellbeing() > -20.0);
    assert_eq!(source, original);
    let started = source.start_action(CitizenAction::Forage).unwrap();
    let partial = started.advance(ACTION_DURATION_MS - 1).unwrap();
    assert_eq!(partial.berries_units(), 0);
    let complete = partial.advance(1).unwrap();
    assert!((5..=15).contains(&complete.berries_units()));
    assert_eq!(complete.coins(), 0);
    assert_eq!(
        complete.berries_units(),
        original
            .start_action(CitizenAction::Forage)
            .unwrap()
            .advance(ACTION_DURATION_MS)
            .unwrap()
            .berries_units()
    );
}
