use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::{DAY_MS, Good, ShoppingList};
use learning_lord_simulation::production::{
    DAILY_CAPACITY_MS, ProductionTargets, Recipe, Skill, recipe_profit,
};
use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, StartingRole, TRADE_DURATION_MS, Universe,
};

fn citizen(universe: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
    citizen
}

fn producer(
    universe: Universe,
    name: &str,
    role: StartingRole,
    stocks: &[(Good, u64)],
    coins: i64,
) -> (Universe, AgentId) {
    let mut worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(role)
        .with_coins(coins)
        .unwrap();
    for &(good, grams) in stocks {
        worker = worker.with_good(good, grams).unwrap();
    }
    let recipe = match role {
        StartingRole::Farmer => Recipe::GrowWheat,
        StartingRole::Miller => Recipe::MillFlour,
        StartingRole::Woodcutter => Recipe::ChopWood,
        StartingRole::Baker => Recipe::BakeBread,
    };
    let (universe, id) = universe.with_citizen(name, worker).unwrap();
    (universe.with_property(id, recipe.location()).unwrap().0, id)
}

fn targets(universe: &Universe, id: AgentId) -> ProductionTargets {
    ProductionTargets::calculate(citizen(universe, id), universe.current_time_ms()).unwrap()
}

fn demand(universe: Universe, goods: &[(Good, u64)]) -> Universe {
    let (universe, buyer) = universe
        .with_citizen(
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(10000).unwrap(),
        )
        .unwrap();
    universe
        .with_purchase_request(buyer, ShoppingList::new(goods.iter().copied()).unwrap())
        .unwrap()
}

fn list(universe: Universe, id: AgentId, good: Good, grams: u64) -> Universe {
    universe
        .start_action(id, CitizenAction::List(good, grams))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap()
}

#[test]
fn competing_bakers_each_consider_the_full_unmet_demand() {
    let inputs = [
        (Good::Berries, 930),
        (Good::Bread, 8),
        (Good::Flour, 2_000),
        (Good::Wood, 500),
        (Good::Water, 2_000),
        (Good::BerryPie, 16),
    ];
    let (universe, first) = producer(
        Universe::with_map(Map::default()),
        "First",
        StartingRole::Baker,
        &inputs,
        0,
    );
    let (universe, second) = producer(universe, "Second", StartingRole::Baker, &inputs, 0);
    let universe = demand(universe, &[(Good::Bread, 28)]);
    assert_eq!(universe.market().unmet_units(Good::Bread), 28);
    for id in [first, second] {
        assert_eq!(
            targets(&universe, id).remaining_batches(Recipe::BakeBread),
            10
        );
    }
}

#[test]
fn carried_and_listed_stock_reduce_the_daily_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 400)],
        0,
    );
    assert_eq!(
        targets(&universe, farmer).remaining_batches(Recipe::GrowWheat),
        10
    );
    let fully_stocked = citizen(&universe, farmer)
        .with_good(Good::Wheat, 2_400)
        .unwrap();
    assert_eq!(
        ProductionTargets::calculate(&fully_stocked, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        0
    );
    let listed = list(universe, farmer, Good::Wheat, 200);
    assert_eq!(citizen(&listed, farmer).units(Good::Wheat), 200);
    assert_eq!(
        targets(&listed, farmer).remaining_batches(Recipe::GrowWheat),
        10
    );
    let withdrawn = listed
        .start_action(farmer, CitizenAction::Withdraw(Good::Wheat, 200))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(
        targets(&withdrawn, farmer).remaining_batches(Recipe::GrowWheat),
        10
    );
}

#[test]
fn skill_adjusted_floor_subtracts_adas_existing_listing() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Ada",
        StartingRole::Farmer,
        &[(Good::Wheat, 200)],
        0,
    );
    let universe = list(universe, farmer, Good::Wheat, 200);
    let worker = citizen(&universe, farmer)
        .with_skill(Skill::Farming, 1.1)
        .unwrap();
    assert_eq!(Recipe::GrowWheat.duration_ms(&worker).unwrap(), 3_582_090);
    assert_eq!(
        ProductionTargets::calculate(&worker, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        11
    );
}

