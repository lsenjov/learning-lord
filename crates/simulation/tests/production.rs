use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::Good;
use learning_lord_simulation::production::{Recipe, Skill, starting_inputs};
use learning_lord_simulation::{
    AgentKind, Citizen, CitizenAction, SimulationError, StartingRole, Universe,
};

fn worker(recipe: Recipe) -> Citizen {
    let role = match recipe {
        Recipe::GrowWheat => StartingRole::Farmer,
        Recipe::MillFlour => StartingRole::Miller,
        Recipe::ChopWood => StartingRole::Woodcutter,
        Recipe::FetchWater | Recipe::BakeBread | Recipe::BakeBerryPie => StartingRole::Baker,
    };
    let mut citizen = Citizen::new(0.0).unwrap().with_starting_role(role);
    for &(good, grams) in recipe.inputs() {
        citizen = citizen.with_good(good, grams).unwrap();
    }
    let (mut universe, id) = Universe::with_map(Map::default())
        .with_citizen("Worker", citizen)
        .unwrap();
    if !recipe.location().is_public() {
        universe = universe.with_property(id, recipe.location()).unwrap().0;
    }
    let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
    citizen.clone()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn every_recipe_transforms_exact_inputs_at_completion_and_preserves_partial_stocks() {
    for recipe in Recipe::ALL {
        let source = worker(recipe);
        let started = source.start_action(CitizenAction::Produce(recipe)).unwrap();
        let duration = started.active_action().unwrap().duration_ms();
        let partial = started.advance(duration - 1).unwrap();
        for good in Good::ALL {
            assert_eq!(partial.grams(good), source.grams(good));
        }
        let complete = partial.advance(1).unwrap();
        for &(good, grams) in recipe.inputs() {
            assert_eq!(source.grams(good), grams);
            assert_eq!(complete.grams(good), 0.0);
        }
        for &(good, grams) in recipe.outputs() {
            assert_eq!(complete.grams(good), grams);
        }
        assert!(complete.hunger() > source.hunger());
        assert!(complete.tiredness() > source.tiredness());
        if let Some(skill) = recipe.skill() {
            close(partial.skill_level(skill), 1.0);
            close(complete.skill_level(skill), 1.1);
        }
        let direct = started.advance(duration).unwrap();
        for good in Good::ALL {
            assert_eq!(complete.grams(good), direct.grams(good));
        }
        close(complete.hunger(), direct.hunger());
        close(complete.tiredness(), direct.tiredness());
        assert_eq!(source.active_action(), None);
    }
}

#[test]
fn all_inputs_skill_and_owned_property_are_required_before_work_starts() {
    let baker = worker(Recipe::BakeBread);
    for &(good, grams) in Recipe::BakeBread.inputs() {
        let short = baker.with_good(good, grams - 1.0).unwrap();
        assert_eq!(
            short.start_action(CitizenAction::Produce(Recipe::BakeBread)),
            Err(SimulationError::MissingInputs)
        );
        assert_eq!(short.active_action(), None);
    }
    let unskilled = baker.with_skill(Skill::Baking, 0.0).unwrap();
    assert_eq!(
        unskilled.start_action(CitizenAction::Produce(Recipe::BakeBread)),
        Err(SimulationError::MissingSkill)
    );
    let farmer = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Farmer);
    assert_eq!(
        farmer.start_action(CitizenAction::Produce(Recipe::GrowWheat)),
        Err(SimulationError::MissingProperty)
    );
    let public = Citizen::new(0.0).unwrap();
    assert!(
        public
            .start_action(CitizenAction::Produce(Recipe::FetchWater))
            .is_ok()
    );
    assert_eq!(
        public.available_recipes().collect::<Vec<_>>(),
        vec![Recipe::FetchWater]
    );
}

#[test]
fn skill_levels_shorten_work_and_completion_improves_future_batches() {
    let source = worker(Recipe::ChopWood);
    assert_eq!(Recipe::ChopWood.duration_ms(&source), Ok(3_600_000));
    let skilled = source.with_skill(Skill::Woodcutting, 11.0).unwrap();
    assert_eq!(Recipe::ChopWood.duration_ms(&skilled), Ok(2_400_000));
    let complete = source
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap()
        .advance(3_600_000)
        .unwrap();
    assert!(Recipe::ChopWood.duration_ms(&complete).unwrap() < 3_600_000);
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            source.with_skill(Skill::Woodcutting, invalid),
            Err(SimulationError::InvalidSkill)
        );
    }
    assert_eq!(
        source
            .start_action(CitizenAction::Produce(Recipe::ChopWood))
            .unwrap()
            .with_skill(Skill::Woodcutting, 5.0),
        Err(SimulationError::CitizenBusy)
    );
}

