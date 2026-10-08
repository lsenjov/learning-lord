use learning_lord_simulation::{
    AgentKind, Citizen, CitizenAction, THIRST_PER_HOUR, Universe, marketplace::Good,
};

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn thirst_advances_and_drinks_partial_water_gradually_without_waste() {
    let citizen = Citizen::new(0.0).unwrap().with_thirst(-50.0).unwrap();
    close(citizen.advance(86_400_000).unwrap().thirst(), 50.0);
    let partial = citizen
        .with_good(Good::Water, 100)
        .unwrap()
        .start_action(CitizenAction::Drink)
        .unwrap();
    assert_eq!(partial.active_action().unwrap().duration_ms(), 30_000);
    assert_eq!(partial.units(Good::Water), 0);
    close(
        partial.advance(15_000).unwrap().thirst(),
        -62.5 + THIRST_PER_HOUR / 240.0,
    );
    let capped = citizen
        .with_thirst(-99.0)
        .unwrap()
        .with_good(Good::Water, 200)
        .unwrap()
        .start_action(CitizenAction::Drink)
        .unwrap();
    assert_eq!(capped.active_action().unwrap().drinking_units(), 4);
    assert_eq!(capped.units(Good::Water), 196);
    assert!(
        citizen
            .with_thirst(-100.0)
            .unwrap()
            .with_good(Good::Water, 200)
            .unwrap()
            .start_action(CitizenAction::Drink)
            .unwrap()
            .active_action()
            .is_none()
    );
}

#[test]
fn meal_and_water_finish_their_components_in_parallel_and_history_is_chunk_independent() {
    for berries in [20, 155] {
        let citizen = Citizen::with_hunger_rate(0.0, 0.0)
            .unwrap()
            .with_thirst(20.0)
            .unwrap()
            .with_berries(berries)
            .unwrap()
            .with_good(Good::Water, 200)
            .unwrap();
        let started = citizen.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(
            started.active_action().unwrap().duration_ms(),
            (berries * 1000).max(60_000)
        );
        let midway = started.advance(30_000).unwrap();
        close(midway.hunger(), -(berries.min(30) as f64) * 100.0 / 310.0);
        close(midway.thirst(), -5.0 + THIRST_PER_HOUR / 120.0);
        let (world, id) = Universe::with_map(citizen.map())
            .with_citizen("Ada", citizen)
            .unwrap();
        let world = world.start_action(id, CitizenAction::Eat).unwrap();
        let whole = world.advance(180_000).unwrap();
        let mut chunks = world;
        for _ in 0..180 {
            chunks = chunks.advance(1000).unwrap();
        }
        let a = &whole.citizen_history(id).unwrap().current;
        let b = &chunks.citizen_history(id).unwrap().current;
        close(
            a.average_wellbeing().unwrap(),
            b.average_wellbeing().unwrap(),
        );
        close(a.worst_hunger, b.worst_hunger);
        close(a.worst_thirst, b.worst_thirst);
        assert_eq!(a.consumed[Good::Water as usize], 200);
        assert_eq!(a.consumed[Good::Berries as usize], berries);
        assert_eq!(
            a.activity.drinking_ms,
            60_000_u64.saturating_sub(berries * 1000)
        );
        assert_eq!(a.activity.eating_ms, berries * 1000);
        let AgentKind::Citizen(done) = &whole.agents()[&id].kind;
        close(done.thirst(), -30.0 + THIRST_PER_HOUR / 20.0);
    }
}
