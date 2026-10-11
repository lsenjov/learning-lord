use learning_lord_simulation::marketplace::{Good, ShoppingList};
use learning_lord_simulation::planning::{goals::Effect, plan};
use learning_lord_simulation::{AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe};

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn reserve_bonus_has_two_slopes_and_a_cap() {
    for (nutrition, bonus) in [
        (0.0, 0.0),
        (50.0, 5.0),
        (100.0, 10.0),
        (200.0, 12.0),
        (300.0, 14.0),
        (400.0, 14.0),
    ] {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_good(
                Good::BreadRoll,
                (nutrition / Good::BreadRoll.nutrition_per_unit().unwrap()).ceil() as u64,
            )
            .unwrap();
        close(citizen.food_reserve_wellbeing(), bonus);
        close(
            citizen.personal_wellbeing().unwrap(),
            citizen.wealth().unwrap() * 0.1 + bonus - 20.0,
        );
    }
    let mixed = Citizen::new(0.0)
        .unwrap()
        .with_good(Good::Berries, 155)
        .unwrap()
        .with_good(Good::BreadRoll, 5)
        .unwrap()
        .with_good(Good::BerryPie, 2)
        .unwrap();
    close(mixed.food_nutrition(), 220.0);
    close(mixed.food_reserve_wellbeing(), 12.4);
}

#[test]
fn listing_excludes_food_from_bonus_and_excess_preserves_the_cap() {
    let citizen = Citizen::new(0.0)
        .unwrap()
        .with_good(Good::BreadRoll, 40)
        .unwrap();
    close(
        citizen.excess_goods().unwrap().units(Good::BreadRoll) as f64,
        10.0,
    );
    let listed = citizen
        .start_action(CitizenAction::ListExcess)
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    close(listed.food_nutrition(), 300.0);
    close(listed.food_reserve_wellbeing(), 14.0);
    close(listed.wealth().unwrap(), citizen.wealth().unwrap());
    let all_listed = citizen
        .start_action(CitizenAction::List(Good::BreadRoll, 40))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    close(all_listed.food_reserve_wellbeing(), 0.0);
}

#[test]
fn eating_loses_reserve_bonus_but_hunger_relief_outweighs_it() {
    let hungry = Citizen::with_needs(80.0, -100.0)
        .unwrap()
        .with_good(Good::BreadRoll, 10)
        .unwrap();
    let eating = hungry.start_action(CitizenAction::Eat).unwrap();
    let eaten = eating
        .advance(eating.active_action().unwrap().duration_ms())
        .unwrap();
    close(eaten.food_reserve_wellbeing(), 5.0);
    assert!(eaten.personal_wellbeing().unwrap() > hungry.personal_wellbeing().unwrap());
}

fn market_with_bread(nutrition: f64) -> (Universe, learning_lord_simulation::AgentId) {
    use learning_lord_simulation::locations::{Location, Map, Position};
    let map = Map::new(
        Position { x: 500.0, y: 0.0 },
        Position::default(),
        Position { x: 20.0, y: 0.0 },
    )
    .unwrap();
    let market = map.public_place(Location::Market);
    let universe = Universe::with_map(map).with_prices(
        learning_lord_simulation::marketplace::Prices::default()
            .with_price(learning_lord_simulation::marketplace::Good::Berries, 5.0)
            .unwrap()
            .with_price(learning_lord_simulation::marketplace::Good::BreadRoll, 3.0)
            .unwrap(),
    );
    let (universe, seller) = universe
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::BreadRoll, 50)
                .unwrap(),
        )
        .unwrap();
    let universe = universe
        .start_action(seller, CitizenAction::Travel(market))
        .unwrap();
    let AgentKind::Citizen(citizen) = &universe.agents()[&seller].kind;
    let universe = universe
        .advance(citizen.active_action().unwrap().remaining_ms())
        .unwrap();
    let universe = universe
        .start_action(seller, CitizenAction::List(Good::BreadRoll, 50))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    universe
        .with_citizen(
            "Buyer",
            Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_coins(300)
                .unwrap()
                .with_good(
                    Good::BreadRoll,
                    (nutrition / Good::BreadRoll.nutrition_per_unit().unwrap()).ceil() as u64,
                )
                .unwrap(),
        )
        .unwrap()
}

