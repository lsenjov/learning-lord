use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, SimulationError, Universe,
    calendar::{FIRST_WEEKLY_SETTLEMENT_MS, WEEK_MS, Weekday, next_weekly_settlement},
    locations::{Location, Map, PlaceId, Position},
    marketplace::{DAY_MS, Good},
    storage::GoodsOwner,
};
use uuid::Uuid;

fn citizen(world: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen
}

#[test]
fn calendar_rolls_over_at_midnight_and_weekly_settlement_starts_next_week() {
    assert_eq!(Weekday::at(6 * 3_600_000), Weekday::Monday);
    assert_eq!(Weekday::at(DAY_MS - 1), Weekday::Monday);
    assert_eq!(Weekday::at(DAY_MS), Weekday::Tuesday);
    assert_eq!(Weekday::at(6 * DAY_MS), Weekday::Sunday);
    assert_eq!(Weekday::at(7 * DAY_MS), Weekday::Monday);
    assert_eq!(
        next_weekly_settlement(6 * 3_600_000).unwrap(),
        FIRST_WEEKLY_SETTLEMENT_MS
    );
    assert_eq!(
        next_weekly_settlement(FIRST_WEEKLY_SETTLEMENT_MS - 1).unwrap(),
        FIRST_WEEKLY_SETTLEMENT_MS
    );
    assert_eq!(
        next_weekly_settlement(FIRST_WEEKLY_SETTLEMENT_MS).unwrap(),
        FIRST_WEEKLY_SETTLEMENT_MS + WEEK_MS
    );
    assert_eq!(
        next_weekly_settlement(FIRST_WEEKLY_SETTLEMENT_MS + 4 * WEEK_MS + 1).unwrap(),
        FIRST_WEEKLY_SETTLEMENT_MS + 5 * WEEK_MS
    );
    assert_eq!(
        next_weekly_settlement(u64::MAX),
        Err(SimulationError::TimeOverflow)
    );
    let world = Universe::starting_at(6 * 3_600_000);
    assert_eq!(world.weekday(), Weekday::Monday);
    assert_eq!(world.town_treasury(), 0);
    assert_eq!(
        world.advance(8 * DAY_MS).unwrap().weekday(),
        Weekday::Tuesday
    );
}

#[test]
fn warehouse_is_town_owned_and_not_a_private_workplace() {
    for map in [Map::default(), Map::random()] {
        let warehouses: Vec<_> = map
            .places()
            .values()
            .filter(|place| place.kind == Location::Warehouse)
            .collect();
        assert_eq!(warehouses.len(), 1);
        assert_eq!(warehouses[0].owner, None);
        assert!(warehouses[0].kind.is_town_owned());
        assert!(!warehouses[0].kind.is_public());
        assert_eq!(warehouses[0].name, "Town warehouse");
        assert_eq!(
            map.with_place(
                Location::Warehouse,
                Position::default(),
                Some(AgentId(Uuid::new_v4())),
                "Private"
            ),
            Err(SimulationError::InvalidOwnership)
        );
    }
}

#[test]
fn storage_retains_goods_ownership_independently_and_snapshots_are_immutable() {
    let (world, ada) = Universe::with_map(Map::default())
        .with_citizen(
            "Ada",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 5)
                .unwrap(),
        )
        .unwrap();
    let (world, bram) = world
        .with_citizen("Bram", Citizen::new(0.0).unwrap())
        .unwrap();
    let home = citizen(&world, ada)
        .owned_properties()
        .find(|place| place.kind == Location::Home)
        .unwrap()
        .id;
    let before = world.clone();
    let stored = world
        .with_stored_good(home, GoodsOwner::Agent(bram), Good::Bread, 3)
        .unwrap()
        .with_stored_good(home, GoodsOwner::Town, Good::Bread, 7)
        .unwrap();
    assert_eq!(
        stored
            .storage()
            .units(home, GoodsOwner::Agent(bram), Good::Bread),
        3
    );
    assert_eq!(
        stored.storage().units(home, GoodsOwner::Town, Good::Bread),
        7
    );
    assert_eq!(citizen(&stored, bram).wealth().unwrap(), 45.0);
    assert_eq!(citizen(&stored, bram).units(Good::Bread), 0);
    assert_eq!(citizen(&stored, bram).food_nutrition(), 0.0);
    assert_eq!(
        stored.withdraw_goods(ada, home, Good::Bread, 1),
        Err(SimulationError::MissingInputs)
    );
    assert_eq!(
        stored.withdraw_goods(bram, home, Good::Bread, 1),
        Err(SimulationError::PrivateProperty)
    );
    assert_eq!(world, before);
    assert!(world.storage().stock().is_empty());
}

#[test]
fn local_transfers_conserve_wealth_and_never_use_another_owners_stock() {
    let (world, ada) = Universe::with_map(Map::default())
        .with_citizen(
            "Ada",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 5)
                .unwrap(),
        )
        .unwrap();
    let home = citizen(&world, ada)
        .owned_properties()
        .find(|place| place.kind == Location::Home)
        .unwrap()
        .id;
    let deposited = world.deposit_goods(ada, home, Good::Bread, 4).unwrap();
    assert_eq!(citizen(&deposited, ada).units(Good::Bread), 1);
    assert_eq!(
        deposited
            .storage()
            .units(home, GoodsOwner::Agent(ada), Good::Bread),
        4
    );
    assert_eq!(
        citizen(&deposited, ada).wealth().unwrap(),
        citizen(&world, ada).wealth().unwrap()
    );
    assert_eq!(citizen(&deposited, ada).food_nutrition(), 50.0);
    assert_eq!(
        deposited.withdraw_goods(ada, home, Good::Bread, 5),
        Err(SimulationError::MissingInputs)
    );
    let restored = deposited.withdraw_goods(ada, home, Good::Bread, 4).unwrap();
    assert_eq!(citizen(&restored, ada).units(Good::Bread), 5);
    assert!(restored.storage().stock().is_empty());
    let busy = world.start_action(ada, CitizenAction::Wait).unwrap();
    let moved = busy.deposit_goods(ada, home, Good::Bread, 1).unwrap();
    assert_eq!(
        citizen(&moved, ada).active_action(),
        citizen(&busy, ada).active_action()
    );
    assert_eq!(
        citizen(&moved.advance(1).unwrap(), ada).units(Good::Bread),
        4
    );
}

