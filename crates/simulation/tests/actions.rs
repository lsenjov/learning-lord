use learning_lord_simulation::{Citizen, CitizenAction, SimulationError, Universe};

const MINUTE_MS: u64 = 60_000;
const HALF_HOUR_MS: u64 = 30 * MINUTE_MS;
const DAY_MS: u64 = 24 * 60 * MINUTE_MS;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn two_meals_balance_a_day_of_standard_hunger_including_time_spent_eating() {
    let original = Citizen::new(-25.0).unwrap().with_berries(310).unwrap();
    assert_close(original.advance(DAY_MS).unwrap().hunger(), 75.0);
    assert_eq!(original.active_action(), None);

    let mut citizen = original.clone();
    for _ in 0..2 {
        citizen = citizen.advance(DAY_MS / 2 - 155_000).unwrap();
        citizen = citizen.start_action(CitizenAction::Eat).unwrap();
        citizen = citizen.advance(155_000).unwrap();
    }

    assert_close(citizen.hunger(), -25.0);
    assert_eq!(citizen.berries_units(), 0);
    assert_eq!(citizen.active_action(), None);
    assert_eq!(original.hunger(), -25.0);
}

#[test]
fn meals_nourish_gradually_for_the_duration_of_the_selected_portion() {
    for (action, completed_hunger) in [(CitizenAction::Eat, -50.0), (CitizenAction::Wait, 0.0)] {
        let original = Citizen::with_hunger_rate(0.0, 0.0)
            .unwrap()
            .with_berries(155)
            .unwrap();
        let duration = if action == CitizenAction::Eat {
            155_000
        } else {
            HALF_HOUR_MS
        };
        let started = original.start_action(action).unwrap();
        let active = started.active_action().unwrap();
        assert_eq!(active.action(), action);
        assert_eq!(active.remaining_ms(), duration);
        assert_eq!(started.hunger(), 0.0);
        assert_eq!(original.active_action(), None);
        assert_eq!(started.advance(0).unwrap(), started);

        let midway = started.advance(duration / 2).unwrap();
        assert_close(midway.hunger(), completed_hunger / 2.0);
        let almost_done = midway.advance(duration / 2 - 1).unwrap();
        assert_close(
            almost_done.hunger(),
            completed_hunger * (duration - 1) as f64 / duration as f64,
        );
        assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
        let completed = almost_done.advance(1).unwrap();
        assert_close(completed.hunger(), completed_hunger);
        assert_eq!(completed.active_action(), None);
        assert_close(
            completed.advance(DAY_MS).unwrap().hunger(),
            completed_hunger,
        );
        assert!(completed.start_action(CitizenAction::Eat).is_ok());
        assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
    }
}

#[test]
fn busy_citizens_reject_all_actions_without_replacing_the_current_action() {
    for current in [
        CitizenAction::Eat,
        CitizenAction::Wait,
        CitizenAction::Sleep,
        CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
    ] {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_berries(155)
            .unwrap()
            .start_action(current)
            .unwrap()
            .advance(1_000)
            .unwrap();
        let snapshot = citizen.clone();
        for requested in [
            CitizenAction::Eat,
            CitizenAction::Wait,
            CitizenAction::Sleep,
            CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
        ] {
            assert_eq!(
                citizen.start_action(requested),
                Err(SimulationError::CitizenBusy)
            );
        }
        assert_eq!(citizen, snapshot);
    }
}

#[test]
fn hunger_advances_during_actions_and_after_their_completion() {
    for (action, expected_hunger) in [
        (CitizenAction::Eat, 100.0 / 24.0 - 50.0),
        (CitizenAction::Wait, 100.0 / 24.0),
    ] {
        let started = Citizen::new(0.0)
            .unwrap()
            .with_berries(155)
            .unwrap()
            .start_action(action)
            .unwrap();
        let midway = started.advance(50_000).unwrap();
        let nourishment = if action == CitizenAction::Eat {
            50.0 * 50_000.0 / 155_000.0
        } else {
            0.0
        };
        assert_close(midway.hunger(), 100.0 / 24.0 * 50.0 / 3600.0 - nourishment);
        assert_close(midway.tiredness(), 100.0 / 24.0 * 50.0 / 3600.0);
        assert_eq!(
            midway.active_action().unwrap().remaining_ms(),
            started.active_action().unwrap().remaining_ms() - 50_000
        );

        let completed = started.advance(60 * MINUTE_MS).unwrap();
        assert_close(completed.hunger(), expected_hunger);
        assert_eq!(completed.active_action(), None);
        assert_eq!(started.hunger(), 0.0);
    }
}