#[test]
fn completed_work_counts_as_stock_without_shrinking_the_stock_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 2_000)],
        0,
    );
    let working = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::GrowWheat))
        .unwrap();
    let finished = working
        .advance(working.active_action().unwrap().duration_ms())
        .unwrap();
    assert_eq!(finished.production_work_today_ms(), 60 * 60_000);
    assert_eq!(finished.units(Good::Wheat), 2_200);
    assert_eq!(
        ProductionTargets::calculate(&finished, 60 * 60_000)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        1
    );
}

#[test]
fn in_progress_output_counts_toward_the_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 2_000)],
        0,
    );
    let working = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::GrowWheat))
        .unwrap();
    let target = ProductionTargets::calculate(&working, 0).unwrap();
    assert_eq!(target.remaining_batches(Recipe::GrowWheat), 2);
}

#[test]
fn each_profitable_good_has_an_independent_twelve_hour_floor() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 3_000),
            (Good::Flour, 3_000),
            (Good::Wood, 750),
            (Good::Water, 3_000),
            (Good::BerryPie, 8),
        ],
        0,
    );
    let target = targets(&universe, baker);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 4);
    assert_eq!(target.remaining_batches(Recipe::BakeBread), 12);
    let worker = citizen(&universe, baker);
    let work: u64 = target
        .batches()
        .filter(|(recipe, _)| matches!(recipe, Recipe::BakeBread | Recipe::BakeBerryPie))
        .map(|(recipe, count)| count * recipe.duration_ms(worker).unwrap())
        .sum();
    assert_eq!(work, 18 * 60 * 60_000);
    let unstocked = worker.with_good(Good::BerryPie, 0).unwrap();
    let full = ProductionTargets::calculate(&unstocked, 0).unwrap();
    assert_eq!(full.remaining_batches(Recipe::BakeBerryPie), 8);
    assert_eq!(full.remaining_batches(Recipe::BakeBread), 12);
}

#[test]
fn low_positive_demand_keeps_the_daily_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[],
        0,
    );
    assert_eq!(
        targets(&universe, farmer).remaining_batches(Recipe::GrowWheat),
        12
    );
    let universe = demand(universe, &[(Good::Wheat, 200)]);
    assert_eq!(
        targets(&universe, farmer).remaining_batches(Recipe::GrowWheat),
        12
    );
}

#[test]
fn alternative_recipes_each_use_the_same_inputs_and_cash() {
    for (flour, wood, water, coins) in [(100, 25, 100, 0), (0, 0, 0, 13)] {
        let (universe, baker) = producer(
            Universe::with_map(Map::default()),
            "Baker",
            StartingRole::Baker,
            &[
                (Good::Berries, 510),
                (Good::Flour, flour),
                (Good::Wood, wood),
                (Good::Water, water),
            ],
            coins,
        );
        let target = targets(&universe, baker);
        let bread = target.remaining_batches(Recipe::BakeBread);
        let pie = target.remaining_batches(Recipe::BakeBerryPie);
        assert_eq!(bread, 1);
        assert_eq!(pie, 1);
    }
}

#[test]
fn missing_skill_or_property_prevents_targets_and_faster_skill_increases_capacity() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Farmer);
    assert_eq!(
        ProductionTargets::calculate(&source, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        0
    );
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[],
        0,
    );
    let worker = citizen(&universe, farmer);
    let unskilled = worker.with_skill(Skill::Farming, 0.0).unwrap();
    assert_eq!(
        ProductionTargets::calculate(&unskilled, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        0
    );
    let normal = targets(&universe, farmer).remaining_batches(Recipe::GrowWheat);
    let skilled = worker.with_skill(Skill::Farming, 11.0).unwrap();
    let faster = ProductionTargets::calculate(&skilled, 0)
        .unwrap()
        .remaining_batches(Recipe::GrowWheat);
    assert_eq!(normal, 12);
    assert_eq!(faster, 18);
    assert!(faster * Recipe::GrowWheat.duration_ms(&skilled).unwrap() <= DAILY_CAPACITY_MS);
}

#[test]
fn active_batch_uses_inputs_once_and_subtracts_its_output_from_the_target() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 930),
            (Good::Flour, 200),
            (Good::Wood, 50),
            (Good::Water, 200),
            (Good::BerryPie, 16),
        ],
        0,
    );
    let universe = demand(universe, &[(Good::Bread, 4)]);
    let working = citizen(&universe, baker)
        .start_action(CitizenAction::Produce(Recipe::BakeBread))
        .unwrap();
    let target = ProductionTargets::calculate(&working, 0).unwrap();
    assert_eq!(target.remaining_batches(Recipe::BakeBread), 2);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 0);
}

