use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, PlanningRuntime, SimulationError, Universe,
    calendar::{FIRST_WEEKLY_SETTLEMENT_MS, WEEK_MS},
    locations::{Location, Map, PlaceId},
    marketplace::{Good, Prices, ShoppingList},
    production::{Recipe, Skill},
    storage::GoodsOwner,
    taxation::{TaxAmount, TaxKind, TaxRate},
};

fn citizen(world: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
    citizen
}

fn rates(good: Good, bp: u16) -> imbl::HashMap<Good, TaxRate> {
    [(good, TaxRate::new(bp).unwrap())].into_iter().collect()
}

fn block_worker() -> (Universe, AgentId, PlaceId) {
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_skill(Skill::Tailoring, 1.0)
        .unwrap()
        .with_good(Good::Cloth, 200)
        .unwrap()
        .with_coins(100)
        .unwrap();
    let (world, id) = Universe::with_map(Map::default())
        .with_citizen("Ada", worker)
        .unwrap();
    let (world, place) = world.with_property(id, Location::Tailory).unwrap();
    (world, id, place)
}

fn produce(world: Universe, id: AgentId, recipe: Recipe) -> Universe {
    let busy = world
        .start_action(id, CitizenAction::Produce(recipe))
        .unwrap();
    busy.advance(citizen(&busy, id).active_action().unwrap().remaining_ms())
        .unwrap()
}

#[test]
fn socage_carries_exact_fractions_and_only_authoritative_outputs_are_taxed() {
    let (source, id, place) = block_worker();
    let (mut world, rule) = source
        .with_tax_rule(
            place,
            "Output levy",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 3000),
            },
        )
        .unwrap();
    let predicted = citizen(&world, id)
        .start_action(CitizenAction::Produce(Recipe::MakeClothingBlock))
        .unwrap();
    let predicted = predicted
        .advance(predicted.active_action().unwrap().remaining_ms())
        .unwrap();
    assert_eq!(predicted.units(Good::FlaxBlock), 1);
    for batch in 1..=4 {
        world = produce(world, id, Recipe::MakeClothingBlock);
        assert_eq!(citizen(&world, id).units(Good::FlaxBlock), batch);
        assert_eq!(
            citizen(&world, id).tax_reserved_units(Good::FlaxBlock),
            u64::from(batch == 4)
        );
    }
    assert_eq!(
        world
            .storage()
            .units(place, GoodsOwner::Town, Good::FlaxBlock),
        0
    );
    assert_eq!(world.tax_history().back().unwrap().rule, rule);
    assert_eq!(
        world.tax_history().back().unwrap().amount,
        TaxAmount::Reservation {
            assessed: 1,
            reserved: 1,
            outstanding: 0
        }
    );
    assert_eq!(
        source
            .storage()
            .units(place, GoodsOwner::Town, Good::FlaxBlock),
        0
    );
    assert!(source.tax_rules().is_empty());
    assert_eq!(
        world.citizen_history(id).unwrap().current.produced[Good::FlaxBlock as usize],
        4
    );
}

#[test]
fn stacked_fraction_claims_never_take_more_than_the_current_batch() {
    let (world, id, place) = block_worker();
    let (world, first) = world
        .with_tax_rule(
            place,
            "First",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 5000),
            },
        )
        .unwrap();
    let (world, second) = world
        .with_tax_rule(
            place,
            "Second",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 5000),
            },
        )
        .unwrap();
    let world = produce(
        produce(world, id, Recipe::MakeClothingBlock),
        id,
        Recipe::MakeClothingBlock,
    );
    assert_eq!(citizen(&world, id).units(Good::FlaxBlock), 2);
    assert_eq!(citizen(&world, id).tax_reserved_units(Good::FlaxBlock), 1);
    assert_eq!(
        world
            .storage()
            .units(place, GoodsOwner::Town, Good::FlaxBlock),
        0
    );
    let receipts: Vec<_> = world.tax_history().iter().rev().take(2).collect();
    assert_eq!(receipts[0].rule, second);
    assert_eq!(
        receipts[0].amount,
        TaxAmount::Reservation {
            assessed: 1,
            reserved: 0,
            outstanding: 1
        }
    );
    assert_eq!(receipts[1].rule, first);
    let world = world.without_tax_rule(second).unwrap();
    let world = produce(world, id, Recipe::MakeClothingBlock);
    assert_eq!(citizen(&world, id).units(Good::FlaxBlock), 3);
    assert_eq!(citizen(&world, id).tax_reserved_units(Good::FlaxBlock), 2);
    let world = world
        .advance(FIRST_WEEKLY_SETTLEMENT_MS - world.current_time_ms())
        .unwrap();
    assert_eq!(citizen(&world, id).units(Good::FlaxBlock), 1);
    assert_eq!(citizen(&world, id).town_carried_units(Good::FlaxBlock), 2);
    assert_eq!(
        world
            .storage()
            .units(place, GoodsOwner::Town, Good::FlaxBlock),
        0
    );
}

