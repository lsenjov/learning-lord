use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::{Good, Prices};
use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, SimulationError, TRADE_DURATION_MS, Universe,
};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}
fn citizen(universe: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(c) = &universe.agents()[&id].kind;
    c
}
fn supply(grams: f64) -> (Universe, AgentId) {
    let (universe, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::new(1.0).unwrap())
        .with_citizen(
            "Seller",
            Citizen::new(0.0).unwrap().with_berries(grams).unwrap(),
        )
        .unwrap();
    let universe = universe
        .start_action(seller, CitizenAction::List(Good::Berries, grams))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    (universe, seller)
}

#[test]
fn purchases_top_up_at_completion_without_borrowing() {
    for (berries, coins, expected_berries, expected_coins) in [
        (0.0, 1.0, 100.0, 0.9),
        (40.0, 1.0, 100.0, 0.94),
        (40.0, 0.02, 60.0, 0.0),
        (0.0, 0.1, 100.0, 0.0),
    ] {
        let (universe, seller) = supply(500.0);
        let (source, buyer) = universe
            .with_citizen(
                "Buyer",
                Citizen::new(10.0)
                    .unwrap()
                    .with_berries(berries)
                    .unwrap()
                    .with_coins(coins)
                    .unwrap(),
            )
            .unwrap();
        let started = source
            .start_action(
                buyer,
                CitizenAction::BuyFood(learning_lord_simulation::marketplace::Good::Berries),
            )
            .unwrap();
        let partial = started.advance(TRADE_DURATION_MS - 1).unwrap();
        assert_eq!(citizen(&partial, buyer).berries_grams(), berries);
        assert_eq!(citizen(&partial, buyer).coins(), coins);
        let complete = partial.advance(1).unwrap();
        close(citizen(&complete, buyer).berries_grams(), expected_berries);
        close(citizen(&complete, buyer).coins(), expected_coins);
        close(citizen(&complete, seller).coins(), coins - expected_coins);
        close(
            citizen(&complete, buyer).wealth().unwrap(),
            citizen(&source, buyer).wealth().unwrap(),
        );
        assert!(citizen(&complete, buyer).hunger() > citizen(&source, buyer).hunger());
        assert!(citizen(&complete, buyer).tiredness() > citizen(&source, buyer).tiredness());
        let direct = started.advance(TRADE_DURATION_MS).unwrap();
        assert_eq!(complete.market(), direct.market());
        for id in [seller, buyer] {
            assert_eq!(citizen(&complete, id).coins(), citizen(&direct, id).coins());
            assert_eq!(
                citizen(&complete, id).berries_grams(),
                citizen(&direct, id).berries_grams()
            );
            close(
                citizen(&complete, id).hunger(),
                citizen(&direct, id).hunger(),
            );
            close(
                citizen(&complete, id).tiredness(),
                citizen(&direct, id).tiredness(),
            );
        }
        assert_eq!(citizen(&source, buyer).coins(), coins);
        assert_eq!(complete.market().trades().len(), 1);
        assert_eq!(complete.market().trades()[0].time_ms, 2 * TRADE_DURATION_MS);
    }
}

#[test]
fn listing_preserves_ownership_wealth_and_coins_until_someone_buys() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_berries(125.5)
        .unwrap()
        .with_good(Good::Wood, 12.0)
        .unwrap()
        .with_coins(-0.1)
        .unwrap();
    let started = source
        .start_action(CitizenAction::List(Good::Berries, 125.5))
        .unwrap();
    let partial = started.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(partial.berries_grams(), 125.5);
    assert_eq!(partial.coins(), -0.1);
    let complete = partial.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(complete.berries_grams(), 0.0);
    assert_eq!(complete.coins(), -0.1);
    assert_eq!(
        complete.market().listed_grams(complete.id(), Good::Berries),
        125.5
    );
    assert_eq!(complete.grams(Good::Wood), 12.0);
    close(complete.wealth().unwrap(), source.wealth().unwrap());
    assert_eq!(source.berries_grams(), 125.5);
    assert_eq!(partial.with_berries(1.0), Err(SimulationError::CitizenBusy));
}

#[test]
fn empty_or_unaffordable_purchases_take_time_without_creating_goods_or_coins() {
    for coins in [0.0, -1.0, 1.0] {
        let source = Citizen::new(0.0).unwrap().with_coins(coins).unwrap();
        let done = source
            .start_action(CitizenAction::BuyFood(
                learning_lord_simulation::marketplace::Good::Berries,
            ))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(done.berries_grams(), 0.0);
        assert_eq!(done.coins(), coins);
        assert!(done.market().trades().is_empty());
        assert!(done.hunger() > source.hunger());
    }
}