#[test]
fn failed_production_does_not_consume_inputs_or_change_progress() {
    let source = worker(Recipe::BakeBread)
        .with_good(Good::Bread, f64::MAX)
        .unwrap();
    let started = source
        .start_action(CitizenAction::Produce(Recipe::BakeBread))
        .unwrap();
    assert_eq!(
        started.advance(3_600_000),
        Err(SimulationError::InventoryOverflow)
    );
    for &(good, grams) in Recipe::BakeBread.inputs() {
        assert_eq!(started.grams(good), grams);
    }
    assert_eq!(started.active_action().unwrap().remaining_ms(), 3_600_000);
    assert_eq!(started.skill_level(Skill::Baking), 1.0);
}

#[test]
fn starting_inputs_cover_four_baseline_hours_of_the_selected_profession() {
    for (role, recipe) in [
        (StartingRole::Miller, Recipe::MillFlour),
        (StartingRole::Baker, Recipe::BakeBread),
    ] {
        let inputs = starting_inputs(role).items().collect::<Vec<_>>();
        for &(good, grams) in recipe.inputs() {
            assert_eq!(
                inputs.iter().find(|(g, _)| *g == good).unwrap().1,
                4.0 * grams
            );
        }
        assert_eq!(inputs.len(), recipe.inputs().len());
    }
    assert!(
        starting_inputs(StartingRole::Farmer)
            .items()
            .next()
            .is_none()
    );
    assert!(
        starting_inputs(StartingRole::Woodcutter)
            .items()
            .next()
            .is_none()
    );
}

#[test]
fn mixed_food_meals_consume_and_restore_nutrition_continuously() {
    let source = Citizen::new(80.0)
        .unwrap()
        .with_berries(40.0)
        .unwrap()
        .with_good(Good::Bread, 40.0)
        .unwrap()
        .with_good(Good::BerryPie, 50.0)
        .unwrap();
    let total_nutrition = 40.0 * Good::Berries.nutrition_per_gram().unwrap() + 20.0 + 30.0;
    close(source.food_nutrition(), total_nutrition);
    let started = source.start_action(CitizenAction::Eat).unwrap();
    let duration = started.active_action().unwrap().duration_ms();
    assert_eq!(duration, 108_495);
    let partial = started.advance(duration / 2).unwrap();
    let fraction = (duration / 2) as f64 / duration as f64;
    close(partial.berries_grams(), 40.0 * (1.0 - fraction));
    close(partial.grams(Good::Bread), 40.0 * (1.0 - fraction));
    close(
        partial.hunger(),
        80.0 + (duration / 2) as f64 / 3_600_000.0 * source.hunger_per_hour() - 50.0 * fraction,
    );
    let complete = started.advance(duration).unwrap();
    assert_eq!(complete.berries_grams(), 0.0);
    assert_eq!(complete.grams(Good::Bread), 0.0);
    close(complete.food_nutrition(), total_nutrition - 50.0);
    close(
        complete.hunger(),
        80.0 + duration as f64 / 3_600_000.0 * source.hunger_per_hour() - 50.0,
    );
    assert_eq!(source.grams(Good::BerryPie), 50.0);
}

#[test]
fn each_edible_good_supports_partial_meals_and_nonfood_cannot_be_requested_as_food() {
    for good in Good::FOOD {
        let grams = 30.0 / good.nutrition_per_gram().unwrap();
        let source = Citizen::new(40.0).unwrap().with_good(good, grams).unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        let duration = started.active_action().unwrap().duration_ms();
        let complete = started.advance(duration).unwrap();
        close(complete.grams(good), 0.0);
        close(
            complete.hunger(),
            40.0 + duration as f64 / 3_600_000.0 * source.hunger_per_hour() - 30.0,
        );
    }
    assert_eq!(
        Citizen::new(0.0)
            .unwrap()
            .start_action(CitizenAction::BuyFood(Good::Wheat)),
        Err(SimulationError::NotFood)
    );
    assert!(
        CitizenAction::BuyFood(Good::Wheat)
            .input_for(&Citizen::new(0.0).unwrap(), 30.0)
            .is_none()
    );
}