#[test]
fn weekly_flat_fees_start_sunday_four_and_multiweek_steps_assess_every_boundary() {
    let (world, id, place) = block_worker();
    let (world, rule) = world
        .with_tax_rule(
            place,
            "Weekly",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(id),
                coins: 7,
            },
        )
        .unwrap();
    let first = world.advance(FIRST_WEEKLY_SETTLEMENT_MS - 1).unwrap();
    assert_eq!(first.town_treasury(), 0);
    let next = first.advance(1 + 2 * WEEK_MS).unwrap();
    assert_eq!(next.town_treasury(), 21);
    assert_eq!(citizen(&next, id).coins(), 79);
    assert_eq!(next.tax_arrears(rule, id), 0);
    assert_eq!(
        next.tax_history()
            .iter()
            .map(|receipt| receipt.time_ms)
            .collect::<Vec<_>>(),
        vec![
            FIRST_WEEKLY_SETTLEMENT_MS,
            FIRST_WEEKLY_SETTLEMENT_MS + WEEK_MS,
            FIRST_WEEKLY_SETTLEMENT_MS + 2 * WEEK_MS
        ]
    );
    assert_eq!(next.advance(0).unwrap(), next);
}

#[test]
fn assets_tax_goods_owners_at_current_prices_and_exclude_town_carried_and_buildings() {
    let (world, owner, place) = block_worker();
    let payer = Citizen::new(0.0)
        .unwrap()
        .with_good(Good::Bread, 2)
        .unwrap()
        .with_coins(1000)
        .unwrap();
    let (world, payer) = world.with_citizen("Bram", payer).unwrap();
    let world = world
        .with_stored_good(place, GoodsOwner::Agent(payer), Good::Bread, 2)
        .unwrap()
        .with_stored_good(place, GoodsOwner::Town, Good::Bread, 100)
        .unwrap();
    let (world, rule) = world
        .with_tax_rule(
            place,
            "Asset",
            TaxKind::Asset {
                rates: rates(Good::Bread, 1000),
            },
        )
        .unwrap();
    let world = world.with_prices(Prices::default().with_price(Good::Bread, 20.0).unwrap());
    let next = world.advance(FIRST_WEEKLY_SETTLEMENT_MS).unwrap();
    assert_eq!(next.town_treasury(), 4);
    assert_eq!(citizen(&next, payer).coins(), 996);
    assert_eq!(citizen(&next, owner).coins(), 100);
    assert_eq!(next.tax_arrears(rule, payer), 0);
    assert_eq!(next.storage(), world.storage());
}

fn seller_world() -> (Universe, AgentId, AgentId, PlaceId) {
    let seller = Citizen::new(0.0)
        .unwrap()
        .with_good(Good::Bread, 10)
        .unwrap();
    let (world, seller) = Universe::with_map(Map::default())
        .with_citizen("Seller", seller)
        .unwrap();
    let (world, shop) = world.with_property(seller, Location::Bakery).unwrap();
    let buyer = Citizen::new(0.0).unwrap().with_coins(1000).unwrap();
    let (world, buyer) = world.with_citizen("Buyer", buyer).unwrap();
    (world, seller, buyer, shop)
}

fn list(world: Universe, seller: AgentId, good: Good, units: u64) -> Universe {
    world
        .start_action(seller, CitizenAction::List(good, units))
        .unwrap()
        .advance(300_000)
        .unwrap()
}