#[test]
fn personal_food_reserve_is_shared_across_edible_goods_and_does_not_block_eating() {
    let worker = Citizen::new(80.0)
        .unwrap()
        .with_good(Good::Berries, 124)
        .unwrap()
        .with_good(Good::Bread, 6)
        .unwrap()
        .with_good(Good::BerryPie, 1)
        .unwrap();
    let reserves = worker.reserved_goods().unwrap();
    let nutrition: f64 = Good::FOOD
        .into_iter()
        .map(|good| reserves.units(good) as f64 * good.nutrition_per_unit().unwrap())
        .sum();
    assert_eq!(nutrition, 340.0);
    assert_eq!(reserves.units(Good::BerryPie), 0);
    let excess = worker.excess_goods().unwrap();
    let excess_nutrition: f64 = Good::FOOD
        .into_iter()
        .map(|good| excess.units(good) as f64 * good.nutrition_per_unit().unwrap())
        .sum();
    assert_eq!(excess_nutrition, 60.0);
    assert_eq!(nutrition + excess_nutrition, worker.food_nutrition());
    let eating = worker.start_action(CitizenAction::Eat).unwrap();
    let eaten = eating
        .advance(eating.active_action().unwrap().duration_ms())
        .unwrap();
    assert!(eaten.food_nutrition() < worker.food_nutrition());
}

#[test]
fn replacement_asks_can_make_a_recipe_unprofitable_even_when_inputs_are_owned() {
    let (universe, seller) = producer(
        Universe::with_map(Map::default()),
        "Seller",
        StartingRole::Miller,
        &[(Good::Flour, 1_000)],
        0,
    );
    let expensive =
        universe.with_prices(universe.prices().with_price(Good::Flour, 1000.0).unwrap());
    let listed = list(expensive, seller, Good::Flour, 1_000);
    let listed = listed.with_prices(Universe::with_map(Map::default()).prices());
    let (universe, baker) = producer(
        listed,
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 310),
            (Good::Flour, 100),
            (Good::Wood, 25),
            (Good::Water, 100),
        ],
        0,
    );
    assert!(recipe_profit(Recipe::BakeBread, citizen(&universe, baker)).unwrap() < 0.0);
    assert_eq!(
        targets(&universe, baker).remaining_batches(Recipe::BakeBread),
        0
    );
    assert_eq!(
        targets(&universe, baker).remaining_batches(Recipe::BakeBerryPie),
        0
    );
}

#[test]
fn own_sales_and_backlog_add_and_inactive_days_reduce_the_sales_rate() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 2_800)],
        0,
    );
    let universe = list(universe, farmer, Good::Wheat, 2_800);
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(200).unwrap())
        .unwrap();
    let universe = universe
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Wheat, 2_800)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let universe = universe
        .with_purchase_request(buyer, ShoppingList::single(Good::Wheat, 200))
        .unwrap();
    assert_eq!(universe.market().unmet_units(Good::Wheat), 200);
    let worker = citizen(&universe, farmer)
        .with_good(Good::Wheat, 800)
        .unwrap();
    assert_eq!(
        ProductionTargets::calculate(&worker, universe.current_time_ms())
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        11
    );
    let later = ProductionTargets::calculate(&worker, 2 * DAY_MS).unwrap();
    assert_eq!(later.remaining_batches(Recipe::GrowWheat), 8);
}

#[test]
fn completed_and_active_work_do_not_shrink_alternative_capacity() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[],
        0,
    );
    let universe = demand(universe, &[(Good::Wheat, 20_000)]);
    let worker = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::GrowWheat))
        .unwrap();
    let done = worker
        .advance(worker.active_action().unwrap().duration_ms())
        .unwrap();
    let second = done
        .start_action(CitizenAction::Produce(Recipe::GrowWheat))
        .unwrap();
    let partial = second.advance(20 * 60_000).unwrap();
    assert_eq!(partial.production_work_today_ms(), 80 * 60_000);
    let daily_batches = DAILY_CAPACITY_MS / Recipe::GrowWheat.duration_ms(&partial).unwrap();
    let target = ProductionTargets::calculate(&partial, 80 * 60_000).unwrap();
    assert_eq!(
        target.remaining_batches(Recipe::GrowWheat),
        daily_batches + 1
    );
}

