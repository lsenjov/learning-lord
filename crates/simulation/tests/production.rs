use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::Good;
use learning_lord_simulation::production::{Recipe, Skill, starting_inputs};
use learning_lord_simulation::{
    AgentKind, Citizen, CitizenAction, SimulationError, StartingRole, Universe,
};

fn worker(recipe: Recipe) -> Citizen {
    let role = match recipe {
        Recipe::GrowWheat | Recipe::GrowFlax => StartingRole::Farmer,
        Recipe::MillFlour => StartingRole::Miller,
        Recipe::SpinThread | Recipe::WeaveCloth => StartingRole::Weaver,
        Recipe::MakeClothingBlock | Recipe::AssembleGarment => StartingRole::Tailor,
        Recipe::ChopWood | Recipe::Forage => StartingRole::Woodcutter,
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

trait Numeric {
    fn number(self) -> f64;
}
impl Numeric for f64 {
    fn number(self) -> f64 {
        self
    }
}
impl Numeric for u64 {
    fn number(self) -> f64 {
        self as f64
    }
}
impl Numeric for i64 {
    fn number(self) -> f64 {
        self as f64
    }
}
fn close(a: impl Numeric, b: impl Numeric) {
    let (a, b) = (a.number(), b.number());
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn every_recipe_transforms_exact_inputs_at_completion_and_preserves_partial_stocks() {
    for recipe in Recipe::ALL.into_iter().filter(|r| *r != Recipe::Forage) {
        let source = worker(recipe);
        let started = source.start_action(CitizenAction::Produce(recipe)).unwrap();
        let duration = started.active_action().unwrap().duration_ms();
        let partial = started.advance(duration - 1).unwrap();
        for good in Good::ALL {
            assert_eq!(partial.units(good), source.units(good));
        }
        let complete = partial.advance(1).unwrap();
        for &(good, grams) in recipe.inputs() {
            assert_eq!(source.units(good), grams);
            assert_eq!(complete.units(good), 0);
        }
        for &(good, grams) in recipe.outputs() {
            assert_eq!(complete.units(good), grams);
        }
        assert!(complete.hunger() > source.hunger());
        assert!(complete.tiredness() > source.tiredness());
        if let Some(skill) = recipe.skill() {
            assert_eq!(partial.skill_practice_ms(skill), Some(duration - 1));
            assert_eq!(complete.skill_practice_ms(skill), Some(duration));
            assert_eq!(
                complete.skill_practice_ms(skill),
                started.advance(duration).unwrap().skill_practice_ms(skill)
            );
        }
        let direct = started.advance(duration).unwrap();
        for good in Good::ALL {
            assert_eq!(complete.units(good), direct.units(good));
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
        let short = baker.with_good(good, grams - 1).unwrap();
        assert_eq!(
            short.start_action(CitizenAction::Produce(Recipe::BakeBread)),
            Err(SimulationError::MissingInputs)
        );
        assert_eq!(short.active_action(), None);
    }
    let unskilled = baker.with_skill_practice_ms(Skill::Baking, None).unwrap();
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
        vec![Recipe::Forage, Recipe::FetchWater]
    );
}

#[test]
fn practice_shortens_future_work_and_caps_at_half_duration() {
    let source = worker(Recipe::ChopWood);
    assert_eq!(Recipe::ChopWood.duration_ms(&source), Ok(3_600_000));
    let skilled = source
        .with_skill_practice_ms(
            Skill::Woodcutting,
            Some(learning_lord_simulation::production::MAX_SKILL_PRACTICE_MS),
        )
        .unwrap();
    assert_eq!(Recipe::ChopWood.duration_ms(&skilled), Ok(1_800_000));
    let complete = source
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap()
        .advance(3_600_000)
        .unwrap();
    assert!(Recipe::ChopWood.duration_ms(&complete).unwrap() < 3_600_000);
    assert_eq!(
        source
            .start_action(CitizenAction::Produce(Recipe::ChopWood))
            .unwrap()
            .with_skill_practice_ms(Skill::Woodcutting, Some(5)),
        Err(SimulationError::CitizenBusy)
    );
}

#[test]
fn failed_production_does_not_consume_inputs_or_change_progress() {
    let source = worker(Recipe::BakeBread)
        .with_good(Good::Bread, u64::MAX)
        .unwrap();
    let started = source
        .start_action(CitizenAction::Produce(Recipe::BakeBread))
        .unwrap();
    assert_eq!(
        started.advance(3_600_000),
        Err(SimulationError::InventoryOverflow)
    );
    for &(good, grams) in Recipe::BakeBread.inputs() {
        assert_eq!(started.units(good), grams);
    }
    assert_eq!(started.active_action().unwrap().remaining_ms(), 3_600_000);
    assert_eq!(started.skill_practice_ms(Skill::Baking), Some(0));
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
                4 * grams
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
fn meals_choose_one_available_food_and_keep_other_inventory() {
    let source = Citizen::new(80.0)
        .unwrap()
        .with_berries(40)
        .unwrap()
        .with_good(Good::Bread, 1)
        .unwrap()
        .with_good(Good::BerryPie, 1)
        .unwrap();
    let started = source.start_action(CitizenAction::Eat).unwrap();
    assert_eq!(started.units(Good::Bread), 0);
    assert_eq!(started.units(Good::BerryPie), 1);
    assert_eq!(started.berries_units(), 40);
    let duration = started.active_action().unwrap().duration_ms();
    assert_eq!(duration, 100_000);
    let partial = started.advance(duration / 2).unwrap();
    close(
        partial.hunger(),
        80.0 + duration as f64 / 7_200_000.0 * source.hunger_per_hour() - 25.0,
    );
    let complete = started.advance(duration).unwrap();
    close(
        complete.hunger(),
        80.0 + duration as f64 / 3_600_000.0 * source.hunger_per_hour() - 50.0,
    );
}

#[test]
fn each_food_requires_a_whole_meal_and_nonfood_is_rejected() {
    for (good, units, nutrition) in [
        (Good::Berries, 155, 50.0),
        (Good::Bread, 1, 50.0),
        (Good::BerryPie, 1, 60.0),
    ] {
        let source = Citizen::new(40.0).unwrap().with_good(good, units).unwrap();
        let started = source.start_action(CitizenAction::Eat).unwrap();
        assert_eq!(started.units(good), 0);
        let duration = started.active_action().unwrap().duration_ms();
        let complete = started.advance(duration).unwrap();
        close(
            complete.hunger(),
            40.0 + duration as f64 / 3_600_000.0 * source.hunger_per_hour() - nutrition,
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

#[test]
fn practice_counts_only_elapsed_matching_work_and_keeps_active_duration() {
    let source = worker(Recipe::ChopWood)
        .with_skill_practice_ms(Skill::Baking, Some(123))
        .unwrap();
    let started = source
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap();
    let partial = started.advance(1_234_567).unwrap();
    assert_eq!(
        partial.skill_practice_ms(Skill::Woodcutting),
        Some(1_234_567)
    );
    assert_eq!(partial.skill_practice_ms(Skill::Baking), Some(123));
    assert_eq!(partial.active_action().unwrap().duration_ms(), 3_600_000);
    let complete = partial.advance(3_600_000).unwrap();
    assert_eq!(
        complete.skill_practice_ms(Skill::Woodcutting),
        Some(3_600_000)
    );
    let waiting = complete.start_action(CitizenAction::Wait).unwrap();
    let waited = waiting
        .advance(waiting.active_action().unwrap().duration_ms())
        .unwrap();
    assert_eq!(
        waited.skill_practice_ms(Skill::Woodcutting),
        Some(3_600_000)
    );
    for recipe in [Recipe::Forage, Recipe::FetchWater] {
        let started = source.start_action(CitizenAction::Produce(recipe)).unwrap();
        let complete = started
            .advance(started.active_action().unwrap().duration_ms())
            .unwrap();
        for skill in Skill::ALL {
            assert_eq!(
                complete.skill_practice_ms(skill),
                source.skill_practice_ms(skill)
            );
        }
    }
}

#[test]
fn duration_curve_has_diminishing_benefits_and_practice_saturates() {
    use learning_lord_simulation::production::{MAX_SKILL_PRACTICE_MS, skill_duration_reduction};
    assert_eq!(skill_duration_reduction(0), 0.0);
    assert_eq!(skill_duration_reduction(MAX_SKILL_PRACTICE_MS / 4), 0.21875);
    assert_eq!(skill_duration_reduction(MAX_SKILL_PRACTICE_MS / 2), 0.375);
    assert_eq!(
        skill_duration_reduction(MAX_SKILL_PRACTICE_MS * 3 / 4),
        0.46875
    );
    assert_eq!(skill_duration_reduction(MAX_SKILL_PRACTICE_MS), 0.5);
    assert_eq!(skill_duration_reduction(u64::MAX), 0.5);
    let near_cap = worker(Recipe::ChopWood)
        .with_skill_practice_ms(Skill::Woodcutting, Some(MAX_SKILL_PRACTICE_MS - 1))
        .unwrap();
    let started = near_cap
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap();
    let complete = started
        .advance(started.active_action().unwrap().duration_ms())
        .unwrap();
    assert_eq!(
        complete.skill_practice_ms(Skill::Woodcutting),
        Some(MAX_SKILL_PRACTICE_MS)
    );
    assert_eq!(Recipe::ChopWood.duration_ms(&complete), Ok(1_800_000));
    assert_eq!(
        near_cap
            .with_skill_practice_ms(Skill::Woodcutting, Some(u64::MAX))
            .unwrap()
            .skill_practice_ms(Skill::Woodcutting),
        Some(MAX_SKILL_PRACTICE_MS)
    );
}
