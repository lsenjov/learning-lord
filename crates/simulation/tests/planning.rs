use learning_lord_simulation::planning::plan;
use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, SimulationError, Universe,
};

const HALF_HOUR_MS: u64 = 30 * 60 * 1000;
const TWO_HOURS_MS: u64 = 4 * HALF_HOUR_MS;

fn citizen(universe: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
    citizen
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn search_matches_all_256_sequences_and_preserves_the_source() {
    for hunger in [-140.0, -80.0, -5.0, 25.0, 140.0] {
        let original = Citizen::new(hunger).unwrap();
        let snapshot = original.clone();
        let chosen = plan(&original).unwrap();
        let mut best_score = f64::NEG_INFINITY;
        let mut best_actions = [CitizenAction::Wait; 8];

        for sequence in 0..256 {
            let actions = std::array::from_fn(|index| {
                if sequence & (1 << (7 - index)) == 0 {
                    CitizenAction::Wait
                } else {
                    CitizenAction::Eat
                }
            });
            let mut predicted = original.clone();
            let mut sum = 0.0;
            for action in actions {
                predicted = predicted
                    .start_action(action)
                    .unwrap()
                    .advance(HALF_HOUR_MS)
                    .unwrap();
                sum += predicted.personal_wellbeing().unwrap();
            }
            if sum / 8.0 > best_score {
                best_score = sum / 8.0;
                best_actions = actions;
            }
        }

        assert_eq!(chosen.actions(), &best_actions);
        assert_close(chosen.average_wellbeing(), best_score);
        assert_eq!(original, snapshot);
    }
}

#[test]
fn scoring_averages_completion_states_and_keeps_the_first_equal_plan() {
    let starving = Citizen::with_hunger_rate(150.0, 0.0).unwrap();
    let chosen = plan(&starving).unwrap();
    assert_eq!(
        chosen.actions(),
        &[
            CitizenAction::Eat,
            CitizenAction::Eat,
            CitizenAction::Eat,
            CitizenAction::Wait,
            CitizenAction::Wait,
            CitizenAction::Wait,
            CitizenAction::Wait,
            CitizenAction::Wait,
        ]
    );
    assert_eq!(chosen.average_wellbeing(), -18.75);

    let satiated = Citizen::with_hunger_rate(-50.0, 0.0).unwrap();
    let chosen = plan(&satiated).unwrap();
    assert_eq!(chosen.actions(), &[CitizenAction::Wait; 8]);
    assert_eq!(chosen.average_wellbeing(), 0.0);
}

#[test]
fn large_finite_scores_can_be_averaged_without_overflowing() {
    let citizen = Citizen::with_hunger_rate(-f64::MAX, 0.0).unwrap();
    assert_eq!(plan(&citizen).unwrap().average_wellbeing(), -f64::MAX);
}

#[test]
fn execution_keeps_four_actions_then_replans_at_exactly_two_hours() {
    let original = Citizen::new(170.0).unwrap();
    let initial_plan = plan(&original).unwrap();
    let mut executing = original.start_planning().unwrap();
    let mut manual = original.clone();

    assert_eq!(executing.hunger(), original.hunger());
    assert_eq!(executing.advance(0).unwrap(), executing);

    for index in 0..4 {
        assert_eq!(executing.plan_execution().unwrap().plan(), &initial_plan);
        assert_eq!(executing.plan_execution().unwrap().action_index(), index);
        assert_eq!(
            executing.active_action().unwrap().action(),
            initial_plan.actions()[index]
        );
        assert_eq!(
            executing.active_action().unwrap().remaining_ms(),
            HALF_HOUR_MS
        );
        manual = manual
            .start_action(initial_plan.actions()[index])
            .unwrap()
            .advance(HALF_HOUR_MS)
            .unwrap();

        let almost_done = executing.advance(HALF_HOUR_MS - 1).unwrap();
        assert_eq!(almost_done.plan_execution().unwrap().plan(), &initial_plan);
        assert_eq!(almost_done.plan_execution().unwrap().action_index(), index);
        assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
        executing = almost_done.advance(1).unwrap();
        assert_close(executing.hunger(), manual.hunger());
    }

    let next_plan = plan(&manual).unwrap();
    let execution = executing.plan_execution().unwrap();
    assert_eq!(execution.action_index(), 0);
    assert_eq!(execution.plan(), &next_plan);
    assert_ne!(
        execution.plan().average_wellbeing(),
        initial_plan.average_wellbeing()
    );
    assert_eq!(
        executing.active_action().unwrap().action(),
        next_plan.actions()[0]
    );
    assert_eq!(
        executing.active_action().unwrap().remaining_ms(),
        HALF_HOUR_MS
    );
    assert_eq!(original.active_action(), None);
    assert_eq!(original.plan_execution(), None);
}

#[test]
fn small_and_large_ticks_agree_across_multiple_batches_and_partial_actions() {
    let original = Citizen::new(17.3).unwrap().start_planning().unwrap();
    let snapshot = original.clone();
    let mut small_ticks = original.clone();
    for _ in 0..1000 {
        small_ticks = small_ticks.advance(37_123).unwrap();
    }
    let large_tick = original.advance(37_123_000).unwrap();

    assert_close(small_ticks.hunger(), large_tick.hunger());
    assert_eq!(small_ticks.active_action(), large_tick.active_action());
    let small_execution = small_ticks.plan_execution().unwrap();
    let large_execution = large_tick.plan_execution().unwrap();
    assert_eq!(
        small_execution.action_index(),
        large_execution.action_index()
    );
    assert_eq!(
        small_execution.plan().actions(),
        large_execution.plan().actions()
    );
    assert_close(
        small_execution.plan().average_wellbeing(),
        large_execution.plan().average_wellbeing(),
    );
    assert_eq!(original, snapshot);
}

#[test]
fn planning_rejects_busy_citizens_and_cannot_replace_a_committed_batch() {
    let idle = Citizen::new(10.0).unwrap();
    let busy = idle.start_action(CitizenAction::Wait).unwrap();
    assert_eq!(plan(&busy), Err(SimulationError::CitizenBusy));
    assert_eq!(busy.start_planning(), Err(SimulationError::CitizenBusy));

    let planned = idle.start_planning().unwrap();
    let snapshot = planned.clone();
    assert_eq!(plan(&planned), Err(SimulationError::CitizenBusy));
    assert_eq!(planned.start_planning(), Err(SimulationError::CitizenBusy));
    assert_eq!(
        planned.start_action(CitizenAction::Eat),
        Err(SimulationError::CitizenBusy)
    );
    assert_eq!(planned, snapshot);
}

#[test]
fn universe_planning_only_predicts_the_selected_citizen_and_preserves_branches() {
    let (original, id) = Universe::default().with_citizen("Ada", Citizen::new(25.0).unwrap());
    let (original, other_id) = original.with_citizen("Bea", Citizen::new(0.0).unwrap());
    let snapshot = original.clone();
    let planned = original.start_planning(id).unwrap();
    let planned_snapshot = planned.clone();

    assert_eq!(planned.current_time_ms(), 0);
    assert_eq!(planned.agents()[&other_id], original.agents()[&other_id]);
    assert_eq!(planned.agents()[&id].name, "Ada");
    assert_eq!(
        planned.agents()[&id],
        original.agents()[&id].start_planning().unwrap()
    );
    assert_eq!(planned.advance(0).unwrap(), planned);
    assert_eq!(
        planned.start_planning(id),
        Err(SimulationError::CitizenBusy)
    );

    let (first, second) = std::thread::scope(|scope| {
        let first = scope.spawn(|| planned.advance(TWO_HOURS_MS).unwrap());
        let second = scope.spawn(|| planned.advance(TWO_HOURS_MS * 2).unwrap());
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(first.current_time_ms(), TWO_HOURS_MS);
    assert_eq!(second.current_time_ms(), TWO_HOURS_MS * 2);
    assert_close(citizen(&first, other_id).hunger(), 100.0 / 12.0);
    assert_close(citizen(&second, other_id).hunger(), 100.0 / 6.0);
    assert!(citizen(&first, other_id).plan_execution().is_none());
    assert_eq!(
        citizen(&first, id).plan_execution().unwrap().action_index(),
        0
    );
    assert_eq!(planned, planned_snapshot);
    assert_eq!(original, snapshot);

    let (_, missing_id) = Universe::default().with_citizen("Missing", Citizen::new(0.0).unwrap());
    assert_eq!(
        original.start_planning(missing_id),
        Err(SimulationError::AgentNotFound)
    );
}

#[test]
fn prediction_and_replanning_errors_preserve_original_state() {
    for (rate, expected) in [
        (0.0, SimulationError::WellbeingOverflow),
        (f64::MAX, SimulationError::HungerOverflow),
    ] {
        let original = Citizen::with_hunger_rate(f64::MAX, rate).unwrap();
        let snapshot = original.clone();
        assert_eq!(plan(&original), Err(expected));
        assert_eq!(original.start_planning(), Err(expected));
        assert_eq!(original, snapshot);
    }

    let planned = Citizen::with_hunger_rate(-f64::MAX, f64::MAX / 8.0)
        .unwrap()
        .start_planning()
        .unwrap();
    let snapshot = planned.clone();
    assert_eq!(
        planned.advance(3 * TWO_HOURS_MS),
        Err(SimulationError::WellbeingOverflow)
    );
    assert_eq!(planned, snapshot);

    let (universe, _) = Universe::default().with_citizen("Ada", planned);
    let snapshot = universe.clone();
    assert_eq!(
        universe.advance(3 * TWO_HOURS_MS),
        Err(SimulationError::WellbeingOverflow)
    );
    assert_eq!(universe, snapshot);
}
