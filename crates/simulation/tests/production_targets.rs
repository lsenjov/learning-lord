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
        StartingRole::Baker => Recipe::BakeRolls,
        StartingRole::Weaver => Recipe::SpinThread,
        StartingRole::Tailor => Recipe::MakeClothingBlock,
        StartingRole::Fisher => Recipe::CatchFish,
    };
    let (universe, id) = universe.with_citizen(name, worker).unwrap();
    let universe = if recipe.location().is_public() {
        universe
    } else {
        universe.with_property(id, recipe.location()).unwrap().0
    };
    (universe, id)
}

fn targets(universe: &Universe, id: AgentId) -> ProductionTargets {
    ProductionTargets::calculate(citizen(universe, id), universe.current_time_ms()).unwrap()
}

fn working_ms(target: &ProductionTargets, worker: &Citizen) -> u64 {
    target
        .batches()
        .map(|(recipe, count)| count * recipe.duration_ms(worker).unwrap())
        .sum()
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
fn unmet_demand_does_not_expand_local_capacity_caps() {
    let inputs = [
        (Good::Berries, 930),
        (Good::BreadRoll, 40),
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
    let universe = demand(universe, &[(Good::BreadRoll, 140)]);
    assert_eq!(universe.market().unmet_units(Good::BreadRoll), 140);
    for id in [first, second] {
        assert!(targets(&universe, id).remaining_batches(Recipe::BakeRolls) <= 8);
    }
}

#[test]
fn input_free_production_keeps_a_finite_budget_when_local_stock_is_full() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[(Good::Wood, 400)],
        0,
    );
    assert!(
        working_ms(&targets(&universe, farmer), citizen(&universe, farmer)) <= DAILY_CAPACITY_MS
    );
    let fully_stocked = citizen(&universe, farmer)
        .with_good(Good::Wood, 2_400)
        .unwrap();
    assert!(recipe_profit(Recipe::ChopWood, &fully_stocked).unwrap() > 0.0);
    assert!(
        working_ms(
            &ProductionTargets::calculate(&fully_stocked, 0).unwrap(),
            &fully_stocked
        ) <= DAILY_CAPACITY_MS
    );
    let listed = list(universe, farmer, Good::Wood, 200);
    assert_eq!(citizen(&listed, farmer).units(Good::Wood), 200);
    assert!(working_ms(&targets(&listed, farmer), citizen(&listed, farmer)) <= DAILY_CAPACITY_MS);
    let withdrawn = listed
        .start_action(farmer, CitizenAction::Withdraw(Good::Wood, 200))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert!(
        working_ms(&targets(&withdrawn, farmer), citizen(&withdrawn, farmer)) <= DAILY_CAPACITY_MS
    );
}

#[test]
fn skill_adjusted_export_work_remains_available_with_existing_listings() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Ada",
        StartingRole::Woodcutter,
        &[(Good::Wood, 200)],
        0,
    );
    let universe = list(universe, farmer, Good::Wood, 200);
    let worker = citizen(&universe, farmer)
        .with_skill_practice_ms(Skill::Woodcutting, Some(3_600_000))
        .unwrap();
    assert_eq!(Recipe::ChopWood.duration_ms(&worker).unwrap(), 3_599_176);
    assert!(
        working_ms(&ProductionTargets::calculate(&worker, 0).unwrap(), &worker)
            <= DAILY_CAPACITY_MS
    );
}

#[test]
fn completed_free_work_remains_available_at_export_rates() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[(Good::Wood, 2_000)],
        0,
    );
    let working = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap();
    let finished = working
        .advance(working.active_action().unwrap().duration_ms())
        .unwrap();
    assert_eq!(finished.production_work_today_ms(), 60 * 60_000);
    assert_eq!(finished.units(Good::Wood), 2_200);
    assert!(recipe_profit(Recipe::ChopWood, &finished).unwrap() > 0.0);
    assert!(
        working_ms(
            &ProductionTargets::calculate(&finished, 60 * 60_000).unwrap(),
            &finished
        ) <= DAILY_CAPACITY_MS
    );
}