#[test]
fn replenishment_above_meal_stock_chooses_an_actual_purchase_and_reports_it_during_travel() {
    let (universe, buyer) = market_with_bread(50.0);
    let snapshot = universe.clone();
    let AgentKind::Citizen(citizen) = &universe.agents()[&buyer].kind;
    let selected = plan(citizen).unwrap();
    assert_eq!(
        selected.decision().unwrap().selected_goal,
        Effect::ReplenishReserves
    );
    let first_goal = &selected.actions()[selected.goals()[0].actions.clone()];
    assert!(matches!(
        first_goal,
        [
            CitizenAction::Travel(_),
            CitizenAction::BuyAt { .. },
            CitizenAction::Travel(_),
            CitizenAction::DepositStored { .. }
        ]
    ));
    let CitizenAction::BuyAt { list: basket, .. } = first_goal[1] else {
        unreachable!()
    };
    let target =
        50.0 + basket.units(Good::BreadRoll) as f64 * Good::BreadRoll.nutrition_per_unit().unwrap();
    assert!([100.0, 200.0, 300.0].contains(&target));
    assert_eq!(universe, snapshot);
    assert_eq!(universe.market().requested(buyer), ShoppingList::default());

    let travelling = universe.start_planning(buyer).unwrap();
    close(
        travelling.market().requested(buyer).units(Good::BreadRoll) as f64,
        basket.units(Good::BreadRoll) as f64,
    );
    let AgentKind::Citizen(citizen) = &travelling.agents()[&buyer].kind;
    assert!(matches!(
        citizen.active_action().unwrap().action(),
        CitizenAction::Travel(_)
    ));
    let arrived = travelling
        .advance(citizen.active_action().unwrap().remaining_ms())
        .unwrap();
    close(
        arrived.market().requested(buyer).units(Good::BreadRoll) as f64,
        basket.units(Good::BreadRoll) as f64,
    );
    let bought = arrived.advance(TRADE_DURATION_MS).unwrap();
    close(
        bought.market().requested(buyer).units(Good::BreadRoll) as f64,
        0.0,
    );
    let AgentKind::Citizen(citizen) = &bought.agents()[&buyer].kind;
    close(citizen.food_nutrition(), target);
}

#[test]
fn foraging_does_not_publish_a_food_reserve_wishlist() {
    let (universe, worker) =
        Universe::with_map(learning_lord_simulation::locations::Map::default())
            .with_citizen(
                "Worker",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(learning_lord_simulation::StartingRole::Woodcutter)
                    .with_good(Good::BreadRoll, 5)
                    .unwrap(),
            )
            .unwrap();
    let forest = universe
        .map()
        .public_place(learning_lord_simulation::locations::Location::Forest);
    let universe = universe
        .start_action(worker, CitizenAction::Travel(forest))
        .unwrap();
    let AgentKind::Citizen(citizen) = &universe.agents()[&worker].kind;
    let travel_ms = citizen
        .active_action()
        .map_or(0, |active| active.remaining_ms());
    let universe = universe
        .advance(travel_ms)
        .unwrap()
        .start_action(
            worker,
            CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
        )
        .unwrap();
    assert_eq!(universe.market().requested(worker), ShoppingList::default());
}

#[test]
fn selected_variety_shopping_publishes_and_clears_its_real_basket() {
    use learning_lord_simulation::locations::{Location, Map, Position};
    let map = Map::new(
        Position::default(),
        Position::default(),
        Position { x: 10.0, y: 0.0 },
    )
    .unwrap();
    let (world, seller) = Universe::with_map(map)
        .with_prices(
            learning_lord_simulation::marketplace::Prices::default()
                .with_price(Good::Fish, 0.1)
                .unwrap()
                .with_price(Good::Cabbage, 0.1)
                .unwrap(),
        )
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Fish, 20)
                .unwrap()
                .with_good(Good::Cabbage, 20)
                .unwrap(),
        )
        .unwrap();
    let market = world.map().public_place(Location::Market);
    let moved = world
        .start_action(seller, CitizenAction::Travel(market))
        .unwrap();
    let AgentKind::Citizen(citizen) = &moved.agents()[&seller].kind;
    let world = moved
        .advance(citizen.active_action().unwrap().remaining_ms())
        .unwrap();
    let world = world
        .start_action(seller, CitizenAction::List(Good::Fish, 20))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let world = world
        .start_action(seller, CitizenAction::List(Good::Cabbage, 20))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let (world, buyer) = world
        .with_citizen(
            "Buyer",
            Citizen::with_needs(-50.0, -100.0)
                .unwrap()
                .with_thirst(-50.0)
                .unwrap()
                .with_garment_condition(Some(1.0))
                .unwrap()
                .with_coins(1000)
                .unwrap()
                .with_good(Good::BreadRoll, 70)
                .unwrap()
                .with_good(Good::Water, 400)
                .unwrap(),
        )
        .unwrap();
    let before = world.clone();
    let AgentKind::Citizen(citizen) = &world.agents()[&buyer].kind;
    let selected = plan(citizen).unwrap();
    let first = &selected.actions()[selected.goals()[0].actions.clone()];
    assert!(
        matches!(
            first,
            [
                CitizenAction::Travel(_),
                CitizenAction::BuyAt { .. },
                CitizenAction::Travel(_),
                CitizenAction::DepositStored { .. }
            ]
        ),
        "{first:?}"
    );
    let CitizenAction::BuyAt { list: basket, .. } = first[1] else {
        unreachable!()
    };
    assert!(basket.units(Good::Fish) > 0 || basket.units(Good::Cabbage) > 0);
    assert_eq!(world, before);
    assert_eq!(world.market().requested(buyer), ShoppingList::default());
    let travelling = world.start_planning(buyer).unwrap();
    for good in Good::FOOD {
        assert_eq!(
            travelling.market().requested(buyer).units(good),
            basket.units(good)
        );
    }
    let AgentKind::Citizen(citizen) = &travelling.agents()[&buyer].kind;
    let arrived = travelling
        .advance(citizen.active_action().unwrap().remaining_ms())
        .unwrap();
    for good in Good::FOOD {
        assert_eq!(
            arrived.market().requested(buyer).units(good),
            basket.units(good)
        );
    }
    let bought = arrived.advance(TRADE_DURATION_MS).unwrap();
    for good in Good::FOOD {
        assert_eq!(bought.market().requested(buyer).units(good), 0);
    }
}