#[test]
fn a_hungry_citizen_can_buy_a_finite_owners_stock_for_a_meal_without_mutating_predictions() {
    let (universe, seller) = supply(100.0);
    let (source, buyer) = universe
        .with_citizen(
            "Buyer",
            Citizen::with_needs(80.0, -100.0)
                .unwrap()
                .with_coins(0.1)
                .unwrap(),
        )
        .unwrap();
    let chosen = learning_lord_simulation::planning::plan(citizen(&source, buyer)).unwrap();
    assert_eq!(
        &chosen.actions()[..2],
        &[
            CitizenAction::BuyFood(learning_lord_simulation::marketplace::Good::Berries),
            CitizenAction::Eat
        ]
    );
    assert_eq!(source.market().orders().count(), 1);
    assert!(source.market().trades().is_empty());
    let eating = source
        .start_planning(buyer)
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(
        citizen(&eating, buyer).active_action().unwrap().action(),
        CitizenAction::Eat
    );
    close(citizen(&eating, buyer).berries_grams(), 100.0);
    close(citizen(&eating, buyer).coins(), 0.0);
    close(citizen(&eating, seller).coins(), 0.1);
    assert_eq!(source.market().listed_grams(seller, Good::Berries), 100.0);
}

#[test]
fn simultaneous_buyers_compete_deterministically_and_partial_fills_conserve_stock() {
    let (universe, seller) = supply(150.0);
    let (universe, a) = universe
        .with_citizen("A", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let (universe, b) = universe
        .with_citizen("B", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let started = universe
        .start_action(
            a,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100.0,
            )),
        )
        .unwrap()
        .start_action(
            b,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100.0,
            )),
        )
        .unwrap();
    let direct = started.advance(TRADE_DURATION_MS).unwrap();
    let split = started
        .advance(1000)
        .unwrap()
        .advance(TRADE_DURATION_MS - 1000)
        .unwrap();
    assert_eq!(direct, split);
    let first = if a.0 < b.0 { a } else { b };
    let second = if first == a { b } else { a };
    assert_eq!(citizen(&direct, first).berries_grams(), 100.0);
    assert_eq!(citizen(&direct, second).berries_grams(), 50.0);
    close(citizen(&direct, seller).coins(), 0.15);
    close(
        citizen(&direct, a).coins()
            + citizen(&direct, b).coins()
            + citizen(&direct, seller).coins(),
        2.0,
    );
    assert_eq!(direct.market().orders().count(), 0);
    assert_eq!(direct.market().trades().len(), 2);
}

#[test]
fn withdrawal_is_partial_owned_and_requires_a_market_action() {
    let (universe, seller) = supply(100.0);
    let (universe, stranger) = universe
        .with_citizen(
            "Stranger",
            Citizen::new(0.0).unwrap().with_coins(1.0).unwrap(),
        )
        .unwrap();
    let no_stock = universe
        .start_action(stranger, CitizenAction::Withdraw(Good::Berries, 100.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&no_stock, stranger).berries_grams(), 0.0);
    let self_buy = no_stock
        .start_action(
            seller,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100.0,
            )),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&self_buy, seller).berries_grams(), 0.0);
    let done = self_buy
        .start_action(seller, CitizenAction::Withdraw(Good::Berries, 40.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&done, seller).berries_grams(), 40.0);
    assert_eq!(done.market().listed_grams(seller, Good::Berries), 60.0);
    close(
        citizen(&done, seller).wealth().unwrap(),
        citizen(&universe, seller).wealth().unwrap(),
    );
}

