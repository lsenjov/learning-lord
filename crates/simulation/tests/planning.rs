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

fn score_actions(original: &Citizen, actions: &[CitizenAction]) -> f64 {
    let mut predicted = original.clone();
    let mut sum = 0.0;
    for &action in actions {
        predicted = predicted.start_action(action).unwrap();
        let duration_ms = predicted.active_action().unwrap().remaining_ms();
        predicted = predicted.advance(duration_ms).unwrap();
        sum += predicted.personal_wellbeing().unwrap();
    }
    sum / actions.len() as f64
}

#[test]
fn search_matches_all_256_sequences_and_preserves_the_source() {
    for hunger in [-140.0, -80.0, -5.0, 25.0, 140.0] {
        let original = Citizen::new(hunger).unwrap();
        let snapshot = original.clone();
        let chosen = plan(&original).unwrap();
        let mut best_score = f64::NEG_INFINITY;

        for sequence in 0..256 {
            let actions: [CitizenAction; 8] = std::array::from_fn(|index| {
                if sequence & (1 << (7 - index)) == 0 {
                    CitizenAction::Wait
                } else {
                    CitizenAction::Eat
                }
            });
            best_score = best_score.max(score_actions(&original, &actions));
        }

        assert_eq!(
            chosen.actions().len() as u64 * HALF_HOUR_MS,
            4 * 60 * 60 * 1000
        );
        assert_close(score_actions(&original, chosen.actions()), best_score);
        assert_close(chosen.average_wellbeing(), best_score);
        assert_eq!(original, snapshot);
    }
}

#[test]
fn scoring_averages_completion_states_and_accepts_any_optimal_tied_plan() {
    let starving = Citizen::with_hunger_rate(150.0, 0.0).unwrap();
    let chosen = plan(&starving).unwrap();
    assert_close(score_actions(&starving, chosen.actions()), -18.75);
    assert_close(chosen.average_wellbeing(), -18.75);

    let satiated = Citizen::with_hunger_rate(-50.0, 0.0).unwrap();
    let chosen = plan(&satiated).unwrap();
    assert_eq!(score_actions(&satiated, chosen.actions()), 0.0);
    assert_eq!(chosen.average_wellbeing(), 0.0);
}

#[test]
fn large_finite_scores_can_be_averaged_without_overflowing() {
    let citizen = Citizen::with_hunger_rate(-f64::MAX, 0.0).unwrap();
    assert_eq!(plan(&citizen).unwrap().average_wellbeing(), -f64::MAX);
}

#[test]
fn execution_keeps_its_plan_for_two_hours_then_replans() {
    let original = Citizen::new(170.0).unwrap();
    let mut executing = original.start_planning().unwrap();
    let initial_plan = executing.active_plan().unwrap().plan().clone();
    let mut manual = original.clone();

    assert_eq!(executing.hunger(), original.hunger());
    assert_eq!(executing.advance(0).unwrap(), executing);
    assert_eq!(executing.active_plan().unwrap().elapsed_ms(), 0);

    for index in 0..4 {
        assert_eq!(executing.active_plan().unwrap().plan(), &initial_plan);
        assert_eq!(executing.active_plan().unwrap().action_index(), index);
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
        assert_eq!(almost_done.active_plan().unwrap().plan(), &initial_plan);
        assert_eq!(almost_done.active_plan().unwrap().action_index(), index);
        assert_eq!(
            almost_done.active_plan().unwrap().elapsed_ms(),
            (index as u64 + 1) * HALF_HOUR_MS - 1
        );
        assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
        executing = almost_done.advance(1).unwrap();
        assert_close(executing.hunger(), manual.hunger());
    }

    let next_plan = plan(&manual).unwrap();
    let execution = executing.active_plan().unwrap();
    assert_eq!(execution.action_index(), 0);
    assert_eq!(execution.elapsed_ms(), 0);
    assert_close(
        execution.plan().average_wellbeing(),
        next_plan.average_wellbeing(),
    );
    assert_close(
        score_actions(&manual, execution.plan().actions()),
        next_plan.average_wellbeing(),
    );
    assert_ne!(
        execution.plan().average_wellbeing(),
        initial_plan.average_wellbeing()
    );
    assert_eq!(
        executing.active_action().unwrap().action(),
        execution.plan().actions()[0]
    );
    assert_eq!(
        executing.active_action().unwrap().remaining_ms(),
        HALF_HOUR_MS
    );
    assert_eq!(original.active_action(), None);
    assert_eq!(original.active_plan(), None);
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
    let small_execution = small_ticks.active_plan().unwrap();
    let large_execution = large_tick.active_plan().unwrap();
    assert_eq!(
        small_execution.action_index(),
        large_execution.action_index()
    );
    assert_eq!(small_execution.elapsed_ms(), 37_123_000 % TWO_HOURS_MS);
    assert_eq!(small_execution.elapsed_ms(), large_execution.elapsed_ms());
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
    let directly_planned = original.agents()[&id].start_planning().unwrap();
    assert_close(
        directly_planned.personal_wellbeing().unwrap(),
        planned.agents()[&id].personal_wellbeing().unwrap(),
    );
    assert_close(
        citizen(&planned, id)
            .active_plan()
            .unwrap()
            .plan()
            .average_wellbeing(),
        plan(citizen(&original, id)).unwrap().average_wellbeing(),
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
    assert!(citizen(&first, other_id).active_plan().is_none());
    assert_eq!(citizen(&first, id).active_plan().unwrap().action_index(), 0);
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