#[test]
fn input_free_budget_counts_the_active_batch_separately() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[(Good::Wood, 2_000)],
        0,
    );
    let working = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap();
    let target = ProductionTargets::calculate(&working, 0).unwrap();
    assert!(target.remaining_batches(Recipe::ChopWood) >= 1);
    assert!(
        working_ms(&target, &working)
            <= DAILY_CAPACITY_MS + Recipe::ChopWood.duration_ms(&working).unwrap()
    );
}

#[test]
fn ingredient_budgets_share_the_working_horizon_and_respect_local_capacity() {
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
    assert!(target.remaining_batches(Recipe::BakeBerryPie) <= 4);
    assert!(target.remaining_batches(Recipe::BakeRolls) > 0);
    let worker = citizen(&universe, baker);
    let work: u64 = target
        .batches()
        .filter(|(recipe, _)| matches!(recipe, Recipe::BakeRolls | Recipe::BakeBerryPie))
        .map(|(recipe, count)| count * recipe.duration_ms(worker).unwrap())
        .sum();
    assert!(work <= DAILY_CAPACITY_MS);
    let unstocked = worker.with_good(Good::BerryPie, 0).unwrap();
    let full = ProductionTargets::calculate(&unstocked, 0).unwrap();
    assert!(full.remaining_batches(Recipe::BakeBerryPie) <= 8);
    assert!(working_ms(&full, &unstocked) <= DAILY_CAPACITY_MS);
}

#[test]
fn low_positive_demand_does_not_expand_the_shared_working_horizon() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[],
        0,
    );
    assert!(
        working_ms(&targets(&universe, farmer), citizen(&universe, farmer)) <= DAILY_CAPACITY_MS
    );
    let universe = demand(universe, &[(Good::Wood, 200)]);
    assert!(
        working_ms(&targets(&universe, farmer), citizen(&universe, farmer)) <= DAILY_CAPACITY_MS
    );
}

#[test]
fn projected_mixed_targets_preserve_desired_work_beyond_the_current_cash() {
    for (flour, wood, water, coins) in [(100, 25, 100, 0), (0, 0, 0, 13)] {
        let (universe, baker) = producer(
            Universe::with_map(Map::default()).with_prices(
                learning_lord_simulation::marketplace::Prices::default()
                    .with_price(learning_lord_simulation::marketplace::Good::Berries, 5.0)
                    .unwrap()
                    .with_price(learning_lord_simulation::marketplace::Good::Flour, 100.0)
                    .unwrap()
                    .with_price(learning_lord_simulation::marketplace::Good::Wood, 50.0)
                    .unwrap()
                    .with_price(learning_lord_simulation::marketplace::Good::Water, 10.0)
                    .unwrap()
                    .with_price(learning_lord_simulation::marketplace::Good::Wheat, 40.0)
                    .unwrap(),
            ),
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
        let bread = target.remaining_batches(Recipe::BakeRolls);
        let pie = target.remaining_batches(Recipe::BakeBerryPie);
        assert!(bread + pie > 0);
        assert!(working_ms(&target, citizen(&universe, baker)) <= DAILY_CAPACITY_MS);
    }
}

#[test]
fn missing_skill_prevents_targets_and_faster_skill_increases_capacity() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[],
        0,
    );
    let worker = citizen(&universe, farmer);
    let unskilled = worker
        .with_skill_practice_ms(Skill::Woodcutting, None)
        .unwrap();
    assert_eq!(
        ProductionTargets::calculate(&unskilled, 0)
            .unwrap()
            .remaining_batches(Recipe::ChopWood),
        0
    );
    let normal = targets(&universe, farmer).remaining_batches(Recipe::ChopWood);
    let skilled = worker
        .with_skill_practice_ms(
            Skill::Woodcutting,
            Some(learning_lord_simulation::production::MAX_SKILL_PRACTICE_MS),
        )
        .unwrap();
    let faster = ProductionTargets::calculate(&skilled, 0)
        .unwrap()
        .remaining_batches(Recipe::ChopWood);
    assert!(normal > 0);
    assert!(faster > normal);
    assert!(faster * Recipe::ChopWood.duration_ms(&skilled).unwrap() <= DAILY_CAPACITY_MS);
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
    let universe = demand(universe, &[(Good::BreadRoll, 20)]);
    let working = citizen(&universe, baker)
        .start_action(CitizenAction::Produce(Recipe::BakeRolls))
        .unwrap();
    let target = ProductionTargets::calculate(&working, 0).unwrap();
    assert!(target.remaining_batches(Recipe::BakeRolls) >= 1);
    assert!(
        working_ms(&target, &working)
            <= DAILY_CAPACITY_MS + Recipe::BakeRolls.duration_ms(&working).unwrap()
    );
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 0);
}