#[test]
fn buyers_use_cheapest_orders_first_and_retain_each_orders_original_price() {
    let (universe, cheap) = supply(40.0);
    let universe = universe.with_prices(Prices::new(2.0).unwrap());
    let (universe, expensive) = universe
        .with_citizen(
            "Expensive",
            Citizen::new(0.0).unwrap().with_berries(100.0).unwrap(),
        )
        .unwrap();
    let universe = universe
        .start_action(expensive, CitizenAction::List(Good::Berries, 100.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let done = universe
        .start_action(
            buyer,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100.0,
            )),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    close(citizen(&done, cheap).coins(), 0.04);
    close(citizen(&done, expensive).coins(), 0.12);
    close(citizen(&done, buyer).coins(), 0.84);
    assert_eq!(done.market().listed_grams(expensive, Good::Berries), 40.0);
    assert_eq!(done.market().trades()[0].seller, cheap);
}

#[test]
fn invalid_quantities_and_failed_settlements_preserve_the_source() {
    let source = Citizen::new(0.0).unwrap();
    for grams in [-1.0, f64::NAN, f64::INFINITY] {
        for action in [
            CitizenAction::List(Good::Berries, grams),
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                grams,
            )),
            CitizenAction::Withdraw(Good::Berries, grams),
        ] {
            assert_eq!(
                source.start_action(action),
                Err(SimulationError::InvalidQuantity)
            );
        }
    }
    let universe = Universe::with_map(Map::default()).with_prices(Prices::new(1e308).unwrap());
    let (universe, seller) = universe
        .with_citizen(
            "Seller",
            source
                .with_berries(1000.0)
                .unwrap()
                .with_coins(f64::MAX)
                .unwrap(),
        )
        .unwrap();
    let universe = universe
        .start_action(seller, CitizenAction::List(Good::Berries, 1000.0))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let (universe, buyer) = universe
        .with_citizen(
            "Buyer",
            Citizen::new(0.0).unwrap().with_coins(1e308).unwrap(),
        )
        .unwrap();
    let started = universe
        .start_action(
            buyer,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                1000.0,
            )),
        )
        .unwrap();
    assert_eq!(
        started.advance(TRADE_DURATION_MS),
        Err(SimulationError::WealthOverflow)
    );
    assert_eq!(started.market().listed_grams(seller, Good::Berries), 1000.0);
    assert!(started.market().trades().is_empty());
    assert_eq!(citizen(&started, buyer).coins(), 1e308);
}

#[test]
fn hunger_planning_quotes_retained_asks_instead_of_new_reference_prices() {
    let (universe, _) = supply(100.0);
    let (universe, buyer) = universe
        .with_prices(Prices::new(2.0).unwrap())
        .with_citizen(
            "Buyer",
            Citizen::with_needs(80.0, -100.0)
                .unwrap()
                .with_coins(0.1)
                .unwrap(),
        )
        .unwrap();
    let chosen = learning_lord_simulation::planning::plan(citizen(&universe, buyer)).unwrap();
    assert_eq!(
        &chosen.actions()[..2],
        &[
            CitizenAction::BuyFood(learning_lord_simulation::marketplace::Good::Berries),
            CitizenAction::Eat
        ]
    );
    assert!(universe.market().trades().is_empty());
}

#[test]
fn earlier_completions_take_stock_before_later_completions_regardless_of_id_order() {
    let (universe, _) = supply(100.0);
    let (universe, a) = universe
        .with_citizen("A", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let (universe, b) = universe
        .with_citizen("B", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let early = if a.0 > b.0 { a } else { b };
    let late = if early == a { b } else { a };
    let source = universe
        .start_action(
            early,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100.0,
            )),
        )
        .unwrap()
        .advance(60_000)
        .unwrap()
        .start_action(
            late,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100.0,
            )),
        )
        .unwrap();
    let direct = source.advance(TRADE_DURATION_MS).unwrap();
    let split = source
        .advance(TRADE_DURATION_MS - 60_000)
        .unwrap()
        .advance(60_000)
        .unwrap();
    assert_eq!(direct.market(), split.market());
    assert_eq!(citizen(&direct, early).berries_grams(), 100.0);
    assert_eq!(citizen(&direct, late).berries_grams(), 0.0);
    assert_eq!(direct.market().trades()[0].buyer, early);
    assert_eq!(direct.market().trades()[0].time_ms, 2 * TRADE_DURATION_MS);
}

#[test]
fn listing_identity_and_local_predictions_are_independent_of_tick_partitioning() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_berries(100.0)
        .unwrap()
        .start_action(CitizenAction::List(Good::Berries, 100.0))
        .unwrap();
    let direct = source.advance(TRADE_DURATION_MS).unwrap();
    let split = source
        .advance(1)
        .unwrap()
        .advance(TRADE_DURATION_MS - 1)
        .unwrap();
    assert_eq!(direct.market(), split.market());
    let buyer = Citizen::new(0.0)
        .unwrap()
        .with_coins(1.0)
        .unwrap()
        .with_market(direct.market().clone());
    let done = buyer
        .start_action(CitizenAction::Buy(
            learning_lord_simulation::marketplace::ShoppingList::single(Good::Berries, 100.0),
        ))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(done.berries_grams(), 100.0);
    assert!(done.market().trades().is_empty());
    assert_eq!(buyer.market().orders().count(), 1);
    assert_eq!(direct.coins(), 0.0);
}

