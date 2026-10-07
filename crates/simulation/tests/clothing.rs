use learning_lord_simulation::{
    CLOTHING_MAX_PENALTY, Citizen, CitizenAction, EQUIP_CLOTHING_DURATION_MS, GARMENT_LIFETIME_MS,
    SimulationError, marketplace::Good,
};

#[test]
fn worn_garments_are_separate_from_inventory_and_scale_wealth_and_need() {
    let bare = Citizen::new(0.0).unwrap();
    let dressed = bare.with_garment_condition(Some(0.5)).unwrap();
    assert_eq!(bare.clothing_need(), CLOTHING_MAX_PENALTY);
    assert_eq!(dressed.clothing_need(), 10.0);
    assert_eq!(dressed.units(Good::FlaxGarment), 0);
    assert_eq!(
        dressed.wealth().unwrap() - bare.wealth().unwrap(),
        Good::FlaxGarment.core_price() * 0.5
    );
    assert_eq!(bare.garment_condition(), None);
    assert_eq!(
        dressed
            .with_garment_condition(Some(2.0))
            .unwrap()
            .garment_condition(),
        Some(1.0)
    );
    assert_eq!(
        dressed
            .with_garment_condition(Some(-1.0))
            .unwrap()
            .garment_condition(),
        Some(0.0)
    );
    assert_eq!(
        dressed.with_garment_condition(Some(f64::NAN)),
        Err(SimulationError::InvalidGarmentCondition)
    );
}

#[test]
fn wear_is_continuous_partition_independent_and_clamped() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_garment_condition(Some(1.0))
        .unwrap();
    let elapsed = GARMENT_LIFETIME_MS / 4;
    let whole = source.advance(elapsed).unwrap();
    let split = source
        .advance(elapsed / 3)
        .unwrap()
        .advance(elapsed - elapsed / 3)
        .unwrap();
    assert!(
        (whole.garment_condition().unwrap() - split.garment_condition().unwrap()).abs() < 1e-12
    );
    assert_eq!(whole.garment_condition(), Some(0.75));
    assert_eq!(source.garment_condition(), Some(1.0));
    assert_eq!(
        whole
            .advance(GARMENT_LIFETIME_MS * 2)
            .unwrap()
            .garment_condition(),
        Some(0.0)
    );
    let sleeping = source
        .start_action(CitizenAction::Sleep)
        .unwrap()
        .advance(elapsed)
        .unwrap();
    assert_eq!(sleeping.garment_condition(), whole.garment_condition());
    let waiting = source
        .start_action(CitizenAction::Wait)
        .unwrap()
        .advance(elapsed)
        .unwrap();
    assert_eq!(waiting.garment_condition(), whole.garment_condition());
}

#[test]
fn equip_consumes_at_completion_and_discards_old_garment() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_garment_condition(Some(0.25))
        .unwrap()
        .with_good(Good::FlaxGarment, 2)
        .unwrap();
    let source = source
        .with_position(learning_lord_simulation::locations::Position {
            x: 500.0,
            y: -400.0,
        })
        .unwrap();
    let started = source.start_action(CitizenAction::EquipClothing).unwrap();
    assert_eq!(started.units(Good::FlaxGarment), 2);
    assert_eq!(started.garment_condition(), Some(0.25));
    let midway = started.advance(EQUIP_CLOTHING_DURATION_MS - 1).unwrap();
    assert_eq!(midway.units(Good::FlaxGarment), 2);
    assert!(midway.garment_condition().unwrap() < 0.25);
    let complete = midway.advance(1).unwrap();
    assert_eq!(complete.units(Good::FlaxGarment), 1);
    assert_eq!(complete.garment_condition(), Some(1.0));
    assert!(complete.active_action().is_none());
    assert_eq!(source.units(Good::FlaxGarment), 2);
    assert_eq!(
        Citizen::new(0.0)
            .unwrap()
            .start_action(CitizenAction::EquipClothing),
        Err(SimulationError::MissingInputs)
    );
    let extra = started
        .advance(EQUIP_CLOTHING_DURATION_MS + GARMENT_LIFETIME_MS / 2)
        .unwrap();
    assert_eq!(extra.garment_condition(), Some(0.5));
}