fn buy(world: Universe, buyer: AgentId, place: PlaceId, good: Good, units: u64) -> Universe {
    world
        .start_action(
            buyer,
            CitizenAction::BuyAt {
                place,
                list: ShoppingList::single(good, units),
            },
        )
        .unwrap()
        .advance(300_000)
        .unwrap()
}

#[test]
fn sale_income_is_stacked_on_original_revenue_and_asset_assesses_order_escrow() {
    let (world, seller, buyer, place) = seller_world();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Income1",
            TaxKind::Income {
                rates: rates(Good::Bread, 2000),
            },
        )
        .unwrap();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Income2",
            TaxKind::Income {
                rates: rates(Good::Bread, 3000),
            },
        )
        .unwrap();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Assets",
            TaxKind::Asset {
                rates: rates(Good::Bread, 1000),
            },
        )
        .unwrap();
    let world = list(world, seller, Good::Bread, 2);
    let before_sale = world.clone();
    let sold = buy(world, buyer, place, Good::Bread, 2);
    assert_eq!(citizen(&sold, seller).coins(), 15);
    assert_eq!(sold.town_treasury(), 15);
    assert_eq!(sold.market().trades().back().unwrap().place, place);
    assert_eq!(citizen(&sold, buyer).units(Good::Bread), 2);
    let weekly = before_sale
        .advance(FIRST_WEEKLY_SETTLEMENT_MS - before_sale.current_time_ms())
        .unwrap();
    assert_eq!(
        weekly.tax_arrears(learning_lord_simulation::taxation::TaxRuleId(2), seller),
        (weekly.prices().value(Good::Bread, 2).unwrap() * 0.1).floor() as i64
    );
    assert_eq!(weekly.town_treasury(), 0);
}

#[test]
fn rule_edits_and_removal_preserve_arrears_and_retired_debts_retry_weekly() {
    let (world, seller, buyer, place) = seller_world();
    let (world, rule) = world
        .with_tax_rule(
            place,
            "Fee",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(seller),
                coins: 20,
            },
        )
        .unwrap();
    let world = world.advance(FIRST_WEEKLY_SETTLEMENT_MS).unwrap();
    assert_eq!(world.tax_arrears(rule, seller), 20);
    assert_eq!(citizen(&world, seller).coins(), 0);
    let edited = world
        .edit_tax_rule(
            rule,
            "Smaller",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(buyer),
                coins: 0,
            },
        )
        .unwrap();
    assert_eq!(edited.tax_arrears(rule, seller), 20);
    let retired = edited.without_tax_rule(rule).unwrap();
    assert_eq!(retired.tax_arrears(rule, seller), 20);
    let funded = buy(
        list(retired, seller, Good::Bread, 2),
        buyer,
        place,
        Good::Bread,
        2,
    );
    assert_eq!(citizen(&funded, seller).coins(), 30);
    let paid = funded
        .advance(FIRST_WEEKLY_SETTLEMENT_MS + WEEK_MS - funded.current_time_ms())
        .unwrap();
    assert_eq!(paid.tax_arrears(rule, seller), 0);
    assert_eq!(paid.town_treasury(), 20);
    assert_eq!(citizen(&paid, seller).coins(), 10);
    assert!(!paid.tax_rules()[&rule].active);
    assert_eq!(world.tax_arrears(rule, seller), 20);
}

