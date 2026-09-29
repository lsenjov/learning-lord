use learning_lord_simulation::{AgentId, AgentKind, Citizen, SimulationError, Universe};
use std::thread;
use uuid::Version;

const HOUR_MS: u64 = 3_600_000;

fn citizen(universe: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
    citizen
}

#[test]
fn creation_assigns_v4_ids_and_preserves_existing_agents_and_time() {
    let empty = Universe::default();
    let (first, first_id) =
        empty.with_citizen("Ada", Citizen::with_hunger_rate(-10.0, 2.0).unwrap());
    let first = first.advance(500).unwrap();
    let (second, second_id) =
        first.with_citizen("Bea", Citizen::with_hunger_rate(0.0, 1.0).unwrap());

    assert_eq!(first_id.0.get_version(), Some(Version::Random));
    assert_eq!(second_id.0.get_version(), Some(Version::Random));
    assert_ne!(first_id, second_id);
    assert!(empty.agents().is_empty());
    assert_eq!(first.agents().len(), 1);
    assert_eq!(second.agents().len(), 2);
    assert_eq!(second.current_time_ms(), 500);
    assert_eq!(second.agents()[&first_id], first.agents()[&first_id]);
    assert_eq!(second.agents()[&first_id].name, "Ada");
    assert_eq!(second.agents()[&second_id].name, "Bea");
    assert_eq!(citizen(&second, second_id).hunger(), 0.0);
}

#[test]
fn starting_hunger_must_be_finite() {
    for hunger in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Citizen::with_hunger_rate(hunger, 1.0),
            Err(SimulationError::InvalidHunger)
        );
    }

    for hunger in [-f64::MAX, -10.0, 0.0, 150.0, f64::MAX] {
        assert_eq!(
            Citizen::with_hunger_rate(hunger, 1.0).unwrap().hunger(),
            hunger
        );
    }
}

#[test]
fn hunger_rate_must_be_finite_and_nonnegative() {
    for rate in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Citizen::with_hunger_rate(0.0, rate),
            Err(SimulationError::InvalidHungerRate)
        );
    }

    for rate in [0.0, 0.25, f64::MAX] {
        assert_eq!(
            Citizen::with_hunger_rate(0.0, rate)
                .unwrap()
                .hunger_per_hour(),
            rate
        );
    }
}

#[test]
fn personal_wellbeing_penalizes_hunger_and_weights_starvation_without_a_jump() {
    for (hunger, expected) in [
        (-f64::MAX, -f64::MAX),
        (-140.0, -40.0),
        (-100.25, -0.25),
        (-100.0, 0.0),
        (-99.75, 0.0),
        (-20.0, 0.0),
        (-0.25, 0.0),
        (0.0, 0.0),
        (0.25, -0.25),
        (20.0, -20.0),
        (99.75, -99.75),
        (100.0, -100.0),
        (100.25, -101.25),
        (120.0, -200.0),
    ] {
        let citizen = Citizen::with_hunger_rate(hunger, 2.0).unwrap();

        assert_eq!(
            citizen.personal_wellbeing(),
            Ok(expected),
            "hunger {hunger}"
        );
    }
}

#[test]
fn personal_wellbeing_reflects_each_snapshot_without_changing_it() {
    let (original, id) =
        Universe::default().with_citizen("Ada", Citizen::with_hunger_rate(-20.0, 120.0).unwrap());
    let snapshot = original.clone();
    let advanced = original.advance(HOUR_MS).unwrap();

    assert_eq!(original.agents()[&id].personal_wellbeing(), Ok(0.0));
    assert_eq!(advanced.agents()[&id].personal_wellbeing(), Ok(-100.0));
    assert_eq!(citizen(&advanced, id).personal_wellbeing(), Ok(-100.0));
    assert_eq!(original, snapshot);
    assert_eq!(citizen(&advanced, id).hunger(), 100.0);
}

