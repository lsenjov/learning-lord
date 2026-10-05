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
    let mut contribution = 0.0;
    let mut elapsed = 0;
    for &action in actions {
        let before = predicted.personal_wellbeing().unwrap();
        let duration = predicted.action_duration_ms(action).unwrap();
        predicted = predict_action(&predicted, action);
        let after = predicted.personal_wellbeing().unwrap();
        contribution += (before + after) * 0.5 * duration as f64;
        elapsed += duration;
    }
    contribution / elapsed as f64
}

fn predict_action(citizen: &Citizen, action: CitizenAction) -> Citizen {
    if action == CitizenAction::Forage {
        citizen
            .advance(HALF_HOUR_MS)
            .unwrap()
            .with_berries(citizen.berries_grams() + 10.0)
            .unwrap()
    } else {
        citizen
            .start_action(action)
            .unwrap()
            .advance(citizen.action_duration_ms(action).unwrap())
            .unwrap()
    }
}

#[test]
fn goal_planning_scores_action_endpoints_and_preserves_the_source() {
    for (hunger, tiredness, berries) in [
        (-140.0, 0.0, 0.0),
        (-80.0, -100.0, 40.0),
        (25.0, 0.0, 10.1234),
        (150.0, 100.0, 200.0),
    ] {
        let original = Citizen::with_needs(hunger, tiredness)
            .unwrap()
            .with_berries(berries)
            .unwrap();
        let snapshot = original.clone();
        let chosen = plan(&original).unwrap();
        assert_close(
            score_actions(&original, chosen.actions()),
            chosen.average_wellbeing(),
        );
        let mut predicted = original.clone();
        let mut elapsed = 0;
        for &action in chosen.actions() {
            elapsed += predicted.action_duration_ms(action).unwrap();
            predicted = predict_action(&predicted, action);
        }
        assert!(elapsed >= 4 * 60 * 60 * 1000);
        assert_eq!(original, snapshot);
    }
}

#[test]
fn scoring_weights_action_endpoint_averages_and_rewards_wealth() {
    let starving = Citizen::with_hunger_rate(150.0, 0.0).unwrap();
    let chosen = plan(&starving).unwrap();
    assert_close(
        score_actions(&starving, chosen.actions()),
        chosen.average_wellbeing(),
    );

    let satiated = Citizen::with_needs(-50.0, -100.0).unwrap();
    let chosen = plan(&satiated).unwrap();
    assert_close(
        score_actions(&satiated, chosen.actions()),
        chosen.average_wellbeing(),
    );
    assert!(chosen.average_wellbeing() > 0.0);
}

#[test]
fn large_finite_scores_can_be_averaged_without_overflowing() {
    let citizen = Citizen::with_hunger_rate(-f64::MAX, 0.0).unwrap();
    assert_eq!(plan(&citizen).unwrap().average_wellbeing(), -f64::MAX);
}