#[test]
fn storage_validates_references_and_overflow_without_changing_source() {
    let (world, ada) = Universe::with_map(Map::default())
        .with_citizen(
            "Ada",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 1)
                .unwrap(),
        )
        .unwrap();
    let home = citizen(&world, ada)
        .owned_properties()
        .find(|place| place.kind == Location::Home)
        .unwrap()
        .id;
    assert_eq!(
        world.with_stored_good(PlaceId(Uuid::new_v4()), GoodsOwner::Town, Good::Bread, 1),
        Err(SimulationError::PlaceNotFound)
    );
    assert_eq!(
        world.with_stored_good(
            home,
            GoodsOwner::Agent(AgentId(Uuid::new_v4())),
            Good::Bread,
            1
        ),
        Err(SimulationError::AgentNotFound)
    );
    assert_eq!(
        world.with_stored_good(home, GoodsOwner::Agent(ada), Good::Bread, u64::MAX),
        Err(SimulationError::WealthOverflow)
    );
    let town = world
        .with_stored_good(home, GoodsOwner::Town, Good::Bread, u64::MAX)
        .unwrap();
    assert_eq!(
        town.withdraw_goods(ada, home, Good::Bread, 1),
        Err(SimulationError::MissingInputs)
    );
    assert!(world.storage().stock().is_empty());
}

#[test]
fn storage_transfers_preserve_active_production_inputs_and_completion() {
    use learning_lord_simulation::production::{Recipe, Skill};
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_skill(Skill::Milling, 1.0)
        .unwrap()
        .with_good(Good::Wheat, 450)
        .unwrap();
    let (world, ada) = Universe::with_map(Map::default())
        .with_citizen("Ada", worker)
        .unwrap();
    let (world, mill) = world.with_property(ada, Location::Mill).unwrap();
    let recipe = Recipe::MillFlour;
    let input = recipe
        .inputs()
        .iter()
        .find(|(good, _)| *good == Good::Wheat)
        .unwrap()
        .1;
    let active = world
        .start_action(ada, CitizenAction::Produce(recipe))
        .unwrap();
    assert_eq!(
        active.deposit_goods(ada, mill, Good::Wheat, 451 - input),
        Err(SimulationError::MissingInputs)
    );
    let stored = active
        .deposit_goods(ada, mill, Good::Wheat, 450 - input)
        .unwrap();
    assert_eq!(
        citizen(&stored, ada).active_action(),
        citizen(&active, ada).active_action()
    );
    let finished = stored
        .advance(
            citizen(&stored, ada)
                .active_action()
                .unwrap()
                .remaining_ms(),
        )
        .unwrap();
    assert_eq!(citizen(&finished, ada).units(Good::Wheat), 0);
    assert!(citizen(&finished, ada).units(Good::Flour) > 0);
    assert_eq!(
        finished
            .storage()
            .units(mill, GoodsOwner::Agent(ada), Good::Wheat),
        450 - input
    );
}

#[test]
fn storage_transfers_require_arrival_and_refuse_travel() {
    let map = Map::new(
        Position { x: 100.0, y: 0.0 },
        Position::default(),
        Position::default(),
    )
    .unwrap();
    let forest = map.public_place(Location::Forest);
    let (world, ada) = Universe::with_map(map)
        .with_citizen(
            "Ada",
            Citizen::new(0.0)
                .unwrap()
                .with_good(Good::Bread, 1)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        world.deposit_goods(ada, forest, Good::Bread, 1),
        Err(SimulationError::WrongLocation)
    );
    let moving = world
        .start_action(ada, CitizenAction::Travel(forest))
        .unwrap();
    assert_eq!(
        moving.deposit_goods(ada, forest, Good::Bread, 1),
        Err(SimulationError::CitizenBusy)
    );
}

#[test]
fn stored_owner_totals_update_on_replacement_removal_and_checked_overflow() {
    let world = Universe::with_map(Map::default());
    let forest = world.map().public_place(Location::Forest);
    let river = world.map().public_place(Location::River);
    let full = world
        .with_stored_good(forest, GoodsOwner::Town, Good::Wood, u64::MAX)
        .unwrap();
    assert_eq!(
        full.with_stored_good(river, GoodsOwner::Town, Good::Wood, 1),
        Err(SimulationError::InventoryOverflow)
    );
    let changed = full
        .with_stored_good(forest, GoodsOwner::Town, Good::Wood, 5)
        .unwrap()
        .with_stored_good(river, GoodsOwner::Town, Good::Wood, 2)
        .unwrap();
    assert_eq!(
        changed.storage().owned_units(GoodsOwner::Town, Good::Wood),
        7
    );
    let removed = changed
        .with_stored_good(forest, GoodsOwner::Town, Good::Wood, 0)
        .unwrap();
    assert_eq!(
        removed.storage().owned_units(GoodsOwner::Town, Good::Wood),
        2
    );
    assert_eq!(
        full.storage().owned_units(GoodsOwner::Town, Good::Wood),
        u64::MAX
    );
}