#[test]
fn personal_wellbeing_rejects_overflow_without_changing_the_citizen() {
    for hunger in [f64::MAX / 4.0, f64::MAX] {
        let (universe, id) = Universe::default()
            .with_citizen("Ada", Citizen::with_hunger_rate(hunger, 0.0).unwrap());
        let snapshot = universe.clone();

        assert_eq!(
            citizen(&universe, id).personal_wellbeing(),
            Err(SimulationError::WellbeingOverflow)
        );
        assert_eq!(
            universe.agents()[&id].personal_wellbeing(),
            Err(SimulationError::WellbeingOverflow)
        );
        assert_eq!(universe, snapshot);
    }
    assert!(
        Citizen::with_hunger_rate(f64::MAX / 8.0, 0.0)
            .unwrap()
            .personal_wellbeing()
            .unwrap()
            .is_finite()
    );
}

#[test]
fn citizen_advances_its_needs_without_a_universe_and_preserves_the_source() {
    let original = Citizen::with_hunger_rate(-0.25, 3600.0).unwrap();

    let advanced = original.advance(500).unwrap();

    assert_eq!(advanced.hunger(), 0.25);
    assert_eq!(advanced.hunger_per_hour(), 3600.0);
    assert_eq!(original.hunger(), -0.25);
    assert_eq!(original.advance(0).unwrap(), original);
}

#[test]
fn citizen_rejects_hunger_overflow_without_changing_its_needs() {
    let original = Citizen::with_hunger_rate(f64::MAX, f64::MAX).unwrap();

    assert_eq!(
        original.advance(HOUR_MS),
        Err(SimulationError::HungerOverflow)
    );
    assert_eq!(original.hunger(), f64::MAX);
    assert_eq!(original.hunger_per_hour(), f64::MAX);
}

#[test]
fn advancing_updates_each_citizen_and_the_clock_once() {
    let (universe, satiated_id) =
        Universe::default().with_citizen("Ada", Citizen::with_hunger_rate(-5.0, 10.0).unwrap());
    let (universe, hungry_id) =
        universe.with_citizen("Bea", Citizen::with_hunger_rate(99.0, 4.0).unwrap());

    let advanced = universe.advance(HOUR_MS).unwrap();

    assert_eq!(advanced.current_time_ms(), HOUR_MS);
    assert_eq!(advanced.agents().len(), 2);
    assert_eq!(citizen(&advanced, satiated_id).hunger(), 5.0);
    assert_eq!(citizen(&advanced, hungry_id).hunger(), 103.0);
    assert_eq!(citizen(&advanced, satiated_id).hunger_per_hour(), 10.0);
    assert_eq!(universe.current_time_ms(), 0);
    assert_eq!(citizen(&universe, satiated_id).hunger(), -5.0);
    assert_eq!(citizen(&universe, hungry_id).hunger(), 99.0);
}

#[test]
fn subsecond_steps_accumulate_and_match_one_large_step() {
    let (universe, id) =
        Universe::default().with_citizen("Ada", Citizen::with_hunger_rate(-0.5, 3.7).unwrap());
    let first_step = universe.advance(17).unwrap();
    assert!(citizen(&first_step, id).hunger() > citizen(&universe, id).hunger());

    let mut small_steps = universe.clone();
    for _ in 0..1000 {
        small_steps = small_steps.advance(17).unwrap();
    }
    let large_step = universe.advance(17_000).unwrap();

    assert_eq!(small_steps.current_time_ms(), 17_000);
    assert_eq!(small_steps.current_time_ms(), large_step.current_time_ms());
    assert!((citizen(&small_steps, id).hunger() - citizen(&large_step, id).hunger()).abs() < 1e-12);
}

#[test]
fn zero_duration_preserves_the_entire_universe() {
    let (universe, _) =
        Universe::default().with_citizen("Ada", Citizen::with_hunger_rate(-10.0, 3.0).unwrap());
    let universe = universe.advance(100).unwrap();

    assert_eq!(universe.advance(0).unwrap(), universe);
}

