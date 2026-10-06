use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, planning::plan};

const HALF_HOUR_MS: u64 = 1_800_000;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn inventory_is_integer_and_busy_replacement_is_rejected() {
    let original = Citizen::new(0.0).unwrap();
    assert_eq!(original.berries_units(), 0);
    let stocked = original.with_berries(310).unwrap();
    assert_eq!(stocked.berries_units(), 310);
    assert_eq!(original.berries_units(), 0);
    let busy = stocked.start_action(CitizenAction::Eat).unwrap();
    assert_eq!(busy.with_berries(1), Err(SimulationError::CitizenBusy));
}

#[test]
fn meals_deduct_integer_portions_at_start_and_restore_nutrition_progressively() {
    for stock in [155, 310, 500] {
        let source = Citizen::new(20.0).unwrap().with_berries(stock).unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let duration_ms = 155_000;
        assert_eq!(started.active_action().unwrap().duration_ms(), duration_ms);
        assert_eq!(started.berries_units(), stock - 155);
        let halfway = started.advance(duration_ms / 2).unwrap();
        assert_eq!(halfway.berries_units(), stock - 155);
        assert_close(
            halfway.hunger(),
            20.0 + 100.0 / 24.0 * duration_ms as f64 / 7_200_000.0 - 25.0,
        );
        let completed = halfway.advance(duration_ms / 2).unwrap();
        assert_eq!(completed.active_action(), None);
        assert_eq!(completed.berries_units(), stock - 155);
        assert_close(
            completed.hunger(),
            20.0 + 100.0 / 24.0 * duration_ms as f64 / 3_600_000.0 - 50.0,
        );
        assert_eq!(source.berries_units(), stock);
    }
}

#[test]
fn empty_meals_skip_and_partial_berries_use_whole_available_units() {
    let empty = Citizen::new(0.0).unwrap();
    assert_eq!(empty.start_action(CitizenAction::Eat).unwrap(), empty);
    for units in [1, 154] {
        let citizen = empty.with_berries(units).unwrap();
        let eating = citizen.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(eating.berries_units(), 0);
        assert_eq!(eating.active_action().unwrap().duration_ms(), units * 1000);
        let eaten = eating.advance(units * 1000).unwrap();
        assert_close(
            eaten.hunger(),
            citizen.hunger_per_hour() * units as f64 / 3600.0
                - units as f64 * learning_lord_simulation::BERRY_NUTRITION_PER_GRAM,
        );
    }
}

#[test]
fn forage_yields_only_at_completion_and_prediction_preserves_actual_outcomes() {
    let original = Citizen::new(20.0).unwrap();
    let snapshot = original.clone();
    let predicted = plan(&original).unwrap();
    assert_eq!(predicted, plan(&original).unwrap());
    assert_eq!(original, snapshot);
    let foraging = original
        .start_action(CitizenAction::Produce(
            learning_lord_simulation::production::Recipe::Forage,
        ))
        .unwrap();
    let almost_done = foraging.advance(HALF_HOUR_MS - 1).unwrap();
    assert_eq!(almost_done.berries_units(), 0);
    let completed = almost_done.advance(1).unwrap();
    assert!((5..=15).contains(&completed.berries_units()));
    assert_close(completed.hunger(), 20.0 + 100.0 / 48.0);
    assert_close(completed.tiredness(), 100.0 / 48.0);
    let direct = snapshot
        .start_action(CitizenAction::Produce(
            learning_lord_simulation::production::Recipe::Forage,
        ))
        .unwrap()
        .advance(HALF_HOUR_MS)
        .unwrap();
    assert_eq!(completed.berries_units(), direct.berries_units());
    assert_eq!(foraging.berries_units(), 0);
    assert_eq!(completed.active_action(), None);
}

#[test]
fn empty_hungry_citizens_can_plan_and_advance_without_zero_duration_loops() {
    let source = Citizen::with_needs(50.0, -50.0).unwrap();
    let planned = source.start_planning().unwrap();
    assert!(matches!(
        planned.active_action().unwrap().action(),
        CitizenAction::Produce(_)
    ));
    let advanced = planned.advance(24 * 3_600_000).unwrap();
    assert!(advanced.hunger().is_finite());
    assert!(advanced.tiredness().is_finite());
    assert!(advanced.active_action().unwrap().remaining_ms() > 0);
    assert_eq!(source.berries_units(), 0);
}
