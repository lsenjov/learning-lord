use learning_lord_simulation::{
    ACTION_DURATION_MS, AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe,
    locations::Location,
    marketplace::{DAY_MS, Good, UPDATE_TIME_MS},
    production::Recipe,
};

fn world(citizen: Citizen) -> (Universe, learning_lord_simulation::AgentId) {
    Universe::with_map(citizen.map())
        .with_citizen("Ada", citizen)
        .unwrap()
}

#[test]
fn actual_yields_and_activity_live_only_on_universe_snapshots() {
    let (source, id) = world(Citizen::new(0.0).unwrap());
    let started = source
        .start_action(id, CitizenAction::Produce(Recipe::Forage))
        .unwrap();
    let done = started.advance(ACTION_DURATION_MS).unwrap();
    let history = &done.citizen_history(id).unwrap().current;
    let AgentKind::Citizen(citizen) = &done.agents()[&id].kind;
    assert_eq!(
        history.produced[Good::Berries as usize],
        citizen.berries_units()
    );
    assert!((5..=15).contains(&history.produced[Good::Berries as usize]));
    assert_eq!(history.activity.production_ms, ACTION_DURATION_MS);
    assert_eq!(source.citizen_history(id).unwrap().current.elapsed_ms, 0);
    let _ = started.start_planning(id); // Busy requests leave source history unchanged.
    assert_eq!(
        started.citizen_history(id).unwrap().current.produced[Good::Berries as usize],
        0
    );
}

#[test]
fn wellbeing_integral_is_independent_of_advance_chunk_size_and_need_clamps() {
    let citizen = Citizen::with_needs(-2.0, -2.0)
        .unwrap()
        .with_garment_condition(Some(0.0001))
        .unwrap();
    let (world, id) = world(citizen);
    let long = world.advance(2 * 3_600_000).unwrap();
    let mut short = world;
    for _ in 0..120 {
        short = short.advance(60_000).unwrap();
    }
    let long = &long.citizen_history(id).unwrap().current;
    let short = &short.citizen_history(id).unwrap().current;
    assert!(
        (long.average_wellbeing().unwrap() - short.average_wellbeing().unwrap()).abs() < 1e-10,
        "{:?} {:?}",
        long.average_wellbeing(),
        short.average_wellbeing()
    );
    assert_eq!(long.worst_clothing_need, 20.0);
    assert!((long.worst_hunger - short.worst_hunger).abs() < 1e-10);
}

#[test]
fn production_reward_is_not_spread_across_the_completed_action() {
    let (world, id) = world(Citizen::new(0.0).unwrap());
    let active = world
        .start_action(id, CitizenAction::Produce(Recipe::Forage))
        .unwrap();
    let whole = active.advance(ACTION_DURATION_MS).unwrap();
    let split = active
        .advance(ACTION_DURATION_MS / 2)
        .unwrap()
        .advance(ACTION_DURATION_MS / 2)
        .unwrap();
    let whole = &whole.citizen_history(id).unwrap().current;
    let split = &split.citizen_history(id).unwrap().current;
    assert_eq!(whole.produced, split.produced);
    assert_eq!(whole.activity, split.activity);
    assert!(
        (whole.average_wellbeing().unwrap() - split.average_wellbeing().unwrap()).abs() < 1e-10
    );
}

#[test]
fn meals_record_reserved_food_and_integrate_consumption_continuously() {
    let (world, id) = world(Citizen::new(50.0).unwrap().with_berries(310).unwrap());
    let started = world.start_action(id, CitizenAction::Eat).unwrap();
    let duration = match &started.agents()[&id].kind {
        AgentKind::Citizen(c) => c.active_action().unwrap().duration_ms(),
    };
    assert_eq!(
        started.citizen_history(id).unwrap().current.consumed[Good::Berries as usize],
        155
    );
    let whole = started.advance(duration).unwrap();
    let split = started
        .advance(duration / 2)
        .unwrap()
        .advance(duration - duration / 2)
        .unwrap();
    let a = whole
        .citizen_history(id)
        .unwrap()
        .current
        .average_wellbeing()
        .unwrap();
    let b = split
        .citizen_history(id)
        .unwrap()
        .current
        .average_wellbeing()
        .unwrap();
    assert!((a - b).abs() < 1e-10);
    assert_eq!(
        whole
            .citizen_history(id)
            .unwrap()
            .current
            .activity
            .eating_ms,
        duration
    );
}

#[test]
fn day_boundary_keeps_completion_in_closing_day_and_retains_thirty() {
    let (world, id) = world(Citizen::new(0.0).unwrap());
    let world = world
        .advance(UPDATE_TIME_MS - ACTION_DURATION_MS)
        .unwrap()
        .start_action(id, CitizenAction::Produce(Recipe::Forage))
        .unwrap()
        .advance(ACTION_DURATION_MS)
        .unwrap();
    let h = world.citizen_history(id).unwrap();
    assert_eq!(h.completed.len(), 1);
    assert!(h.completed[0].produced[Good::Berries as usize] > 0);
    assert_eq!(h.current.start_ms, UPDATE_TIME_MS);
    assert_eq!(h.current.elapsed_ms, 0);
    let next = world.advance(31 * DAY_MS).unwrap();
    let h = next.citizen_history(id).unwrap();
    assert_eq!(h.completed.len(), 30);
    assert_eq!(h.current.elapsed_ms, 0);
}

#[test]
fn purchases_record_both_sides_and_ignore_price_revaluation() {
    let seller = Citizen::new(0.0).unwrap().with_berries(1000).unwrap();
    let map = seller.map();
    let seller = seller
        .with_position(map.position(map.public_place(Location::Market)))
        .unwrap();
    let (world, seller_id) = world(seller);
    let buyer = Citizen::new(0.0)
        .unwrap()
        .with_map(world.map())
        .unwrap()
        .with_coins(100)
        .unwrap();
    let buyer = buyer
        .with_position(
            world
                .map()
                .position(world.map().public_place(Location::Market)),
        )
        .unwrap();
    let (world, buyer_id) = world.with_citizen("Bram", buyer).unwrap();
    let world = world
        .start_action(seller_id, CitizenAction::List(Good::Berries, 1000))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let before = world.citizen_history(seller_id).unwrap().current.clone();
    let world = world.with_prices(world.prices().with_price(Good::Berries, 4.0).unwrap());
    assert_eq!(world.citizen_history(seller_id).unwrap().current, before);
    let world = world
        .start_action(buyer_id, CitizenAction::BuyFood(Good::Berries))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let seller = &world.citizen_history(seller_id).unwrap().current;
    let buyer = &world.citizen_history(buyer_id).unwrap().current;
    assert_eq!(
        seller.sold[Good::Berries as usize],
        buyer.bought[Good::Berries as usize]
    );
    assert!(seller.sold[Good::Berries as usize] > 0);
    assert_eq!(seller.coins_earned, buyer.coins_spent);
    assert!(seller.coins_earned > 0);
    assert_eq!(seller.coins_spent, 0);
    assert_eq!(buyer.coins_earned, 0);
}