#[test]
fn remaining_recipe_inputs_and_personal_food_use_separate_portions_of_inventory() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 1130),
            (Good::Flour, 200),
            (Good::Wood, 50),
            (Good::Water, 150),
            (Good::Bread, 22),
            (Good::BerryPie, 15),
        ],
        0,
    );
    let planned = universe.start_planning(baker).unwrap();
    let worker = citizen(&planned, baker);
    let target = worker.production_targets().unwrap();
    assert_eq!(target.remaining_batches(Recipe::BakeBread), 1);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 1);
    let reserves = worker.reserved_goods().unwrap();
    assert_eq!(reserves.units(Good::Flour), 100);
    assert_eq!(reserves.units(Good::Wood), 25);
    assert_eq!(reserves.units(Good::Water), 100);
    assert_eq!(reserves.units(Good::Berries), 1030);
    assert_eq!(worker.excess_goods().unwrap().units(Good::Berries), 100);
    assert_eq!(citizen(&universe, baker).production_targets(), None);
    assert_eq!(citizen(&universe, baker).units(Good::Berries), 1130);
}

#[test]
fn decision_snapshots_inputs_targets_and_observed_production() {
    let (universe, id) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Berries, 930)],
        0,
    );
    let universe = universe.start_planning(id).unwrap();
    let worker = citizen(&universe, id);
    let decision = worker.active_plan().unwrap().plan().decision().unwrap();
    let wheat = decision
        .production
        .iter()
        .find(|d| d.recipe == Recipe::GrowWheat)
        .unwrap();
    assert!(wheat.profit_per_hour.unwrap() > 0.0);
    assert!(wheat.remaining_batches.unwrap() > 0);
    assert!(wheat.inputs.is_empty());
    assert!(wheat.prefix_attempted);
    assert!(wheat.feasible_sequence_found);
    assert!(wheat.best_competing_score.is_some());
    assert_eq!(
        wheat.selected,
        worker
            .active_plan()
            .unwrap()
            .plan()
            .actions()
            .contains(&CitizenAction::Produce(Recipe::GrowWheat))
    );
    let snapshot = decision.clone();
    let later = universe.advance(1).unwrap();
    assert_eq!(
        citizen(&later, id)
            .active_plan()
            .unwrap()
            .plan()
            .decision()
            .unwrap(),
        &snapshot
    );
}

#[test]
fn decision_proves_missing_supplies_without_claiming_missing_water_is_unavailable() {
    let (universe, id) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[(Good::Berries, 930)],
        0,
    );
    let universe = universe.start_planning(id).unwrap();
    let decision = citizen(&universe, id)
        .active_plan()
        .unwrap()
        .plan()
        .decision()
        .unwrap();
    let bread = decision
        .production
        .iter()
        .find(|d| d.recipe == Recipe::BakeBread)
        .unwrap();
    let flour = bread
        .inputs
        .iter()
        .find(|input| input.good == Good::Flour)
        .unwrap();
    let water = bread
        .inputs
        .iter()
        .find(|input| input.good == Good::Water)
        .unwrap();
    assert_eq!(flour.carried_units, 0);
    assert!(flour.supply_shortfall);
    assert!(!water.supply_shortfall);
    assert!(!bread.feasible_sequence_found);
    assert!(!bread.selected);
}

#[test]
fn decision_observes_recipe_only_reached_in_later_goals() {
    let (universe, id) = producer(
        Universe::with_map(Map::default()).with_prices(
            learning_lord_simulation::marketplace::Prices::default()
                .with_price(Good::Wheat, 10000.0)
                .unwrap(),
        ),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Berries, 930), (Good::Wheat, 400)],
        0,
    );
    let universe = universe.start_planning(id).unwrap();
    let plan = citizen(&universe, id).active_plan().unwrap().plan();
    let decision = plan.decision().unwrap();
    assert_eq!(
        decision.selected_goal,
        learning_lord_simulation::planning::goals::Effect::ListExcess
    );
    assert!(decision.candidates.iter().all(|c| {
        c.forecast
            .as_ref()
            .unwrap()
            .actions
            .iter()
            .all(|a| !matches!(a, CitizenAction::Produce(_)))
    }));
    let wheat = decision
        .production
        .iter()
        .find(|d| d.recipe == Recipe::GrowWheat)
        .unwrap();
    assert!(!wheat.prefix_attempted);
    assert!(wheat.feasible_sequence_found);
    assert!(wheat.best_competing_score.is_some());
}