#[test]
fn invalid_rules_and_references_preserve_configuration_snapshots() {
    let (world, id, place) = block_worker();
    assert_eq!(TaxRate::new(10_001), Err(SimulationError::InvalidTaxRate));
    assert_eq!(
        world.with_tax_rule(
            world.map().public_place(Location::Forest),
            "Public",
            TaxKind::Socage {
                rates: rates(Good::Wood, 1000)
            }
        ),
        Err(SimulationError::InvalidTaxRule)
    );
    let warehouse = world
        .map()
        .places()
        .values()
        .find(|place| place.kind.is_town_owned())
        .unwrap()
        .id;
    assert_eq!(
        world.with_tax_rule(
            warehouse,
            "Town",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(id),
                coins: 1
            }
        ),
        Err(SimulationError::InvalidTaxRule)
    );
    assert_eq!(
        world.with_tax_rule(
            PlaceId(uuid::Uuid::new_v4()),
            "Missing",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(id),
                coins: 1
            }
        ),
        Err(SimulationError::PlaceNotFound)
    );
    assert_eq!(
        world.with_tax_rule(
            place,
            "Missing",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(AgentId(
                    uuid::Uuid::new_v4()
                )),
                coins: 1
            }
        ),
        Err(SimulationError::AgentNotFound)
    );
    let (full, rule) = world
        .with_tax_rule(
            place,
            "All",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 10_000),
            },
        )
        .unwrap();
    assert_eq!(
        full.with_tax_rule(
            place,
            "Extra",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 1)
            }
        ),
        Err(SimulationError::InvalidTaxRule)
    );
    assert!(world.tax_rules().is_empty());
    assert!(
        full.without_tax_rule(rule)
            .unwrap()
            .with_tax_rule(
                place,
                "New",
                TaxKind::Socage {
                    rates: rates(Good::FlaxBlock, 10_000)
                }
            )
            .is_ok()
    );
}

#[test]
fn planner_backed_and_sync_advances_match_and_cancellation_preserves_source() {
    let (world, id, place) = block_worker();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Socage",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 10_000),
            },
        )
        .unwrap();
    let world = world
        .start_action(id, CitizenAction::Produce(Recipe::MakeClothingBlock))
        .unwrap();
    let duration = citizen(&world, id).active_action().unwrap().remaining_ms();
    let expected = world.advance(duration).unwrap();
    let mut runtime = PlanningRuntime::default();
    let actual = world
        .advance_with_planner(&mut runtime, duration, || false)
        .unwrap()
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        world
            .advance_with_planner(&mut runtime, duration, || true)
            .unwrap(),
        None
    );
    assert_eq!(citizen(&world, id).units(Good::FlaxBlock), 0);
    assert_eq!(world.town_treasury(), 0);
    assert!(world.tax_history().is_empty());
}

#[test]
fn income_fraction_survives_rule_edit_and_collects_after_four_one_coin_sales() {
    let (world, seller, buyer, place) = seller_world();
    let world = world.with_prices(Prices::default().with_price(Good::Bread, 1.0).unwrap());
    let (world, rule) = world
        .with_tax_rule(
            place,
            "Income",
            TaxKind::Income {
                rates: rates(Good::Bread, 3000),
            },
        )
        .unwrap();
    let mut world = list(world, seller, Good::Bread, 4);
    for sale in 1..=4 {
        world = buy(world, buyer, place, Good::Bread, 1);
        assert_eq!(
            citizen(&world, seller).coins(),
            if sale < 4 { sale } else { 3 }
        );
        if sale == 2 {
            world = world
                .edit_tax_rule(
                    rule,
                    "Renamed",
                    TaxKind::Income {
                        rates: rates(Good::Bread, 3000),
                    },
                )
                .unwrap();
        }
    }
    assert_eq!(world.town_treasury(), 1);
    assert_eq!(
        world.tax_history().back().unwrap().amount,
        TaxAmount::Coins {
            assessed: 1,
            paid: 1,
            arrears: 0
        }
    );
}

#[test]
fn cancellation_after_collection_and_treasury_overflow_roll_back_all_tax_state() {
    let (world, id, place) = block_worker();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Output",
            TaxKind::Socage {
                rates: rates(Good::FlaxBlock, 10_000),
            },
        )
        .unwrap();
    let busy = world
        .start_action(id, CitizenAction::Produce(Recipe::MakeClothingBlock))
        .unwrap();
    let duration = citizen(&busy, id).active_action().unwrap().remaining_ms();
    let saved = busy.clone();
    let mut calls = 0;
    let mut runtime = PlanningRuntime::default();
    let cancelled = busy
        .advance_with_planner(&mut runtime, duration + 1, || {
            calls += 1;
            calls > 2
        })
        .unwrap();
    assert_eq!(cancelled, None);
    assert_eq!(calls, 3);
    assert_eq!(busy, saved);
    assert!(busy.tax_history().is_empty());
    assert!(busy.storage().stock().is_empty());
    let (world, rich) = world
        .with_citizen(
            "Rich",
            Citizen::new(0.0).unwrap().with_coins(i64::MAX).unwrap(),
        )
        .unwrap();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Maximum",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(rich),
                coins: i64::MAX,
            },
        )
        .unwrap();
    let (world, _) = world
        .with_tax_rule(
            place,
            "Overflow",
            TaxKind::FlatFee {
                payer: learning_lord_simulation::taxation::TaxPayer::Agent(id),
                coins: 1,
            },
        )
        .unwrap();
    let saved = world.clone();
    assert_eq!(
        world.advance(FIRST_WEEKLY_SETTLEMENT_MS),
        Err(SimulationError::WealthOverflow)
    );
    assert_eq!(world, saved);
    assert_eq!(world.town_treasury(), 0);
    assert_eq!(citizen(&world, rich).coins(), i64::MAX);
    assert!(world.tax_history().is_empty());
}