#[test]
fn zero_hunger_rate_preserves_satiation_while_time_advances() {
    let (universe, id) =
        Universe::default().with_citizen("Ada", Citizen::with_hunger_rate(-10.0, 0.0).unwrap());

    let advanced = universe.advance(HOUR_MS).unwrap();

    assert_eq!(advanced.current_time_ms(), HOUR_MS);
    assert_eq!(citizen(&advanced, id).hunger(), -10.0);
}

#[test]
fn empty_universe_advances_and_rejects_clock_overflow() {
    let original = Universe::default();
    let universe = original.advance(u64::MAX).unwrap();

    assert_eq!(original.current_time_ms(), 0);
    assert_eq!(universe.current_time_ms(), u64::MAX);
    assert_eq!(universe.advance(0).unwrap(), universe);
    assert_eq!(universe.advance(1), Err(SimulationError::TimeOverflow));
    assert_eq!(universe.current_time_ms(), u64::MAX);
}

#[test]
fn nonfinite_hunger_results_are_rejected_without_changing_the_source() {
    for (starting_hunger, elapsed_ms) in [(f64::MAX, HOUR_MS), (0.0, 2 * HOUR_MS)] {
        let (universe, _) =
            Universe::default().with_citizen("Ada", Citizen::with_hunger_rate(1.0, 1.0).unwrap());
        let (universe, _) = universe.with_citizen(
            "Bea",
            Citizen::with_hunger_rate(starting_hunger, f64::MAX).unwrap(),
        );
        let snapshot = universe.clone();

        assert_eq!(
            universe.advance(elapsed_ms),
            Err(SimulationError::HungerOverflow)
        );
        assert_eq!(universe, snapshot);
    }
}

#[test]
fn branches_can_advance_and_create_citizens_on_other_threads_independently() {
    let mut universe = Universe::default();
    let mut ids = Vec::new();
    for index in 0..128 {
        let (next, id) = universe.with_citizen(
            format!("Citizen {index}"),
            Citizen::with_hunger_rate(-10.0, 2.0).unwrap(),
        );
        universe = next;
        ids.push(id);
    }

    let branches = thread::scope(|scope| {
        let tasks = [(HOUR_MS, "First branch"), (2 * HOUR_MS, "Second branch")].map(
            |(elapsed_ms, name)| {
                let snapshot = universe.clone();
                let original = &universe;

                scope.spawn(move || {
                    let advanced = snapshot.advance(elapsed_ms).unwrap();
                    let branch =
                        advanced.with_citizen(name, Citizen::with_hunger_rate(0.0, 1.0).unwrap());
                    assert_eq!(original.current_time_ms(), 0);
                    assert_eq!(original.agents().len(), 128);
                    branch
                })
            },
        );

        tasks.map(|task| task.join().unwrap())
    });

    let [(first, first_new_id), (second, second_new_id)] = branches;
    assert_eq!(universe.current_time_ms(), 0);
    assert_eq!(universe.agents().len(), 128);
    assert_eq!(first.current_time_ms(), HOUR_MS);
    assert_eq!(second.current_time_ms(), 2 * HOUR_MS);
    assert_eq!(first.agents().len(), 129);
    assert_eq!(second.agents().len(), 129);
    assert_ne!(first_new_id, second_new_id);
    assert!(!first.agents().contains_key(&second_new_id));
    assert!(!second.agents().contains_key(&first_new_id));
    assert!(!universe.agents().contains_key(&first_new_id));
    assert!(!universe.agents().contains_key(&second_new_id));

    for id in ids {
        assert_eq!(citizen(&universe, id).hunger(), -10.0);
        assert_eq!(citizen(&first, id).hunger(), -8.0);
        assert_eq!(citizen(&second, id).hunger(), -6.0);
        assert_eq!(first.agents()[&id].name, universe.agents()[&id].name);
        assert_eq!(second.agents()[&id].name, universe.agents()[&id].name);
    }
}
