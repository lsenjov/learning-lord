use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, Universe};

const HOUR_MS: u64 = 3_600_000;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn tiredness_starts_at_zero_or_explicit_finite_values_with_a_floor() {
    assert_eq!(Citizen::new(0.0).unwrap().tiredness(), 0.0);
    assert_eq!(
        Citizen::with_hunger_rate(0.0, 0.0).unwrap().tiredness(),
        0.0
    );
    for (starting, expected) in [
        (-200.0, -100.0),
        (-100.0, -100.0),
        (-50.0, -50.0),
        (150.0, 150.0),
    ] {
        assert_eq!(
            Citizen::with_needs(0.0, starting).unwrap().tiredness(),
            expected
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Citizen::with_needs(0.0, invalid),
            Err(SimulationError::InvalidTiredness)
        );
        assert_eq!(
            Citizen::with_needs(invalid, 0.0),
            Err(SimulationError::InvalidHunger)
        );
    }
}

#[test]
fn eight_hours_of_sleep_balance_a_day_of_tiredness_growth() {
    let original = Citizen::with_needs(0.0, -50.0).unwrap();
    assert_close(original.advance(24 * HOUR_MS).unwrap().tiredness(), 50.0);
    let awake = original.advance(16 * HOUR_MS).unwrap();
    let rested = awake
        .start_action(CitizenAction::Sleep)
        .unwrap()
        .advance(8 * HOUR_MS)
        .unwrap();
    assert_close(rested.tiredness(), -50.0);
    assert_close(rested.hunger(), 100.0);
    assert_eq!(rested.active_action(), None);
    assert_eq!(original.tiredness(), -50.0);
}

#[test]
fn sleep_recovers_continuously_while_hunger_and_tiredness_keep_growing() {
    let original = Citizen::with_needs(-50.0, 100.0).unwrap();
    let sleeping = original.start_action(CitizenAction::Sleep).unwrap();
    assert_eq!(
        sleeping.active_action().unwrap().remaining_ms(),
        8 * HOUR_MS
    );
    assert_eq!(sleeping.advance(0).unwrap(), sleeping);
    let halfway = sleeping.advance(4 * HOUR_MS).unwrap();
    assert_close(halfway.tiredness(), 100.0 + 100.0 / 6.0 - 50.0);
    assert_close(halfway.hunger(), -50.0 + 100.0 / 6.0);
    assert_eq!(halfway.active_action().unwrap().remaining_ms(), 4 * HOUR_MS);
    let almost_done = halfway.advance(4 * HOUR_MS - 1).unwrap();
    assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
    let finished = almost_done.advance(1).unwrap();
    assert_close(finished.tiredness(), 100.0 / 3.0);
    assert_close(finished.hunger(), -50.0 + 100.0 / 3.0);
    assert_eq!(finished.active_action(), None);
    assert_eq!(original.tiredness(), 100.0);
    assert_eq!(sleeping.tiredness(), 100.0);
}

#[test]
fn recovery_stops_at_the_floor_and_idle_time_after_sleep_grows_tiredness() {
    let sleeping = Citizen::with_needs(-50.0, -90.0)
        .unwrap()
        .start_action(CitizenAction::Sleep)
        .unwrap();
    assert_eq!(sleeping.advance(4 * HOUR_MS).unwrap().tiredness(), -100.0);
    let large = sleeping.advance(10 * HOUR_MS).unwrap();
    assert_close(large.tiredness(), -100.0 + 100.0 / 12.0);
    let mut small = sleeping.clone();
    for _ in 0..600 {
        small = small.advance(60_000).unwrap();
    }
    assert_close(small.tiredness(), large.tiredness());
    assert_close(small.hunger(), large.hunger());
    assert_eq!(small.active_action(), None);
    assert_eq!(large.active_action(), None);
    assert_eq!(sleeping.tiredness(), -90.0);
}

#[test]
fn tiredness_penalizes_wellbeing_without_a_bonus_for_negative_values() {
    for (hunger, tiredness, expected) in [
        (-50.0, -100.0, 0.0),
        (-50.0, 0.0, 0.0),
        (-50.0, 20.0, -20.0),
        (30.0, 20.0, -50.0),
        (110.0, 20.0, -170.0),
        (-110.0, 20.0, -30.0),
    ] {
        let citizen = Citizen::with_needs(hunger, tiredness).unwrap();
        assert_eq!(citizen.personal_wellbeing(), Ok(expected - 20.0));
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(learning_lord_simulation::marketplace::Prices::default())
                .with_citizen("Ada", citizen)
                .unwrap();
        assert_eq!(
            universe.agents()[&id].personal_wellbeing(),
            Ok(expected - 20.0)
        );
    }
    assert_eq!(
        Citizen::with_needs(0.0, f64::MAX)
            .unwrap()
            .personal_wellbeing(),
        Ok(-f64::MAX)
    );
    assert_eq!(
        Citizen::with_needs(-f64::MAX, f64::MAX)
            .unwrap()
            .personal_wellbeing(),
        Err(SimulationError::WellbeingOverflow)
    );
}