#[test]
fn personal_food_reserve_is_shared_across_edible_goods_and_does_not_block_eating() {
    let worker = Citizen::new(80.0)
        .unwrap()
        .with_good(Good::Berries, 124)
        .unwrap()
        .with_good(Good::BreadRoll, 30)
        .unwrap()
        .with_good(Good::BerryPie, 1)
        .unwrap();
    let reserves = worker.reserved_goods().unwrap();
    let nutrition: f64 = Good::FOOD
        .into_iter()
        .map(|good| reserves.units(good) as f64 * good.nutrition_per_unit().unwrap())
        .sum();
    assert_eq!(nutrition, 310.0);
    assert_eq!(reserves.units(Good::BerryPie), 1);
    let excess = worker.excess_goods().unwrap();
    let excess_nutrition: f64 = Good::FOOD
        .into_iter()
        .map(|good| excess.units(good) as f64 * good.nutrition_per_unit().unwrap())
        .sum();
    assert_eq!(excess_nutrition, 90.0);
    assert_eq!(nutrition + excess_nutrition, worker.food_nutrition());
    let eating = worker.start_action(CitizenAction::Eat).unwrap();
    let eaten = eating
        .advance(eating.active_action().unwrap().duration_ms())
        .unwrap();
    assert!(eaten.food_nutrition() < worker.food_nutrition());
}

#[test]
fn expensive_replacement_asks_do_not_hide_profitable_owned_input_work() {
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
    assert!(recipe_profit(Recipe::BakeRolls, citizen(&universe, baker)).unwrap() > 0.0);
    let target = targets(&universe, baker);
    assert_eq!(
        target.remaining_batches(Recipe::BakeRolls)
            + target.remaining_batches(Recipe::BakeBerryPie),
        1
    );
}

#[test]
fn sales_history_and_backlog_do_not_change_stock_capacity() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[(Good::Wood, 2_800)],
        0,
    );
    let universe = list(universe, farmer, Good::Wood, 2_800);
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(200).unwrap())
        .unwrap();
    let universe = universe
        .start_action(
            buyer,
            CitizenAction::BuyAt {
                place: citizen(&universe, farmer).selling_place(),
                list: ShoppingList::single(Good::Wood, 2_800),
            },
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let universe = universe
        .with_purchase_request(buyer, ShoppingList::single(Good::Wood, 200))
        .unwrap();
    assert_eq!(universe.market().unmet_units(Good::Wood), 200);
    let worker = citizen(&universe, farmer)
        .with_good(Good::Wood, 800)
        .unwrap();
    let current = ProductionTargets::calculate(&worker, universe.current_time_ms()).unwrap();
    let later = ProductionTargets::calculate(&worker, 2 * DAY_MS).unwrap();
    assert_eq!(
        current.batches().collect::<Vec<_>>(),
        later.batches().collect::<Vec<_>>()
    );
}

