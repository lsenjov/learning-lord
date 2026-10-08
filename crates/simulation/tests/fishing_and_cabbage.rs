use learning_lord_simulation::{
    Citizen, CitizenAction, SimulationError, StartingRole,
    farming::{CropKind, CropStatus, GROWTH_MS, HOUR_MS},
    locations::{Location, Position},
    marketplace::Good,
    production::{MAX_SKILL_PRACTICE_MS, Recipe, Skill},
};

fn perform(citizen: Citizen, action: CitizenAction) -> Citizen {
    let duration = citizen.action_duration_ms(action).unwrap();
    let started = citizen.start_action(action).unwrap();
    started.advance(duration).unwrap()
}

#[test]
fn fishing_requires_training_and_produces_one_whole_fish_with_practice() {
    let action = CitizenAction::Produce(Recipe::CatchFish);
    let untrained = Citizen::new(0.0).unwrap();
    assert_eq!(
        untrained.action_duration_ms(action),
        Err(SimulationError::MissingSkill)
    );
    let fisher = untrained.with_starting_role(StartingRole::Fisher);
    assert_eq!(fisher.action_duration_ms(action).unwrap(), HOUR_MS / 2);
    let river = fisher.map().public_place(Location::River);
    assert_eq!(action.required_place(&fisher), Some(river));
    assert_eq!(
        fisher.selling_place(),
        fisher.map().public_place(Location::Market)
    );
    let arrived = perform(fisher, CitizenAction::Travel(river));
    let completed = perform(arrived, action);
    assert_eq!(completed.units(Good::Fish), 1);
    assert_eq!(
        completed.skill_practice_ms(Skill::Fishing),
        Some(HOUR_MS / 2)
    );
    let expert = completed
        .with_skill_practice_ms(Skill::Fishing, Some(MAX_SKILL_PRACTICE_MS))
        .unwrap();
    assert_eq!(expert.action_duration_ms(action).unwrap(), HOUR_MS / 4);
    assert_eq!(perform(expert, action).units(Good::Fish), 2);
    assert_eq!(Good::Fish.weight_grams(), 250);
    assert_eq!(Good::Fish.core_price(), 6.5);
    assert_eq!(Good::Fish.units_per_price_unit(), 1);
}

#[test]
fn cabbage_is_harvested_as_whole_heads_and_obeys_care_windows() {
    for care in 0..=2 {
        let farmer = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Farmer);
        let (map, field) = farmer
            .map()
            .with_place(
                Location::Field,
                Position::default(),
                Some(farmer.id()),
                "Field",
            )
            .unwrap();
        let mut farmer = perform(
            farmer.with_map(map).unwrap(),
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Cabbage,
            },
        );
        let planted = farmer.crop(field, 0).unwrap().unwrap().planted_at_ms;
        for stage in 0..care {
            farmer = farmer
                .advance(planted + (24 + stage as u64 * 24) * HOUR_MS - farmer.current_time_ms())
                .unwrap();
            farmer = perform(
                farmer,
                CitizenAction::Tend {
                    field,
                    plot: 0,
                    stage,
                },
            );
        }
        farmer = farmer
            .advance(planted + GROWTH_MS - farmer.current_time_ms())
            .unwrap();
        let crop = farmer.crop(field, 0).unwrap().unwrap();
        if care == 0 {
            assert_eq!(crop.status(farmer.current_time_ms()), CropStatus::Dead);
            assert_eq!(crop.expected_yield(farmer.current_time_ms()), 0);
            assert!(
                farmer
                    .start_action(CitizenAction::Harvest { field, plot: 0 })
                    .is_err()
            );
        } else {
            farmer = perform(farmer, CitizenAction::Harvest { field, plot: 0 });
            assert_eq!(farmer.units(Good::Cabbage), if care == 1 { 5 } else { 8 });
            assert_eq!(farmer.crop(field, 0).unwrap(), None);
        }
        assert!(
            farmer
                .start_action(CitizenAction::Produce(Recipe::GrowCabbage))
                .is_err()
        );
    }
    assert_eq!(Good::Cabbage.weight_grams(), 500);
    assert_eq!(Good::Cabbage.core_price(), 39.0 / 8.0);
    assert_eq!(Good::Cabbage.units_per_price_unit(), 1);
}