#[test]
fn execution_keeps_its_plan_for_two_hours_then_replans() {
    let original = Citizen::with_needs(-50.0, -100.0).unwrap();
    let mut executing = original.start_planning().unwrap();
    let initial_plan = executing.active_plan().unwrap().plan().clone();
    let mut manual = original.clone();

    assert_eq!(executing.hunger(), original.hunger());
    assert_eq!(executing.advance(0).unwrap(), executing);
    assert_eq!(executing.active_plan().unwrap().elapsed_ms(), 0);

    let mut elapsed_ms = 0;
    for (index, action) in initial_plan.actions().iter().enumerate() {
        assert_eq!(executing.active_plan().unwrap().plan(), &initial_plan);
        assert_eq!(executing.active_plan().unwrap().action_index(), index);
        assert_eq!(
            executing.active_action().unwrap().action(),
            initial_plan.actions()[index]
        );
        let duration_ms = executing.active_action().unwrap().remaining_ms();
        manual = manual
            .start_action(*action)
            .unwrap()
            .advance(duration_ms)
            .unwrap();

        let almost_done = executing.advance(duration_ms - 1).unwrap();
        assert_eq!(almost_done.active_plan().unwrap().plan(), &initial_plan);
        assert_eq!(almost_done.active_plan().unwrap().action_index(), index);
        assert_eq!(
            almost_done.active_plan().unwrap().elapsed_ms(),
            elapsed_ms + duration_ms - 1
        );
        assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
        executing = almost_done.advance(1).unwrap();
        assert_close(executing.hunger(), manual.hunger());
        elapsed_ms += duration_ms;
        if elapsed_ms >= TWO_HOURS_MS {
            break;
        }
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
    assert_eq!(
        executing.active_action().unwrap().action(),
        execution.plan().actions()[0]
    );
    assert_eq!(
        executing.active_action().unwrap().remaining_ms(),
        manual
            .action_duration_ms(execution.plan().actions()[0])
            .unwrap()
    );
    assert_eq!(original.active_action(), None);
    assert_eq!(original.active_plan(), None);
}

#[test]
fn small_and_large_ticks_agree_across_multiple_batches_and_partial_actions() {
    let original = Citizen::with_needs(60.0, 100.0)
        .unwrap()
        .start_planning()
        .unwrap();
    let snapshot = original.clone();
    let mut small_ticks = original.clone();
    for _ in 0..1000 {
        small_ticks = small_ticks.advance(37_123).unwrap();
    }
    let large_tick = original.advance(37_123_000).unwrap();

    assert_close(small_ticks.hunger(), large_tick.hunger());
    assert_close(small_ticks.tiredness(), large_tick.tiredness());
    assert_eq!(small_ticks.active_action(), large_tick.active_action());
    let small_execution = small_ticks.active_plan().unwrap();
    let large_execution = large_tick.active_plan().unwrap();
    assert_eq!(
        small_execution.action_index(),
        large_execution.action_index()
    );
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
    let (original, id) = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_prices(learning_lord_simulation::marketplace::Prices::default())
        .with_citizen("Ada", Citizen::new(25.0).unwrap())
        .unwrap();
    let (original, other_id) = original
        .with_citizen("Bea", Citizen::new(0.0).unwrap())
        .unwrap();
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
    assert_close(citizen(&first, other_id).tiredness(), 100.0 / 12.0);
    assert_close(citizen(&second, other_id).tiredness(), 100.0 / 6.0);
    assert_eq!(planned, planned_snapshot);
    assert_eq!(original, snapshot);

    let (_, missing_id) = Universe::with_map(learning_lord_simulation::locations::Map::default())
        .with_prices(learning_lord_simulation::marketplace::Prices::default())
        .with_citizen("Missing", Citizen::new(0.0).unwrap())
        .unwrap();
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

    let planned = Citizen::with_hunger_rate(-f64::MAX, f64::MAX / 16.0)
        .unwrap()
        .start_planning()
        .unwrap();
    let snapshot = planned.clone();
    assert_eq!(
        planned.advance(8 * TWO_HOURS_MS),
        Err(SimulationError::WellbeingOverflow)
    );
    assert_eq!(planned, snapshot);

    let (universe, _) = Universe::with_map(planned.map())
        .with_prices(learning_lord_simulation::marketplace::Prices::default())
        .with_citizen("Ada", planned)
        .unwrap();
    let snapshot = universe.clone();
    assert_eq!(
        universe.advance(8 * TWO_HOURS_MS),
        Err(SimulationError::WellbeingOverflow)
    );
    assert_eq!(universe, snapshot);
}

#[test]
fn planning_sleeps_when_tired_and_eats_before_sleep_when_hungry() {
    let tired = Citizen::with_needs(-50.0, 50.0).unwrap();
    assert_eq!(plan(&tired).unwrap().actions(), &[CitizenAction::Sleep]);
    let hungry = Citizen::with_needs(60.0, 100.0)
        .unwrap()
        .with_berries(200.0)
        .unwrap();
    let chosen = plan(&hungry).unwrap();
    assert_eq!(chosen.actions()[0], CitizenAction::Eat);
    assert!(chosen.actions().contains(&CitizenAction::Sleep));
    assert!(chosen.average_wellbeing() > score_actions(&hungry, &[CitizenAction::Sleep]));
}

#[test]
fn sleep_crosses_both_horizons_and_replanning_waits_for_completion() {
    let original = Citizen::with_needs(-50.0, 50.0)
        .unwrap()
        .start_planning()
        .unwrap();
    let snapshot = original.clone();
    assert_eq!(
        original.active_plan().unwrap().plan().actions(),
        &[CitizenAction::Sleep]
    );
    let at_commitment = original.advance(TWO_HOURS_MS).unwrap();
    assert_eq!(
        at_commitment.active_plan().unwrap().elapsed_ms(),
        TWO_HOURS_MS
    );
    assert_eq!(
        at_commitment.active_plan().unwrap().plan(),
        original.active_plan().unwrap().plan()
    );
    assert_eq!(
        at_commitment.active_action().unwrap().remaining_ms(),
        3 * TWO_HOURS_MS
    );
    let almost_done = at_commitment.advance(3 * TWO_HOURS_MS - 1).unwrap();
    assert_eq!(
        almost_done.active_plan().unwrap().elapsed_ms(),
        4 * TWO_HOURS_MS - 1
    );
    assert_eq!(
        almost_done.active_action().unwrap().action(),
        CitizenAction::Sleep
    );
    assert_eq!(almost_done.active_action().unwrap().remaining_ms(), 1);
    let finished = almost_done.advance(1).unwrap();
    assert_close(finished.hunger(), -50.0 + 100.0 / 3.0);
    assert_close(finished.tiredness(), 50.0 - 100.0 * 2.0 / 3.0);
    assert_eq!(finished.active_plan().unwrap().elapsed_ms(), 0);
    assert_eq!(finished.active_plan().unwrap().action_index(), 0);
    assert!(finished.active_action().is_some());
    let crossed = original.advance(4 * TWO_HOURS_MS + 15 * 60_000).unwrap();
    let split = finished.advance(15 * 60_000).unwrap();
    assert_close(crossed.hunger(), split.hunger());
    assert_close(crossed.tiredness(), split.tiredness());
    assert_eq!(crossed.active_action(), split.active_action());
    let crossed_plan = crossed.active_plan().unwrap();
    let split_plan = split.active_plan().unwrap();
    assert_eq!(crossed_plan.plan().actions(), split_plan.plan().actions());
    assert_eq!(crossed_plan.action_index(), split_plan.action_index());
    assert_eq!(crossed_plan.elapsed_ms(), split_plan.elapsed_ms());
    assert_close(
        crossed_plan.plan().average_wellbeing(),
        split_plan.plan().average_wellbeing(),
    );
    assert_eq!(original, snapshot);
}
