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
    stocks: &[(Good, f64)],
    coins: f64,
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

fn demand(universe: Universe, goods: &[(Good, f64)]) -> Universe {
    let (universe, buyer) = universe
        .with_citizen(
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(100.0).unwrap(),
        )
        .unwrap();
    universe
        .with_purchase_request(buyer, ShoppingList::new(goods.iter().copied()).unwrap())
        .unwrap()
}

fn list(universe: Universe, id: AgentId, good: Good, grams: f64) -> Universe {
    universe
        .start_action(id, CitizenAction::List(good, grams))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap()
}

#[test]
fn competing_bakers_each_consider_the_full_unmet_demand() {
    let inputs = [
        (Good::Berries, 930.0),
        (Good::Bread, 800.0),
        (Good::Flour, 2_000.0),
        (Good::Wood, 500.0),
        (Good::Water, 2_000.0),
        (Good::BerryPie, 2_000.0),
    ];
    let (universe, first) = producer(
        Universe::with_map(Map::default()),
        "First",
        StartingRole::Baker,
        &inputs,
        0.0,
    );
    let (universe, second) = producer(universe, "Second", StartingRole::Baker, &inputs, 0.0);
    let universe = demand(universe, &[(Good::Bread, 2_800.0)]);
    assert_eq!(universe.market().unmet_grams(Good::Bread), 2_800.0);
    for id in [first, second] {
        assert_eq!(
            targets(&universe, id).remaining_batches(Recipe::BakeBread),
            10.0
        );
    }
}

#[test]
fn carried_and_listed_stock_reduce_the_daily_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 400.0)],
        0.0,
    );
    assert_eq!(
        targets(&universe, farmer).remaining_batches(Recipe::GrowWheat),
        10.0
    );
    let fully_stocked = citizen(&universe, farmer)
        .with_good(Good::Wheat, 2_400.0)
        .unwrap();
    assert_eq!(
        ProductionTargets::calculate(&fully_stocked, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        0.0
    );
    let listed = list(universe, farmer, Good::Wheat, 200.0);
    assert_eq!(citizen(&listed, farmer).grams(Good::Wheat), 200.0);
    assert_eq!(
        targets(&listed, farmer).remaining_batches(Recipe::GrowWheat),
        10.0
    );
    let withdrawn = listed
        .start_action(farmer, CitizenAction::Withdraw(Good::Wheat, 200.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(
        targets(&withdrawn, farmer).remaining_batches(Recipe::GrowWheat),
        10.0
    );
}

#[test]
fn skill_adjusted_floor_subtracts_adas_existing_listing() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Ada",
        StartingRole::Farmer,
        &[(Good::Wheat, 200.0)],
        0.0,
    );
    let universe = list(universe, farmer, Good::Wheat, 200.0);
    let worker = citizen(&universe, farmer)
        .with_skill(Skill::Farming, 1.1)
        .unwrap();
    assert_eq!(Recipe::GrowWheat.duration_ms(&worker).unwrap(), 3_582_090);
    assert_eq!(
        ProductionTargets::calculate(&worker, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        11.0
    );
}

#[test]
fn completed_work_reduces_capacity_without_shrinking_the_stock_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 2_000.0)],
        0.0,
    );
    let working = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::GrowWheat))
        .unwrap();
    let finished = working
        .advance(working.active_action().unwrap().duration_ms())
        .unwrap();
    assert_eq!(finished.production_work_today_ms(), 60 * 60_000);
    assert_eq!(finished.grams(Good::Wheat), 2_200.0);
    assert_eq!(
        ProductionTargets::calculate(&finished, 60 * 60_000)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        1.0
    );
}

#[test]
fn in_progress_output_counts_toward_the_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 2_000.0)],
        0.0,
    );
    let working = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::GrowWheat))
        .unwrap();
    let target = ProductionTargets::calculate(&working, 0).unwrap();
    assert_eq!(target.remaining_batches(Recipe::GrowWheat), 2.0);
}