#[test]
fn shared_socage_validates_empty_types_and_stacks_with_local_rules() {
    use learning_lord_simulation::taxation::TaxScope;
    let (source, id, place) = block_worker();
    let (shared, _) = source
        .with_scoped_tax_rule(
            TaxScope::LocationType(Location::Field),
            "Fields",
            TaxKind::Socage {
                rates: rates(Good::Wheat, 7000),
            },
        )
        .unwrap();
    assert!(matches!(
        shared.with_scoped_tax_rule(
            TaxScope::LocationType(Location::Field),
            "Too much",
            TaxKind::Socage {
                rates: rates(Good::Wheat, 3001)
            }
        ),
        Err(SimulationError::InvalidTaxRule)
    ));
    let (future, field) = shared.with_property(id, Location::Field).unwrap();
    assert_eq!(future.socage_rate(field, Good::Wheat).basis_points(), 7000);
    let (local, _) = future
        .with_tax_rule(
            field,
            "Local",
            TaxKind::Socage {
                rates: rates(Good::Wheat, 3000),
            },
        )
        .unwrap();
    assert_eq!(local.socage_rate(field, Good::Wheat).basis_points(), 10_000);
    assert_eq!(local.socage_rate(place, Good::Wheat).basis_points(), 0);
    assert!(matches!(
        local.with_scoped_tax_rule(
            TaxScope::LocationType(Location::Field),
            "Extra",
            TaxKind::Socage {
                rates: rates(Good::Wheat, 1)
            }
        ),
        Err(SimulationError::InvalidTaxRule)
    ));
    for kind in [
        Location::Forest,
        Location::River,
        Location::Market,
        Location::Warehouse,
    ] {
        assert!(matches!(
            source.with_scoped_tax_rule(
                TaxScope::LocationType(kind),
                "Public",
                TaxKind::Socage {
                    rates: rates(Good::Wheat, 100)
                }
            ),
            Err(SimulationError::InvalidTaxRule)
        ));
    }
    assert!(source.tax_rules().is_empty());
}

#[test]
fn shared_fees_charge_each_property_owner_and_keep_per_place_debts() {
    use learning_lord_simulation::taxation::{TaxPayer, TaxScope};
    let (world, ada, first) = block_worker();
    let (world, second) = world.with_property(ada, Location::Tailory).unwrap();
    let (world, ben) = world
        .with_citizen("Ben", Citizen::new(0.0).unwrap().with_coins(10).unwrap())
        .unwrap();
    let (world, third) = world.with_property(ben, Location::Tailory).unwrap();
    let (world, rule) = world
        .with_scoped_tax_rule(
            TaxScope::LocationType(Location::Tailory),
            "Owners",
            TaxKind::FlatFee {
                payer: TaxPayer::LocationOwner,
                coins: 60,
            },
        )
        .unwrap();
    let world = world.advance(FIRST_WEEKLY_SETTLEMENT_MS).unwrap();
    assert_eq!(world.tax_arrears(rule, ada), 20);
    assert_eq!(world.tax_arrears(rule, ben), 50);
    assert_eq!(world.town_treasury(), 110);
    let receipts: Vec<_> = world
        .tax_history()
        .iter()
        .filter(|receipt| receipt.rule == rule)
        .collect();
    assert_eq!(receipts.len(), 3);
    assert!(
        receipts
            .windows(2)
            .all(|pair| pair[0].place.0 < pair[1].place.0)
    );
    let ada_receipts: Vec<_> = receipts
        .iter()
        .filter(|receipt| receipt.payer == ada)
        .collect();
    assert!(matches!(
        ada_receipts[0].amount,
        TaxAmount::Coins {
            paid: 60,
            arrears: 0,
            ..
        }
    ));
    assert!(matches!(
        ada_receipts[1].amount,
        TaxAmount::Coins {
            paid: 40,
            arrears: 20,
            ..
        }
    ));
    assert!(
        receipts
            .iter()
            .any(|receipt| receipt.place == first && receipt.payer == ada)
    );
    assert!(
        receipts
            .iter()
            .any(|receipt| receipt.place == second && receipt.payer == ada)
    );
    assert!(
        receipts
            .iter()
            .any(|receipt| receipt.place == third && receipt.payer == ben)
    );
    let retired = world.without_tax_rule(rule).unwrap();
    assert_eq!(retired.tax_arrears(rule, ada), 20);
    assert_eq!(retired.tax_arrears(rule, ben), 50);
}