fn production_supply() -> (Universe, AgentId) {
    let seller = Citizen::new(0.0)
        .unwrap()
        .with_good(Good::Flour, 300.0)
        .unwrap()
        .with_good(Good::Wood, 100.0)
        .unwrap()
        .with_good(Good::Water, 200.0)
        .unwrap();
    let (mut universe, id) = Universe::with_map(Map::default())
        .with_prices(Prices::new(1.0).unwrap())
        .with_citizen("Supplier", seller)
        .unwrap();
    for (good, grams) in [
        (Good::Flour, 300.0),
        (Good::Wood, 100.0),
        (Good::Water, 200.0),
    ] {
        universe = universe
            .start_action(id, CitizenAction::List(good, grams))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
    }
    (universe, id)
}

#[test]
fn one_five_minute_shopping_action_acquires_multiple_inputs_and_records_each_fill() {
    use learning_lord_simulation::marketplace::ShoppingList;
    let (universe, seller) = production_supply();
    let (universe, buyer) = universe
        .with_citizen("Baker", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let list = ShoppingList::new([
        (Good::Water, 100.0),
        (Good::Wood, 25.0),
        (Good::Flour, 100.0),
    ])
    .unwrap();
    let started = universe
        .start_action(buyer, CitizenAction::Buy(list))
        .unwrap();
    assert_eq!(
        citizen(&started, buyer)
            .active_action()
            .unwrap()
            .duration_ms(),
        TRADE_DURATION_MS
    );
    let partial = started.advance(TRADE_DURATION_MS - 1).unwrap();
    for good in [Good::Flour, Good::Wood, Good::Water] {
        assert_eq!(citizen(&partial, buyer).grams(good), 0.0);
    }
    let done = partial.advance(1).unwrap();
    assert_eq!(citizen(&done, buyer).grams(Good::Flour), 100.0);
    assert_eq!(citizen(&done, buyer).grams(Good::Wood), 25.0);
    assert_eq!(citizen(&done, buyer).grams(Good::Water), 100.0);
    close(citizen(&done, buyer).coins(), 0.8775);
    close(citizen(&done, seller).coins(), 0.1225);
    assert_eq!(done.market().trades().len(), 3);
    for trade in done.market().trades() {
        assert_eq!(trade.time_ms, 4 * TRADE_DURATION_MS);
    }
    assert_eq!(
        done.market(),
        started.advance(TRADE_DURATION_MS).unwrap().market()
    );
    assert!(universe.market().trades().is_empty());
}

#[test]
fn a_shopping_list_fills_in_catalogue_order_with_one_shared_budget_and_finite_stock() {
    use learning_lord_simulation::marketplace::ShoppingList;
    let (universe, seller) = production_supply();
    let (universe, buyer) = universe
        .with_citizen(
            "Baker",
            Citizen::new(0.0).unwrap().with_coins(0.125).unwrap(),
        )
        .unwrap();
    let list = ShoppingList::new([
        (Good::Water, 100.0),
        (Good::Wood, 100.0),
        (Good::Flour, 100.0),
    ])
    .unwrap();
    let done = universe
        .start_action(buyer, CitizenAction::Buy(list))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    close(citizen(&done, buyer).grams(Good::Flour), 100.0);
    close(citizen(&done, buyer).grams(Good::Wood), 50.0);
    assert_eq!(citizen(&done, buyer).grams(Good::Water), 0.0);
    close(citizen(&done, buyer).coins(), 0.0);
    close(citizen(&done, seller).coins(), 0.125);
    assert_eq!(done.market().trades().len(), 2);
    let remaining = ShoppingList::new([(Good::Flour, 500.0), (Good::Water, 500.0)]).unwrap();
    let (done, rich) = done
        .with_citizen("Rich", Citizen::new(0.0).unwrap().with_coins(1.0).unwrap())
        .unwrap();
    let done = done
        .start_action(rich, CitizenAction::Buy(remaining))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&done, rich).grams(Good::Flour), 200.0);
    assert_eq!(citizen(&done, rich).grams(Good::Water), 200.0);
    close(citizen(&done, rich).coins(), 0.78);
}

#[test]
fn shopping_lists_validate_every_entry_and_combine_duplicate_requirements() {
    use learning_lord_simulation::marketplace::ShoppingList;
    let list = ShoppingList::new([
        (Good::Flour, 30.0),
        (Good::Flour, 70.0),
        (Good::Water, 50.0),
    ])
    .unwrap();
    assert_eq!(
        list.items().collect::<Vec<_>>(),
        vec![(Good::Flour, 100.0), (Good::Water, 50.0)]
    );
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            ShoppingList::new([(Good::Flour, 1.0), (Good::Water, invalid)]),
            Err(SimulationError::InvalidQuantity)
        );
    }
    assert_eq!(
        ShoppingList::new([(Good::Flour, f64::MAX), (Good::Flour, f64::MAX)]),
        Err(SimulationError::InvalidQuantity)
    );
}