#[test]
fn each_profitable_good_has_a_floor_with_a_shared_work_budget() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 3_000.0),
            (Good::Flour, 3_000.0),
            (Good::Wood, 750.0),
            (Good::Water, 3_000.0),
            (Good::BerryPie, 1_000.0),
        ],
        0.0,
    );
    let target = targets(&universe, baker);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 4.0);
    assert_eq!(target.remaining_batches(Recipe::BakeBread), 6.0);
    let worker = citizen(&universe, baker);
    let work: f64 = target
        .batches()
        .map(|(recipe, count)| count * recipe.duration_ms(worker).unwrap() as f64)
        .sum();
    assert_eq!(work, DAILY_CAPACITY_MS as f64);
}

#[test]
fn low_positive_demand_keeps_the_daily_floor() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[],
        0.0,
    );
    assert_eq!(
        targets(&universe, farmer).remaining_batches(Recipe::GrowWheat),
        12.0
    );
    let universe = demand(universe, &[(Good::Wheat, 200.0)]);
    assert_eq!(
        targets(&universe, farmer).remaining_batches(Recipe::GrowWheat),
        12.0
    );
}

#[test]
fn shared_recipe_inputs_and_cash_cannot_fund_each_recipe_independently() {
    for (flour, wood, water, coins) in [(100.0, 25.0, 100.0, 0.0), (0.0, 0.0, 0.0, 0.123)] {
        let (universe, baker) = producer(
            Universe::with_map(Map::default()),
            "Baker",
            StartingRole::Baker,
            &[
                (Good::Berries, 510.0),
                (Good::Flour, flour),
                (Good::Wood, wood),
                (Good::Water, water),
            ],
            coins,
        );
        let target = targets(&universe, baker);
        let bread = target.remaining_batches(Recipe::BakeBread);
        let pie = target.remaining_batches(Recipe::BakeBerryPie);
        assert_eq!(bread + pie, 1.0, "shared inputs or coins were spent twice");
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
        0.0
    );
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[],
        0.0,
    );
    let worker = citizen(&universe, farmer);
    let unskilled = worker.with_skill(Skill::Farming, 0.0).unwrap();
    assert_eq!(
        ProductionTargets::calculate(&unskilled, 0)
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        0.0
    );
    let normal = targets(&universe, farmer).remaining_batches(Recipe::GrowWheat);
    let skilled = worker.with_skill(Skill::Farming, 11.0).unwrap();
    let faster = ProductionTargets::calculate(&skilled, 0)
        .unwrap()
        .remaining_batches(Recipe::GrowWheat);
    assert_eq!(normal, 12.0);
    assert_eq!(faster, 18.0);
    assert!(
        faster * Recipe::GrowWheat.duration_ms(&skilled).unwrap() as f64
            <= DAILY_CAPACITY_MS as f64
    );
}

#[test]
fn active_batch_uses_inputs_once_and_subtracts_its_output_from_the_target() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 930.0),
            (Good::Flour, 200.0),
            (Good::Wood, 50.0),
            (Good::Water, 200.0),
            (Good::BerryPie, 2_000.0),
        ],
        0.0,
    );
    let universe = demand(universe, &[(Good::Bread, 400.0)]);
    let working = citizen(&universe, baker)
        .start_action(CitizenAction::Produce(Recipe::BakeBread))
        .unwrap();
    let target = ProductionTargets::calculate(&working, 0).unwrap();
    assert_eq!(target.remaining_batches(Recipe::BakeBread), 2.0);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 0.0);
}

#[test]
fn personal_food_reserve_is_shared_across_edible_goods_and_does_not_block_eating() {
    let worker = Citizen::new(80.0)
        .unwrap()
        .with_good(Good::Berries, 124.0)
        .unwrap()
        .with_good(Good::Bread, 480.0)
        .unwrap()
        .with_good(Good::BerryPie, 100.0)
        .unwrap();
    let reserves = worker.reserved_goods().unwrap();
    let nutrition: f64 = Good::FOOD
        .into_iter()
        .map(|good| reserves.grams(good) * good.nutrition_per_gram().unwrap())
        .sum();
    assert!((nutrition - 300.0).abs() < 1e-9);
    let excess = worker.excess_goods().unwrap();
    let excess_nutrition: f64 = Good::FOOD
        .into_iter()
        .map(|good| excess.grams(good) * good.nutrition_per_gram().unwrap())
        .sum();
    assert!((excess_nutrition - 40.0).abs() < 1e-9);
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
        &[(Good::Flour, 1_000.0)],
        0.0,
    );
    let expensive = universe.with_prices(universe.prices().with_price(Good::Flour, 10.0).unwrap());
    let listed = list(expensive, seller, Good::Flour, 1_000.0);
    let listed = listed.with_prices(Universe::with_map(Map::default()).prices());
    let (universe, baker) = producer(
        listed,
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 310.0),
            (Good::Flour, 100.0),
            (Good::Wood, 25.0),
            (Good::Water, 100.0),
        ],
        0.0,
    );
    assert!(recipe_profit(Recipe::BakeBread, citizen(&universe, baker)).unwrap() < 0.0);
    assert_eq!(
        targets(&universe, baker).remaining_batches(Recipe::BakeBread),
        0.0
    );
    assert_eq!(
        targets(&universe, baker).remaining_batches(Recipe::BakeBerryPie),
        0.0
    );
}

