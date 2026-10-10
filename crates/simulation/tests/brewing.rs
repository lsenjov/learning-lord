use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, SimulationError, StartingRole, Universe,
    brewing::{Batch, COLLECTION_MS, FERMENTATION_MS, PREPARATION_MS},
    farming::{CropKind, GROWTH_MS, HOUR_MS},
    locations::{Location, Map, PlaceId, Position},
    marketplace::Good,
    persistence::SaveError,
    production::{MAX_SKILL_PRACTICE_MS, ProductionTargets, Recipe, Skill},
    taxation::{TaxKind, TaxRate},
};

fn brewer() -> (Universe, AgentId, PlaceId) {
    brewer_batches(4)
}
fn brewer_batches(batches: u64) -> (Universe, AgentId, PlaceId) {
    let mut worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Brewer)
        .with_thirst(-100.0)
        .unwrap()
        .with_garment_condition(Some(1.0))
        .unwrap()
        .with_good(Good::Berries, 930)
        .unwrap();
    for &(good, units) in Recipe::BrewBeer.inputs() {
        worker = worker.with_good(good, units * batches).unwrap();
    }
    let (world, id) = Universe::with_map(Map::default())
        .with_citizen("Brewer", worker)
        .unwrap();
    let (world, brewery) = world.with_property(id, Location::Brewery).unwrap();
    (world, id, brewery)
}
fn citizen(world: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen
}
fn prep(brewery: PlaceId) -> CitizenAction {
    CitizenAction::ProduceAt {
        place: brewery,
        recipe: Recipe::BrewBeer,
    }
}
fn perform(worker: Citizen, action: CitizenAction) -> Citizen {
    let duration = worker.action_duration_ms(action).unwrap();
    worker
        .start_action(action)
        .unwrap()
        .advance(duration)
        .unwrap()
}
#[test]
fn preparation_consumes_inputs_now_and_collection_alone_produces_beer() {
    let (world, id, site) = brewer();
    let started = world.start_action(id, prep(site)).unwrap();
    assert_eq!(citizen(&started, id).units(Good::Water), 3000);
    assert_eq!(
        started.map().place(site).unwrap().batches[0],
        Some(Batch::Preparing)
    );
    assert_eq!(
        started.citizen_history(id).unwrap().current.consumed[Good::Water as usize],
        1000
    );
    let half = started.advance(PREPARATION_MS / 2).unwrap();
    let loaded = Universe::from_save_str(&half.to_save_string().unwrap()).unwrap();
    assert_eq!(half, loaded);
    let done = loaded.advance(PREPARATION_MS / 2).unwrap();
    assert_eq!(citizen(&done, id).units(Good::Beer), 0);
    assert_eq!(
        done.map().place(site).unwrap().batches[0],
        Some(Batch::Fermenting {
            ready_at_ms: PREPARATION_MS + FERMENTATION_MS
        })
    );
    assert_eq!(
        done.citizen_history(id).unwrap().current.produced[Good::Beer as usize],
        0
    );
    assert!(
        citizen(&done, id)
            .action_duration_ms(CitizenAction::CollectBeer {
                brewery: site,
                slot: 0
            })
            .is_err()
    );
    let mature = done.advance(FERMENTATION_MS + HOUR_MS).unwrap();
    assert_eq!(
        citizen(&mature, id).skill_practice_ms(Skill::Brewing),
        Some(PREPARATION_MS)
    );
    let collect = CitizenAction::CollectBeer {
        brewery: site,
        slot: 0,
    };
    let started = mature.start_action(id, collect).unwrap();
    let duration = citizen(&started, id).active_action().unwrap().duration_ms();
    assert!(duration <= COLLECTION_MS);
    let half = started.advance(duration / 2).unwrap();
    assert_eq!(citizen(&half, id).units(Good::Beer), 0);
    assert!(half.map().place(site).unwrap().batches[0].is_some());
    let collected = half.advance(duration - duration / 2).unwrap();
    assert_eq!(citizen(&collected, id).units(Good::Beer), 1000);
    assert_eq!(collected.map().place(site).unwrap().batches[0], None);
    assert_eq!(
        collected.citizen_history(id).unwrap().current.produced[Good::Beer as usize],
        1000
    );
}
#[test]
fn slots_limit_preparation_until_a_finished_batch_is_collected() {
    let (world, id, site) = brewer();
    let mut worker = citizen(&world, id).clone();
    for &(good, units) in Recipe::BrewBeer.inputs() {
        worker = worker.with_good(good, units * 8).unwrap();
    }
    let target = ProductionTargets::calculate(&worker, 0).unwrap();
    assert_eq!(target.remaining_batches(Recipe::BrewBeer), 4);
    for _ in 0..4 {
        worker = perform(worker, prep(site));
    }
    assert_eq!(
        ProductionTargets::calculate(&worker, worker.current_time_ms())
            .unwrap()
            .remaining_batches(Recipe::BrewBeer),
        0
    );
    assert_eq!(
        worker.start_action(prep(site)),
        Err(SimulationError::MissingInputs)
    );
    worker = worker.advance(FERMENTATION_MS).unwrap();
    assert!(worker.start_action(prep(site)).is_err());
    worker = perform(
        worker,
        CitizenAction::CollectBeer {
            brewery: site,
            slot: 0,
        },
    );
    worker = perform(worker, prep(site));
    assert_eq!(worker.units(Good::Beer), 1000);
}
#[test]
fn skill_reduces_labor_but_never_fermentation_and_batches_are_discounted() {
    let (world, id, site) = brewer();
    let worker = citizen(&world, id)
        .with_skill_practice_ms(Skill::Brewing, Some(MAX_SKILL_PRACTICE_MS))
        .unwrap();
    let start_wealth = worker.wealth().unwrap();
    let input_value: f64 = Recipe::BrewBeer
        .inputs()
        .iter()
        .map(|&(g, n)| worker.prices().value(g, n).unwrap())
        .sum();
    let started = worker.start_action(prep(site)).unwrap();
    assert_eq!(
        started.active_action().unwrap().duration_ms(),
        PREPARATION_MS / 2
    );
    assert!(
        (started.wealth().unwrap()
            - (start_wealth - input_value
                + worker.prices().value(Good::Beer, 1000).unwrap() * 0.7))
            .abs()
            < 1e-8
    );
    assert_eq!(
        ProductionTargets::calculate(&started, 0)
            .unwrap()
            .remaining_batches(Recipe::BrewBeer),
        4
    );
    let done = started.advance(PREPARATION_MS / 2).unwrap();
    assert_eq!(
        done.beer_batch(site, 0).unwrap(),
        Some(Batch::Fermenting {
            ready_at_ms: PREPARATION_MS / 2 + FERMENTATION_MS
        })
    );
    let done = done.advance(FERMENTATION_MS).unwrap();
    assert_eq!(
        done.action_duration_ms(CitizenAction::CollectBeer {
            brewery: site,
            slot: 0
        })
        .unwrap(),
        COLLECTION_MS / 2
    );
}
#[test]
fn beer_socage_uses_collection_site_and_preparation_has_no_assessment() {
    let (world, id, site) = brewer();
    let (world, _) = world
        .with_tax_rule(
            site,
            "Beer half",
            TaxKind::Socage {
                rates: [(Good::Beer, TaxRate::new(5000).unwrap())]
                    .into_iter()
                    .collect(),
            },
        )
        .unwrap();
    let done = world
        .start_action(id, prep(site))
        .unwrap()
        .advance(PREPARATION_MS)
        .unwrap();
    assert_eq!(citizen(&done, id).tax_reserved_units(Good::Beer), 0);
    let done = done.advance(FERMENTATION_MS).unwrap();
    let started = done
        .start_action(
            id,
            CitizenAction::CollectBeer {
                brewery: site,
                slot: 0,
            },
        )
        .unwrap();
    let duration = citizen(&started, id).active_action().unwrap().duration_ms();
    let done = started.advance(duration).unwrap();
    assert_eq!(citizen(&done, id).tax_reserved_units(Good::Beer), 500);
}
#[test]
fn old_save_versions_reject_before_decoding_the_old_schema() {
    assert!(matches!(
        Universe::from_save_str("(version: 1, universe: ())"),
        Err(SaveError::UnsupportedVersion(1))
    ));
}
#[test]
fn hops_follow_the_four_day_crop_lifecycle() {
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Farmer);
    let (map, field) = worker
        .map()
        .with_place(
            Location::Field,
            Position::default(),
            Some(worker.id()),
            "Hops",
        )
        .unwrap();
    let mut worker = perform(
        worker.with_map(map).unwrap(),
        CitizenAction::Plant {
            field,
            plot: 0,
            crop: CropKind::Hops,
        },
    );
    let planted = worker.crop(field, 0).unwrap().unwrap().planted_at_ms;
    for stage in 0..2 {
        worker = worker
            .advance(planted + (24 + stage as u64 * 24) * HOUR_MS - worker.current_time_ms())
            .unwrap();
        worker = perform(
            worker,
            CitizenAction::Tend {
                field,
                plot: 0,
                stage,
            },
        );
    }
    worker = worker
        .advance(planted + GROWTH_MS - worker.current_time_ms())
        .unwrap();
    worker = perform(worker, CitizenAction::Harvest { field, plot: 0 });
    assert_eq!(worker.units(Good::Hops), 600);
}