fn garment_worker() -> (Universe, AgentId, PlaceId) {
    let worker = Citizen::new(0.0)
        .unwrap()
        .with_skill(Skill::Tailoring, 1.0)
        .unwrap()
        .with_good(Good::FlaxBlock, 8)
        .unwrap()
        .with_coins(100)
        .unwrap();
    let (world, id) = Universe::with_map(Map::default())
        .with_citizen("Ada", worker)
        .unwrap();
    let (world, place) = world.with_property(id, Location::Tailory).unwrap();
    (world, id, place)
}

#[test]
fn whole_garment_is_private_wealth_but_unusable_until_sunday_changes_owner() {
    let (world, id, place) = garment_worker();
    let (world, _) = world
        .with_tax_rule(
            place,
            "All garments",
            TaxKind::Socage {
                rates: rates(Good::FlaxGarment, 10_000),
            },
        )
        .unwrap();
    let source = world.clone();
    let predicted = citizen(&world, id)
        .start_action(CitizenAction::Produce(Recipe::AssembleGarment))
        .unwrap();
    let predicted = predicted
        .advance(predicted.active_action().unwrap().remaining_ms())
        .unwrap();
    assert_eq!(predicted.available_units(Good::FlaxGarment), 1);
    assert_eq!(predicted.tax_reserved_units(Good::FlaxGarment), 0);
    assert!(
        learning_lord_simulation::production::recipe_profit(
            Recipe::AssembleGarment,
            citizen(&world, id)
        )
        .unwrap()
            < 0.0
    );
    assert_eq!(
        learning_lord_simulation::production::ProductionTargets::calculate(citizen(&world, id), 0)
            .unwrap()
            .remaining_batches(Recipe::AssembleGarment),
        0
    );
    let world = produce(world, id, Recipe::AssembleGarment);
    let worker = citizen(&world, id);
    assert_eq!(worker.units(Good::FlaxGarment), 1);
    assert_eq!(worker.tax_reserved_units(Good::FlaxGarment), 1);
    assert_eq!(worker.available_units(Good::FlaxGarment), 0);
    assert_eq!(worker.wealth(), predicted.wealth());
    assert_eq!(worker.personal_wellbeing(), predicted.personal_wellbeing());
    assert_eq!(
        world.start_action(id, CitizenAction::EquipClothing),
        Err(SimulationError::MissingInputs)
    );
    assert_eq!(
        worker.with_good(Good::FlaxGarment, 0),
        Err(SimulationError::MissingInputs)
    );
    let listed = world
        .start_action(id, CitizenAction::List(Good::FlaxGarment, 1))
        .unwrap();
    let listed = listed
        .advance(citizen(&listed, id).active_action().unwrap().remaining_ms())
        .unwrap();
    assert_eq!(listed.market().listed_units(id, Good::FlaxGarment), 0);
    let almost = listed
        .advance(FIRST_WEEKLY_SETTLEMENT_MS - listed.current_time_ms() - 1)
        .unwrap();
    assert_eq!(citizen(&almost, id).units(Good::FlaxGarment), 1);
    let settled = almost.advance(1).unwrap();
    assert_eq!(citizen(&settled, id).units(Good::FlaxGarment), 0);
    assert_eq!(
        citizen(&settled, id).town_carried_units(Good::FlaxGarment),
        1
    );
    assert_eq!(
        citizen(&settled, id).tax_reserved_units(Good::FlaxGarment),
        0
    );
    assert_eq!(citizen(&settled, id).wealth().unwrap(), 100.0);
    assert_eq!(
        settled
            .storage()
            .units(place, GoodsOwner::Town, Good::FlaxGarment),
        0
    );
    assert_eq!(citizen(&source, id).units(Good::FlaxGarment), 0);
    assert_eq!(
        settled.citizen_history(id).unwrap().current.produced[Good::FlaxGarment as usize],
        0
    );
}