#[test]
fn own_sales_and_backlog_add_and_inactive_days_reduce_the_sales_rate() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[(Good::Wheat, 2_800.0)],
        0.0,
    );
    let universe = list(universe, farmer, Good::Wheat, 2_800.0);
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(2.0).unwrap())
        .unwrap();
    let universe = universe
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Wheat, 2_800.0)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let universe = universe
        .with_purchase_request(buyer, ShoppingList::single(Good::Wheat, 200.0))
        .unwrap();
    assert_eq!(universe.market().unmet_grams(Good::Wheat), 200.0);
    let worker = citizen(&universe, farmer)
        .with_good(Good::Wheat, 800.0)
        .unwrap();
    assert_eq!(
        ProductionTargets::calculate(&worker, universe.current_time_ms())
            .unwrap()
            .remaining_batches(Recipe::GrowWheat),
        11.0
    );
    let later = ProductionTargets::calculate(&worker, 2 * DAY_MS).unwrap();
    assert_eq!(later.remaining_batches(Recipe::GrowWheat), 8.0);
}

#[test]
fn daily_capacity_subtracts_completed_work_and_active_batch_remaining_time() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Farmer",
        StartingRole::Farmer,
        &[],
        0.0,
    );
    let universe = demand(universe, &[(Good::Wheat, 20_000.0)]);
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
    let active = partial.active_action().unwrap();
    assert_eq!(partial.production_work_today_ms(), 80 * 60_000);
    let free_time = DAILY_CAPACITY_MS - partial.production_work_today_ms() - active.remaining_ms();
    let available_batches = free_time / Recipe::GrowWheat.duration_ms(&partial).unwrap();
    let target = ProductionTargets::calculate(&partial, 80 * 60_000).unwrap();
    assert_eq!(
        target.remaining_batches(Recipe::GrowWheat),
        available_batches as f64 + 1.0
    );
    let future_time: f64 = target
        .batches()
        .map(|(recipe, count)| {
            let future = if recipe == Recipe::GrowWheat {
                count - 1.0
            } else {
                count
            };
            future * recipe.duration_ms(&partial).unwrap() as f64
        })
        .sum();
    assert!(
        future_time + active.remaining_ms() as f64 + partial.production_work_today_ms() as f64
            <= DAILY_CAPACITY_MS as f64
    );
}

#[test]
fn remaining_recipe_inputs_and_personal_food_use_separate_portions_of_inventory() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 1130.0),
            (Good::Flour, 200.0),
            (Good::Wood, 50.0),
            (Good::Water, 150.0),
            (Good::Bread, 2_200.0),
            (Good::BerryPie, 1_900.0),
        ],
        0.0,
    );
    let planned = universe.start_planning(baker).unwrap();
    let worker = citizen(&planned, baker);
    let target = worker.production_targets().unwrap();
    assert_eq!(target.remaining_batches(Recipe::BakeBread), 1.0);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 1.0);
    let reserves = worker.reserved_goods().unwrap();
    assert_eq!(reserves.grams(Good::Flour), 200.0);
    assert_eq!(reserves.grams(Good::Wood), 50.0);
    assert_eq!(reserves.grams(Good::Water), 150.0);
    assert_eq!(reserves.grams(Good::Berries), 1030.0);
    assert_eq!(worker.excess_goods().unwrap().grams(Good::Berries), 100.0);
    assert_eq!(citizen(&universe, baker).production_targets(), None);
    assert_eq!(citizen(&universe, baker).grams(Good::Berries), 1130.0);
}
