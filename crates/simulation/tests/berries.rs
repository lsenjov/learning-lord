use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, planning::plan};

const HALF_HOUR_MS: u64 = 1_800_000;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn inventory_starts_empty_and_rejects_invalid_quantities_or_busy_replacement() {
    let original = Citizen::new(0.0).unwrap();
    assert_eq!(original.berries_grams(), 0.0);
    for grams in [-1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(
            original.with_berries(grams),
            Err(SimulationError::InvalidBerries)
        );
    }
    let stocked = original.with_berries(12.345).unwrap();
    assert_eq!(stocked.berries_grams(), 12.345);
    assert_eq!(original.berries_grams(), 0.0);
    let busy = stocked.start_action(CitizenAction::Eat).unwrap();
    assert_eq!(busy.with_berries(1.0), Err(SimulationError::CitizenBusy));
}

#[test]
fn meals_use_available_grams_up_to_fifty_nutrition_and_consume_continuously() {
    for (stock, portion) in [(10.0, 10.0), (63.25, 63.25), (150.5, 100.0)] {
        let source = Citizen::new(20.0).unwrap().with_berries(stock).unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let duration_ms = (portion * 1000.0) as u64;
        assert_eq!(started.active_action().unwrap().duration_ms(), duration_ms);
        assert_eq!(started.berries_grams(), stock);
        let halfway = started.advance(duration_ms / 2).unwrap();
        assert_close(halfway.berries_grams(), stock - portion / 2.0);
        assert_close(
            halfway.hunger(),
            20.0 + 100.0 / 24.0 * duration_ms as f64 / 7_200_000.0 - portion / 4.0,
        );
        let completed = halfway.advance(duration_ms / 2).unwrap();
        assert_eq!(completed.active_action(), None);
        assert_close(completed.berries_grams(), stock - portion);
        assert_close(
            completed.hunger(),
            20.0 + 100.0 / 24.0 * duration_ms as f64 / 3_600_000.0 - portion / 2.0,
        );
        assert_eq!(source.berries_grams(), stock);
    }
}

#[test]
fn tiny_meals_round_up_to_one_millisecond_and_empty_meals_are_skipped() {
    let empty = Citizen::new(0.0).unwrap();
    assert_eq!(empty.start_action(CitizenAction::Eat).unwrap(), empty);
    for grams in [0.0001, f64::MIN_POSITIVE, f64::from_bits(1)] {
        let started = empty
            .with_berries(grams)
            .unwrap()
            .start_action(CitizenAction::Eat)
            .unwrap();
        assert_eq!(started.active_action().unwrap().remaining_ms(), 1);
        let finished = started.advance(1).unwrap();
        assert_eq!(finished.berries_grams(), 0.0);
        assert_eq!(finished.active_action(), None);
        assert_close(finished.hunger(), 100.0 / 24.0 / 3_600_000.0 - grams * 0.5);
    }
}

#[test]
fn forage_yields_only_at_completion_and_prediction_preserves_actual_outcomes() {
    let original = Citizen::new(20.0).unwrap();
    let snapshot = original.clone();
    let predicted = plan(&original).unwrap();
    assert_eq!(predicted, plan(&original).unwrap());
    assert_eq!(original, snapshot);
    let foraging = original.start_action(CitizenAction::Forage).unwrap();
    let almost_done = foraging.advance(HALF_HOUR_MS - 1).unwrap();
    assert_eq!(almost_done.berries_grams(), 0.0);
    let completed = almost_done.advance(1).unwrap();
    assert!((5.0..=15.0).contains(&completed.berries_grams()));
    assert_close(completed.hunger(), 20.0 + 100.0 / 48.0);
    assert_close(completed.tiredness(), 100.0 / 48.0);
    let direct = snapshot
        .start_action(CitizenAction::Forage)
        .unwrap()
        .advance(HALF_HOUR_MS)
        .unwrap();
    assert_eq!(completed.berries_grams(), direct.berries_grams());
    assert_eq!(foraging.berries_grams(), 0.0);
    assert_eq!(completed.active_action(), None);
}

#[test]
fn empty_hungry_citizens_can_plan_and_advance_without_zero_duration_loops() {
    let source = Citizen::with_needs(50.0, -50.0).unwrap();
    let planned = source.start_planning().unwrap();
    assert_eq!(
        planned.active_action().unwrap().action(),
        CitizenAction::Forage
    );
    let advanced = planned.advance(24 * 3_600_000).unwrap();
    assert!(advanced.hunger().is_finite());
    assert!(advanced.tiredness().is_finite());
    assert!(advanced.berries_grams() >= 0.0);
    assert!(advanced.active_action().unwrap().remaining_ms() > 0);
    assert_eq!(source.berries_grams(), 0.0);
}
