use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, StartingRole, Universe,
    farming::{CropKind, CropStatus, HOUR_MS},
    locations::{Location, Map, PlaceId},
    marketplace::Good,
    taxation::{TaxKind, TaxRate},
};

fn citizen(world: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen
}

fn farmer(world: Universe) -> (Universe, AgentId, PlaceId) {
    let (world, id) = world
        .with_citizen(
            "Farmer",
            Citizen::new(0.0)
                .unwrap()
                .with_starting_role(StartingRole::Farmer)
                .with_coins(100)
                .unwrap(),
        )
        .unwrap();
    let (world, field) = world.with_property(id, Location::Field).unwrap();
    (world, id, field)
}

fn perform(world: Universe, id: AgentId, action: CitizenAction) -> Universe {
    let world = world.start_action(id, action).unwrap();
    world
        .advance(citizen(&world, id).active_action().unwrap().remaining_ms())
        .unwrap()
}

#[test]
fn multiple_farmers_and_fields_keep_independent_plots_through_elapsed_jumps() {
    let (world, first, field) = farmer(Universe::with_map(Map::default()));
    let (world, second_field) = world.with_property(first, Location::Field).unwrap();
    let (world, second, other_field) = farmer(world);
    let world = world
        .start_action(
            first,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Wheat,
            },
        )
        .unwrap()
        .start_action(
            second,
            CitizenAction::Plant {
                field: other_field,
                plot: 0,
                crop: CropKind::Flax,
            },
        )
        .unwrap()
        .advance(HOUR_MS)
        .unwrap();
    let world = perform(
        world,
        first,
        CitizenAction::Plant {
            field: second_field,
            plot: 5,
            crop: CropKind::Flax,
        },
    );
    assert_eq!(
        world.map().place(field).unwrap().plots[0].unwrap().kind,
        CropKind::Wheat
    );
    assert_eq!(
        world.map().place(other_field).unwrap().plots[0]
            .unwrap()
            .kind,
        CropKind::Flax
    );
    assert!(world.map().place(second_field).unwrap().plots[5].is_some());
    let one_jump = world.advance(80 * HOUR_MS).unwrap();
    let split = world
        .advance(30 * HOUR_MS)
        .unwrap()
        .advance(50 * HOUR_MS)
        .unwrap();
    assert_eq!(one_jump.map(), split.map());
    assert_eq!(citizen(&one_jump, first).map(), one_jump.map());
    assert_eq!(citizen(&one_jump, second).map(), one_jump.map());
    for field in [field, other_field, second_field] {
        for crop in one_jump.map().place(field).unwrap().plots.iter().flatten() {
            assert_eq!(crop.status(one_jump.current_time_ms()), CropStatus::Dead);
        }
    }
}

#[test]
fn harvest_reserves_socage_on_actual_yield_and_records_crop_labor_and_output() {
    for both_tends in [false, true] {
        let (world, id, field) = farmer(Universe::with_map(Map::default()));
        let (world, _) = world
            .with_tax_rule(
                field,
                "Crop levy",
                TaxKind::Socage {
                    rates: [(Good::Wheat, TaxRate::new(2500).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let world = perform(
            world,
            id,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Wheat,
            },
        );
        let planted_at = world.map().place(field).unwrap().plots[0]
            .unwrap()
            .planted_at_ms;
        assert_eq!(
            world
                .storage()
                .reserved_units(field, citizen(&world, id).goods_owner(), Good::Wheat),
            0
        );
        let world = world.advance(24 * HOUR_MS).unwrap();
        let mut world = perform(
            world,
            id,
            CitizenAction::Tend {
                field,
                plot: 0,
                stage: 0,
            },
        );
        if both_tends {
            world = world
                .advance(planted_at + 48 * HOUR_MS - world.current_time_ms())
                .unwrap();
            world = perform(
                world,
                id,
                CitizenAction::Tend {
                    field,
                    plot: 0,
                    stage: 1,
                },
            );
        }
        world = world
            .advance(planted_at + 96 * HOUR_MS - world.current_time_ms())
            .unwrap();
        let before = citizen(&world, id).clone();
        let action = CitizenAction::Harvest { field, plot: 0 };
        let predicted = before.start_action(action).unwrap();
        let predicted = predicted
            .advance(predicted.active_action().unwrap().remaining_ms())
            .unwrap();
        let world = perform(world, id, action);
        let expected = if both_tends { 600 } else { 375 };
        assert_eq!(citizen(&world, id).units(Good::Wheat), 0);
        assert_eq!(
            world
                .storage()
                .units(field, citizen(&world, id).goods_owner(), Good::Wheat),
            expected
        );
        assert_eq!(
            world
                .storage()
                .reserved_units(field, citizen(&world, id).goods_owner(), Good::Wheat),
            expected / 4
        );
        assert_eq!(
            predicted
                .storage()
                .units(field, predicted.goods_owner(), Good::Wheat),
            expected
        );
        assert_eq!(predicted.crop(field, 0).unwrap(), None);
        assert_eq!(world.map().place(field).unwrap().plots[0], None);
        let history = world.citizen_history(id).unwrap();
        assert_eq!(history.current.produced[Good::Wheat as usize], expected);
        assert!(history.current.activity.production_ms > 0);
        assert!(citizen(&world, id).wealth().unwrap() > before.wealth().unwrap());
    }
}

#[test]
fn weekly_assets_exclude_standing_crops() {
    let settlement = learning_lord_simulation::calendar::FIRST_WEEKLY_SETTLEMENT_MS;
    let (world, id, field) = farmer(
        Universe::with_map(Map::default())
            .advance(settlement - 48 * HOUR_MS)
            .unwrap(),
    );
    let (world, _) = world
        .with_tax_rule(
            field,
            "Crop assets",
            TaxKind::Asset {
                rates: [(Good::Wheat, TaxRate::new(10000).unwrap())]
                    .into_iter()
                    .collect(),
            },
        )
        .unwrap();
    let world = perform(
        world,
        id,
        CitizenAction::Plant {
            field,
            plot: 0,
            crop: CropKind::Wheat,
        },
    );
    let world = world.advance(24 * HOUR_MS).unwrap();
    let world = perform(
        world,
        id,
        CitizenAction::Tend {
            field,
            plot: 0,
            stage: 0,
        },
    );
    let world = world.advance(settlement - world.current_time_ms()).unwrap();
    assert!(citizen(&world, id).crop(field, 0).unwrap().is_some());
    assert!(citizen(&world, id).wealth().unwrap() > 100.0);
    assert_eq!(citizen(&world, id).coins(), 100);
    assert_eq!(world.town_treasury(), 0);
}