#[test]
fn small_ticks_and_one_large_tick_agree_across_action_completion() {
    for action in [
        CitizenAction::Eat,
        CitizenAction::Wait,
        CitizenAction::Sleep,
        CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
    ] {
        let original = Citizen::new(-80.0)
            .unwrap()
            .with_berries(150)
            .unwrap()
            .start_action(action)
            .unwrap();
        let mut small_ticks = original.clone();
        for _ in 0..240 {
            small_ticks = small_ticks.advance(17_000).unwrap();
        }
        let large_tick = original.advance(240 * 17_000).unwrap();

        assert_close(small_ticks.hunger(), large_tick.hunger());
        assert_close(small_ticks.tiredness(), large_tick.tiredness());
        assert_eq!(small_ticks.berries_units(), large_tick.berries_units());
        assert_eq!(small_ticks.active_action(), large_tick.active_action());
    }
}

#[test]
fn waiting_avoids_overfull_discomfort_while_eating_helps_a_hungry_citizen() {
    for (hunger, eating_is_better) in [(-80.0, false), (50.0, true)] {
        let citizen = Citizen::new(hunger).unwrap().with_berries(155).unwrap();
        let eat = citizen
            .start_action(CitizenAction::Eat)
            .unwrap()
            .advance(HALF_HOUR_MS)
            .unwrap();
        let wait = citizen
            .start_action(CitizenAction::Wait)
            .unwrap()
            .advance(HALF_HOUR_MS)
            .unwrap();

        assert_eq!(
            eat.personal_wellbeing().unwrap() > wait.personal_wellbeing().unwrap(),
            eating_is_better
        );
    }
}

#[test]
fn universe_action_branches_preserve_the_clock_source_and_other_agents() {
    let (original, id) = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_prices(learning_lord_simulation::marketplace::Prices::default())
        .with_citizen("Ada", Citizen::new(0.0).unwrap().with_berries(155).unwrap())
        .unwrap();
    let (original, other_id) = original
        .with_citizen("Bea", Citizen::new(10.0).unwrap())
        .unwrap();
    let original = original.advance(MINUTE_MS).unwrap();
    let snapshot = original.clone();
    let eating = original.start_action(id, CitizenAction::Eat).unwrap();
    let waiting = original.start_action(id, CitizenAction::Wait).unwrap();
    let eating_snapshot = eating.clone();

    assert_eq!(eating.current_time_ms(), MINUTE_MS);
    assert_eq!(waiting.current_time_ms(), MINUTE_MS);
    assert_eq!(eating.agents()[&other_id], original.agents()[&other_id]);
    assert_eq!(eating.agents()[&id].name, "Ada");
    assert_eq!(
        eating.start_action(id, CitizenAction::Wait),
        Err(SimulationError::CitizenBusy)
    );

    let (eaten, waited) = std::thread::scope(|scope| {
        let eat = scope.spawn(|| eating.advance(HALF_HOUR_MS).unwrap());
        let wait = scope.spawn(|| waiting.advance(HALF_HOUR_MS).unwrap());
        (eat.join().unwrap(), wait.join().unwrap())
    });

    assert_eq!(eaten.current_time_ms(), 31 * MINUTE_MS);
    assert_eq!(waited.current_time_ms(), eaten.current_time_ms());
    assert_eq!(eaten.agents()[&other_id], waited.agents()[&other_id]);
    assert_close(
        eaten.agents()[&id].personal_wellbeing().unwrap(),
        -20.0 - 100.0 / 24.0 * 31.0 / 60.0,
    );
    assert_close(
        waited.agents()[&id].personal_wellbeing().unwrap(),
        -20.0 + 5.0 + 155.0 / 1000.0 * 0.05 * 10.0 - 2.0 * 100.0 / 24.0 * 31.0 / 60.0,
    );
    assert_eq!(eating, eating_snapshot);
    assert_eq!(original, snapshot);

    let (_, missing_id) = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_prices(learning_lord_simulation::marketplace::Prices::default())
        .with_citizen("Missing", Citizen::new(0.0).unwrap())
        .unwrap();
    assert_eq!(
        original.start_action(missing_id, CitizenAction::Eat),
        Err(SimulationError::AgentNotFound)
    );
    assert_eq!(original, snapshot);
}

#[test]
fn failed_advances_preserve_action_progress_even_when_failure_is_after_completion() {
    for action in [
        CitizenAction::Eat,
        CitizenAction::Wait,
        CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
    ] {
        for hunger in [0.0, f64::MAX] {
            let citizen = Citizen::with_hunger_rate(hunger, f64::MAX)
                .unwrap()
                .with_berries(155)
                .unwrap()
                .start_action(action)
                .unwrap();
            let snapshot = citizen.clone();
            assert_eq!(
                citizen.advance(120 * MINUTE_MS),
                Err(SimulationError::HungerOverflow)
            );
            assert_eq!(citizen, snapshot);

            let (universe, _) =
                Universe::with_map(learning_lord_simulation::locations::Map::default())
                    .with_prices(learning_lord_simulation::marketplace::Prices::default())
                    .with_citizen("Ada", citizen)
                    .unwrap();
            let snapshot = universe.clone();
            assert_eq!(
                universe.advance(120 * MINUTE_MS),
                Err(SimulationError::HungerOverflow)
            );
            assert_eq!(universe, snapshot);
        }
    }
}