#[test]
fn garments_trade_as_whole_items_without_transferring_worn_condition() {
    use learning_lord_simulation::locations::Map;
    use learning_lord_simulation::{
        AgentKind, TRADE_DURATION_MS, Universe, marketplace::ShoppingList,
    };
    let (town, seller) = Universe::with_map(Map::default())
        .with_prices(
            learning_lord_simulation::marketplace::Prices::default()
                .with_price(
                    learning_lord_simulation::marketplace::Good::FlaxGarment,
                    96.0,
                )
                .unwrap(),
        )
        .with_citizen(
            "Seller",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::FlaxGarment, 1)
                .unwrap()
                .with_garment_condition(Some(0.5))
                .unwrap(),
        )
        .unwrap();
    let (town, buyer) = town
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let listed = town
        .start_action(seller, CitizenAction::List(Good::FlaxGarment, 1))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let bought = listed
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::new([(Good::FlaxGarment, 1)]).unwrap()),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let AgentKind::Citizen(buyer_state) = &bought.agents()[&buyer].kind;
    let AgentKind::Citizen(seller_state) = &bought.agents()[&seller].kind;
    assert_eq!(buyer_state.units(Good::FlaxGarment), 1);
    assert_eq!(buyer_state.garment_condition(), None);
    assert_eq!(buyer_state.coins(), 4);
    assert_eq!(seller_state.coins(), 96);
    assert!(seller_state.garment_condition().unwrap() < 0.5);
    assert_eq!(town.market().trades().len(), 0);
    assert_eq!(bought.market().trades()[0].units, 1);
}

#[test]
fn clothing_chain_keeps_exact_integer_intermediates_until_garment_completion() {
    use learning_lord_simulation::{
        AgentKind, StartingRole, Universe,
        locations::{Location, Map},
        production::{Recipe, Skill},
    };
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Farmer)
        .with_skill(Skill::Weaving, 1.0)
        .unwrap()
        .with_skill(Skill::Tailoring, 1.0)
        .unwrap();
    let (mut town, id) = Universe::with_map(Map::default())
        .with_citizen("Worker", worker)
        .unwrap();
    for location in [Location::Field, Location::Weavery, Location::Tailory] {
        town = town.with_property(id, location).unwrap().0;
    }
    let AgentKind::Citizen(worker) = &town.agents()[&id].kind;
    let mut worker = worker.clone();
    for recipe in [Recipe::GrowFlax, Recipe::SpinThread, Recipe::WeaveCloth] {
        let duration = worker
            .action_duration_ms(CitizenAction::Produce(recipe))
            .unwrap();
        worker = worker
            .start_action(CitizenAction::Produce(recipe))
            .unwrap()
            .advance(duration)
            .unwrap();
    }
    assert_eq!(worker.units(Good::Cloth), 200);
    for _ in 0..8 {
        let duration = worker
            .action_duration_ms(CitizenAction::Produce(Recipe::MakeClothingBlock))
            .unwrap();
        worker = worker
            .start_action(CitizenAction::Produce(Recipe::MakeClothingBlock))
            .unwrap()
            .advance(duration)
            .unwrap();
    }
    assert_eq!(worker.units(Good::Cloth), 0);
    assert_eq!(worker.units(Good::FlaxBlock), 8);
    let duration = worker
        .action_duration_ms(CitizenAction::Produce(Recipe::AssembleGarment))
        .unwrap();
    let started = worker
        .start_action(CitizenAction::Produce(Recipe::AssembleGarment))
        .unwrap();
    assert_eq!(
        started
            .advance(duration - 1)
            .unwrap()
            .units(Good::FlaxBlock),
        8
    );
    let finished = started.advance(duration).unwrap();
    assert_eq!(finished.units(Good::FlaxBlock), 0);
    assert_eq!(finished.units(Good::FlaxGarment), 1);
    for good in [Good::Flax, Good::Thread, Good::Cloth] {
        assert_eq!(finished.units(good), 0);
    }
}