#[test]
fn moved_reservations_keep_original_tax_place_and_settle_before_stored_asset_tax() {
    let (world, id, origin) = garment_worker();
    let destination = citizen(&world, id).home();
    let (world, socage) = world
        .with_tax_rule(
            origin,
            "Half",
            TaxKind::Socage {
                rates: rates(Good::FlaxGarment, 5000),
            },
        )
        .unwrap();
    let (world, id2) = world
        .with_citizen("Ben", Citizen::new(0.0).unwrap())
        .unwrap();
    let world = world
        .with_stored_good(destination, GoodsOwner::Agent(id), Good::FlaxBlock, 8)
        .unwrap();
    let world = produce(world, id, Recipe::AssembleGarment);
    let world = world
        .withdraw_goods(id, destination, Good::FlaxBlock, 8)
        .unwrap();
    let world = produce(world, id, Recipe::AssembleGarment);
    assert_eq!(citizen(&world, id).units(Good::FlaxGarment), 2);
    let world = world
        .deposit_goods(id, destination, Good::FlaxGarment, 1)
        .unwrap();
    assert_eq!(citizen(&world, id).tax_reserved_units(Good::FlaxGarment), 1);
    assert_eq!(
        world
            .storage()
            .reserved_units(destination, GoodsOwner::Agent(id), Good::FlaxGarment),
        0
    );
    let world = world
        .deposit_goods(id, destination, Good::FlaxGarment, 1)
        .unwrap();
    assert_eq!(
        world
            .storage()
            .reserved_units(destination, GoodsOwner::Agent(id), Good::FlaxGarment),
        1
    );
    assert_eq!(
        world.with_stored_good(destination, GoodsOwner::Agent(id), Good::FlaxGarment, 0),
        Err(SimulationError::MissingInputs)
    );
    let world = world
        .withdraw_goods(id, destination, Good::FlaxGarment, 2)
        .unwrap();
    assert_eq!(citizen(&world, id).tax_reserved_units(Good::FlaxGarment), 1);
    let world = world
        .deposit_goods(id, destination, Good::FlaxGarment, 2)
        .unwrap();
    let (world, asset) = world
        .with_tax_rule(
            destination,
            "Assets",
            TaxKind::Asset {
                rates: rates(Good::FlaxGarment, 10_000),
            },
        )
        .unwrap();
    let expected = world.prices().value(Good::FlaxGarment, 1).unwrap().floor() as i64;
    let settled = world
        .advance(FIRST_WEEKLY_SETTLEMENT_MS - world.current_time_ms())
        .unwrap();
    assert_eq!(
        settled
            .storage()
            .units(destination, GoodsOwner::Agent(id), Good::FlaxGarment),
        1
    );
    assert_eq!(
        settled
            .storage()
            .units(destination, GoodsOwner::Town, Good::FlaxGarment),
        1
    );
    assert_eq!(
        settled
            .storage()
            .reserved_units(destination, GoodsOwner::Agent(id), Good::FlaxGarment),
        0
    );
    let receipt = settled
        .tax_history()
        .iter()
        .find(|receipt| receipt.rule == socage && matches!(receipt.amount, TaxAmount::Goods { .. }))
        .unwrap();
    assert_eq!(receipt.place, origin);
    assert_eq!(receipt.payer, id);
    assert!(settled.tax_history().iter().any(|receipt| receipt.rule == asset && matches!(receipt.amount, TaxAmount::Coins { assessed, .. } if assessed == expected)));
    assert_eq!(
        citizen(&settled, id2).town_carried_units(Good::FlaxGarment),
        0
    );
}