#[test]
fn preparation_plans_keep_fermentation_unattended_and_sync_the_map() {
    let (world, id, site) = brewer_batches(2);
    let world = world.with_prices(world.prices().with_price(Good::Beer, 1000.0).unwrap());
    let started = world.start_planning(id).unwrap();
    let worker = citizen(&started, id);
    assert_eq!(
        worker.active_action().unwrap().action().production_recipe(),
        Some(Recipe::BrewBeer)
    );
    assert_eq!(
        started.map().place(site).unwrap().batches[0],
        Some(Batch::Preparing)
    );
    let plan = worker.active_plan().unwrap().plan();
    assert!(
        plan.actions()
            .iter()
            .filter(|action| action.production_recipe() == Some(Recipe::BrewBeer))
            .count()
            >= 2
    );
    assert!(
        plan.action_durations_ms()
            .iter()
            .all(|duration| *duration < FERMENTATION_MS)
    );
    let advanced = started.advance(2 * PREPARATION_MS).unwrap();
    assert!(
        advanced
            .map()
            .place(site)
            .unwrap()
            .batches
            .iter()
            .flatten()
            .count()
            >= 2
    );
    assert_eq!(
        Universe::from_save_str(&advanced.to_save_string().unwrap()).unwrap(),
        advanced
    );
}
#[test]
fn independent_brewers_preserve_each_others_slots_when_plans_resume() {
    let (world, first, first_site) = brewer_batches(2);
    let mut worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Brewer)
        .with_thirst(-100.0)
        .unwrap()
        .with_garment_condition(Some(1.0))
        .unwrap()
        .with_good(Good::Berries, 930)
        .unwrap();
    for &(good, units) in Recipe::BrewBeer.inputs() {
        worker = worker.with_good(good, units * 2).unwrap();
    }
    let (world, second) = world.with_citizen("Second brewer", worker).unwrap();
    let (world, second_site) = world.with_property(second, Location::Brewery).unwrap();
    let world = world.with_prices(world.prices().with_price(Good::Beer, 1000.0).unwrap());
    let world = world
        .start_planning(first)
        .unwrap()
        .start_planning(second)
        .unwrap();
    let world = world.advance(PREPARATION_MS).unwrap();
    let map = world.map();
    for site in [first_site, second_site] {
        let place = map.place(site).unwrap();
        assert_eq!(place.batches.iter().flatten().count(), 2);
        assert!(place.batches.contains(&Some(Batch::Preparing)));
    }
    assert_eq!(
        Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
        world
    );
}
#[test]
fn core_prices_charge_inputs_and_all_active_labor() {
    assert_eq!(Good::Hops.core_price(), 65.0);
    let materials: f64 = Recipe::BrewBeer
        .inputs()
        .iter()
        .map(|&(g, n)| g.core_price() * n as f64 / g.units_per_price_unit() as f64)
        .sum();
    let beer_value = Good::Beer.core_price();
    assert!((beer_value - (materials + 12.5) * 1.3).abs() < 1e-10);
    let (world, id, _) = brewer();
    assert!(
        (learning_lord_simulation::production::recipe_profit(
            Recipe::BrewBeer,
            citizen(&world, id)
        )
        .unwrap()
            - (beer_value - materials))
            .abs()
            < 1e-10
    );
}
#[test]
fn finished_batches_are_offered_by_production_planning() {
    let mut worker = Citizen::with_needs(-210.0, -210.0)
        .unwrap()
        .with_thirst(-210.0)
        .unwrap()
        .with_starting_role(StartingRole::Brewer);
    let (map, site) = worker
        .map()
        .with_place(
            Location::Brewery,
            Position::default(),
            Some(worker.id()),
            "Mature brewery",
        )
        .unwrap();
    worker = worker.with_map(map).unwrap();
    for &(good, units) in Recipe::BrewBeer.inputs() {
        worker = worker.with_good(good, units).unwrap();
    }
    let worker = perform(worker, prep(site))
        .advance(FERMENTATION_MS)
        .unwrap();
    let plan = learning_lord_simulation::planning::plan(&worker).unwrap();
    assert!(
        plan.decision()
            .unwrap()
            .candidates
            .iter()
            .filter_map(|candidate| candidate.forecast.as_ref())
            .any(
                |forecast| forecast.actions.contains(&CitizenAction::CollectBeer {
                    brewery: site,
                    slot: 0
                })
            )
    );
}
#[test]
fn foreign_breweries_reject_preparation_and_collection() {
    let (world, id, site) = brewer();
    let foreign = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Brewer)
        .with_map(citizen(&world, id).map())
        .unwrap();
    assert!(foreign.action_duration_ms(prep(site)).is_err());
    assert_eq!(
        foreign.beer_batch(site, 0),
        Err(SimulationError::PrivateProperty)
    );
}