#[test]
fn completed_and_active_work_keep_future_projection_within_the_shared_horizon() {
    let (universe, farmer) = producer(
        Universe::with_map(Map::default()),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[],
        0,
    );
    let universe = demand(universe, &[(Good::Wood, 20_000)]);
    let worker = citizen(&universe, farmer)
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap();
    let done = worker
        .advance(worker.active_action().unwrap().duration_ms())
        .unwrap();
    let second = done
        .start_action(CitizenAction::Produce(Recipe::ChopWood))
        .unwrap();
    let partial = second.advance(20 * 60_000).unwrap();
    assert_eq!(partial.production_work_today_ms(), 80 * 60_000);
    let target = ProductionTargets::calculate(&partial, 80 * 60_000).unwrap();
    assert!(target.remaining_batches(Recipe::ChopWood) >= 1);
    assert!(
        working_ms(&target, &partial)
            <= DAILY_CAPACITY_MS + Recipe::ChopWood.duration_ms(&partial).unwrap()
    );
}

#[test]
fn remaining_recipe_inputs_and_personal_reserves_use_separate_portions_of_inventory() {
    let (universe, baker) = producer(
        Universe::with_map(Map::default()),
        "Baker",
        StartingRole::Baker,
        &[
            (Good::Berries, 1130),
            (Good::Flour, 200),
            (Good::Wood, 50),
            (Good::Water, 150),
            (Good::BreadRoll, 110),
            (Good::BerryPie, 15),
        ],
        0,
    );
    let planned = universe.start_planning(baker).unwrap();
    let worker = citizen(&planned, baker);
    let target = worker.production_targets().unwrap();
    assert_eq!(target.remaining_batches(Recipe::BakeRolls), 1);
    assert_eq!(target.remaining_batches(Recipe::BakeBerryPie), 1);
    let reserves = worker.reserved_goods().unwrap();
    assert_eq!(reserves.units(Good::Flour), 200);
    assert_eq!(reserves.units(Good::Wood), 50);
    assert_eq!(reserves.units(Good::Water), 400);
    assert_eq!(reserves.units(Good::Berries), 100);
    assert_eq!(worker.excess_goods().unwrap().units(Good::Berries), 1030);
    assert_eq!(citizen(&universe, baker).production_targets(), None);
    assert_eq!(citizen(&universe, baker).units(Good::Berries), 1130);
}

#[test]
fn decision_snapshots_inputs_targets_and_observed_production() {
    let (universe, id) = producer(
        Universe::with_map(Map::default()).with_prices(
            learning_lord_simulation::marketplace::Prices::default()
                .with_price(learning_lord_simulation::marketplace::Good::Berries, 5.0)
                .unwrap()
                .with_price(learning_lord_simulation::marketplace::Good::Flour, 100.0)
                .unwrap()
                .with_price(learning_lord_simulation::marketplace::Good::Wood, 50.0)
                .unwrap()
                .with_price(learning_lord_simulation::marketplace::Good::Water, 10.0)
                .unwrap()
                .with_price(learning_lord_simulation::marketplace::Good::Wood, 40.0)
                .unwrap()
                .with_price(learning_lord_simulation::marketplace::Good::Flax, 40.0)
                .unwrap(),
        ),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[(Good::Berries, 930)],
        0,
    );
    let universe = universe.start_planning(id).unwrap();
    let worker = citizen(&universe, id);
    let decision = worker.active_plan().unwrap().plan().decision().unwrap();
    let wheat = decision
        .production
        .iter()
        .find(|d| d.recipe == Recipe::ChopWood)
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
            .contains(&CitizenAction::Produce(Recipe::ChopWood))
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
        .find(|d| d.recipe == Recipe::BakeRolls)
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
                .with_price(Good::Wood, 10000.0)
                .unwrap(),
        ),
        "Woodcutter",
        StartingRole::Woodcutter,
        &[(Good::Berries, 930), (Good::Wood, 400)],
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
        .find(|d| d.recipe == Recipe::ChopWood)
        .unwrap();
    assert!(!wheat.prefix_attempted);
    assert!(wheat.feasible_sequence_found);
    assert!(wheat.best_competing_score.is_some());
}
