use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, SimulationError, StartingRole, Universe,
    locations::{Location, Map, PlaceId, Position},
    marketplace::Good,
    production::{Recipe, Skill},
    taxation::{TaxKind, TaxRate},
};

fn citizen(world: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen
}

#[test]
fn explicit_production_uses_the_selected_colocated_workshop_and_its_tax_rule() {
    let (world, id) = Universe::with_map(Map::default())
        .with_citizen(
            "Miller",
            Citizen::new(0.0)
                .unwrap()
                .with_starting_role(StartingRole::Miller)
                .with_good(Good::Wheat, 600)
                .unwrap(),
        )
        .unwrap();
    let (world, first) = world.with_property(id, Location::Mill).unwrap();
    let (world, second) = world.with_property(id, Location::Mill).unwrap();
    assert_eq!(world.map().position(first), world.map().position(second));
    let canonical = citizen(&world, id)
        .production_place(Recipe::MillFlour)
        .unwrap();
    let chosen = if canonical == first { second } else { first };
    let (world, _) = world
        .with_tax_rule(
            chosen,
            "Half",
            TaxKind::Socage {
                rates: [(Good::Flour, TaxRate::new(5000).unwrap())]
                    .into_iter()
                    .collect(),
            },
        )
        .unwrap();
    let worker = citizen(&world, id);
    assert_eq!(
        worker
            .production_socage_rate(Recipe::MillFlour, Good::Flour)
            .basis_points(),
        0
    );
    assert_eq!(
        worker
            .production_socage_rate_at(chosen, Recipe::MillFlour, Good::Flour)
            .basis_points(),
        5000
    );
    let action = CitizenAction::ProduceAt {
        place: chosen,
        recipe: Recipe::MillFlour,
    };
    assert_eq!(action.required_place(worker), Some(chosen));
    let world = world
        .start_action(id, action)
        .unwrap()
        .advance(60 * 60_000)
        .unwrap();
    let worker = citizen(&world, id);
    assert_eq!(worker.units(Good::Wheat), 300);
    assert_eq!(worker.units(Good::Flour), 0);
    assert_eq!(
        world
            .storage()
            .units(chosen, worker.goods_owner(), Good::Flour),
        240
    );
    assert_eq!(worker.tax_reserved_units(Good::Flour), 0);
    assert_eq!(
        world
            .storage()
            .reserved_units(chosen, worker.goods_owner(), Good::Flour),
        120
    );
    assert_eq!(worker.skill_practice_ms(Skill::Milling), Some(60 * 60_000));
    let history = &world.citizen_history(id).unwrap().current;
    assert_eq!(history.activity.production_ms, 60 * 60_000);
    assert_eq!(history.produced[Good::Flour as usize], 240);
    assert_eq!(history.consumed[Good::Wheat as usize], 300);
}

#[test]
fn explicit_production_rejects_private_mismatched_and_unknown_sites() {
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Miller)
        .with_good(Good::Wheat, 300)
        .unwrap();
    let (map, own) = worker
        .map()
        .with_place(
            Location::Mill,
            Position::default(),
            Some(worker.id()),
            "Own mill",
        )
        .unwrap();
    let (map, private) = map
        .with_place(
            Location::Mill,
            Position::default(),
            Some(AgentId(uuid::Uuid::new_v4())),
            "Other mill",
        )
        .unwrap();
    let worker = worker.with_map(map).unwrap();
    for place in [private, worker.map().public_place(Location::Forest)] {
        assert!(matches!(
            worker.start_action(CitizenAction::ProduceAt {
                place,
                recipe: Recipe::MillFlour
            }),
            Err(SimulationError::MissingProperty)
        ));
    }
    let unknown = PlaceId(uuid::Uuid::new_v4());
    assert!(matches!(
        worker.start_action(CitizenAction::ProduceAt {
            place: unknown,
            recipe: Recipe::MillFlour
        }),
        Err(SimulationError::PlaceNotFound)
    ));
    let unskilled = worker.with_skill_practice_ms(Skill::Milling, None).unwrap();
    assert!(matches!(
        unskilled.start_action(CitizenAction::ProduceAt {
            place: own,
            recipe: Recipe::MillFlour
        }),
        Err(SimulationError::MissingSkill)
    ));
    let empty = worker.with_good(Good::Wheat, 0).unwrap();
    assert!(matches!(
        empty.start_action(CitizenAction::ProduceAt {
            place: own,
            recipe: Recipe::MillFlour
        }),
        Err(SimulationError::MissingInputs)
    ));
}

#[test]
fn explicit_production_requires_arrival_and_preserves_legacy_actions() {
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Miller)
        .with_good(Good::Wheat, 300)
        .unwrap();
    let (map, place) = worker
        .map()
        .with_place(
            Location::Mill,
            Position { x: 1.0, y: 0.0 },
            Some(worker.id()),
            "Remote mill",
        )
        .unwrap();
    let worker = worker.with_map(map).unwrap();
    let action = CitizenAction::ProduceAt {
        place,
        recipe: Recipe::MillFlour,
    };
    assert!(matches!(
        worker.start_action(action),
        Err(SimulationError::WrongLocation)
    ));
    let arrived = worker
        .start_action(CitizenAction::Travel(place))
        .unwrap()
        .advance(600)
        .unwrap();
    assert!(arrived.start_action(action).is_ok());
    assert!(
        arrived
            .start_action(CitizenAction::Produce(Recipe::MillFlour))
            .is_ok()
    );
}