#[test]
fn beer_is_not_a_substitute_for_recipe_water() {
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_starting_role(StartingRole::Baker)
        .with_good(Good::Flour, 100)
        .unwrap()
        .with_good(Good::Wood, 25)
        .unwrap()
        .with_good(Good::Beer, 1000)
        .unwrap();
    let (world, id) = Universe::with_map(Map::default())
        .with_citizen("Baker", worker)
        .unwrap();
    let (world, bakery) = world.with_property(id, Location::Bakery).unwrap();
    let worker = citizen(&world, id);
    assert_eq!(
        worker.start_action(CitizenAction::ProduceAt {
            place: bakery,
            recipe: Recipe::BakeRolls
        }),
        Err(SimulationError::MissingInputs)
    );
    assert_eq!(worker.units(Good::Beer), 1000);
}

#[test]
fn active_beer_meal_persists_and_resumes_identical_benefits_and_history() {
    let worker = Citizen::new(80.0)
        .unwrap()
        .with_thirst(80.0)
        .unwrap()
        .with_good(Good::Beer, 200)
        .unwrap()
        .with_good(Good::BreadRoll, 4)
        .unwrap();
    let (world, id) = Universe::with_map(Map::default())
        .with_citizen("Diner", worker)
        .unwrap();
    let started = world.start_action(id, CitizenAction::Eat).unwrap();
    let halfway = started.advance(30_000).unwrap();
    assert_eq!(
        citizen(&halfway, id)
            .active_action()
            .unwrap()
            .drinking_good(),
        Good::Beer
    );
    let loaded = Universe::from_save_str(&halfway.to_save_string().unwrap()).unwrap();
    assert_eq!(loaded, halfway);
    assert_eq!(
        loaded.advance(50_000).unwrap(),
        halfway.advance(50_000).unwrap()
    );
}
