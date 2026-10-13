use learning_lord_simulation::locations::Map;
use learning_lord_simulation::marketplace::{Good, Prices};
use learning_lord_simulation::{
    AgentId, AgentKind, Citizen, CitizenAction, SimulationError, TRADE_DURATION_MS, Universe,
};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn citizen(universe: &Universe, id: AgentId) -> &Citizen {
    let AgentKind::Citizen(c) = &universe.agents()[&id].kind;
    c
}
fn supply(grams: u64) -> (Universe, AgentId) {
    let (universe, seller) = Universe::with_map(Map::default())
        .with_prices(Prices::new(100.0).unwrap())
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
    for (berries, coins) in [(0, 100), (40, 100), (40, 2), (0, 10)] {
        let (universe, seller) = supply(500);
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
        let quote = source.market().orders().next().unwrap().quoted_price;
        let expected_berries =
            155_u64.min(berries + (coins as f64 * 1000.0 / quote).floor() as u64);
        let expected_coins =
            coins - ((expected_berries - berries) as f64 * quote / 1000.0).ceil() as i64;
        let started = source
            .start_action(
                buyer,
                CitizenAction::BuyFood(learning_lord_simulation::marketplace::Good::Berries),
            )
            .unwrap();
        let partial = started.advance(TRADE_DURATION_MS - 1).unwrap();
        assert_eq!(citizen(&partial, buyer).berries_units(), berries);
        assert_eq!(citizen(&partial, buyer).coins(), coins);
        let complete = partial.advance(1).unwrap();
        assert_eq!(citizen(&complete, buyer).berries_units(), expected_berries);
        assert_eq!(citizen(&complete, buyer).coins(), expected_coins);
        assert_eq!(citizen(&complete, seller).coins(), coins - expected_coins);
        let rounding_loss =
            citizen(&source, buyer).wealth().unwrap() - citizen(&complete, buyer).wealth().unwrap();
        let premium = (expected_berries - berries) as f64 * (quote - 100.0) / 1000.0;
        assert!((0.0..1.0).contains(&(rounding_loss - premium)));
        assert_eq!(
            citizen(&complete, buyer).coins() + citizen(&complete, seller).coins(),
            coins
        );
        assert!(citizen(&complete, buyer).hunger() > citizen(&source, buyer).hunger());
        assert!(citizen(&complete, buyer).tiredness() > citizen(&source, buyer).tiredness());
        let direct = started.advance(TRADE_DURATION_MS).unwrap();
        assert_eq!(complete.market(), direct.market());
        for id in [seller, buyer] {
            assert_eq!(citizen(&complete, id).coins(), citizen(&direct, id).coins());
            assert_eq!(
                citizen(&complete, id).berries_units(),
                citizen(&direct, id).berries_units()
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
        .with_berries(126)
        .unwrap()
        .with_good(Good::Wood, 12)
        .unwrap()
        .with_coins(-10)
        .unwrap();
    let started = source
        .start_action(CitizenAction::List(Good::Berries, 126))
        .unwrap();
    let partial = started.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(partial.berries_units(), 126);
    assert_eq!(partial.coins(), -10);
    let complete = partial.advance(TRADE_DURATION_MS / 2).unwrap();
    assert_eq!(complete.berries_units(), 0);
    assert_eq!(complete.coins(), -10);
    assert_eq!(
        complete.market().listed_units(complete.id(), Good::Berries),
        126
    );
    assert_eq!(complete.units(Good::Wood), 12);
    close(complete.wealth().unwrap(), source.wealth().unwrap());
    assert_eq!(source.berries_units(), 126);
    assert_eq!(partial.with_berries(1), Err(SimulationError::CitizenBusy));
}

#[test]
fn empty_or_unaffordable_purchases_take_time_without_creating_goods_or_coins() {
    for coins in [0, -100, 100] {
        let source = Citizen::new(0.0).unwrap().with_coins(coins).unwrap();
        let done = source
            .start_action(CitizenAction::BuyFood(
                learning_lord_simulation::marketplace::Good::Berries,
            ))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(done.berries_units(), 0);
        assert_eq!(done.coins(), coins);
        assert!(done.market().trades().is_empty());
        assert!(done.hunger() > source.hunger());
    }
}

#[test]
fn a_hungry_citizen_can_buy_a_finite_owners_stock_for_a_meal_without_mutating_predictions() {
    let (universe, seller) = supply(155);
    let meal_cost = (universe.market().orders().next().unwrap().quoted_price * 0.155).ceil() as i64;
    let (source, buyer) = universe
        .with_citizen(
            "Buyer",
            Citizen::with_needs(80.0, -100.0)
                .unwrap()
                .with_thirst(-100.0)
                .unwrap()
                .with_coins(meal_cost)
                .unwrap(),
        )
        .unwrap();
    let source = source
        .with_stored_good(
            citizen(&source, buyer).home(),
            learning_lord_simulation::storage::GoodsOwner::Agent(buyer),
            Good::Water,
            1200,
        )
        .unwrap();
    let source = source.with_prices(source.prices().with_price(Good::Water, 0.1).unwrap());
    let chosen = learning_lord_simulation::planning::plan(citizen(&source, buyer)).unwrap();
    assert!(
        matches!(chosen.actions()[0], CitizenAction::BuyAt { place, list } if place == source.map().public_place(learning_lord_simulation::locations::Location::Market) && list.units(Good::Berries) == 155),
        "{:?}",
        chosen.actions()
    );
    assert!(
        matches!(chosen.actions()[1], CitizenAction::DepositStored { list, .. } if list.units(Good::Berries) == 155),
        "{:?}",
        chosen.actions()
    );
    assert!(chosen.actions().contains(&CitizenAction::Eat));
    assert_eq!(source.market().orders().count(), 1);
    assert!(source.market().trades().is_empty());
    let bought = source
        .start_planning(buyer)
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert!(matches!(
        citizen(&bought, buyer).active_action().unwrap().action(),
        CitizenAction::DepositStored { .. }
    ));
    assert_eq!(citizen(&bought, buyer).berries_units(), 155);
    assert_eq!(
        citizen(&bought, buyer)
            .stored_available_units(citizen(&bought, buyer).home(), Good::Berries),
        0
    );
    assert_eq!(
        bought.citizen_history(buyer).unwrap().current.consumed[Good::Berries as usize],
        0
    );
    let eating = bought.advance(TRADE_DURATION_MS).unwrap();
    assert_eq!(
        citizen(&eating, buyer).active_action().unwrap().action(),
        CitizenAction::Eat
    );
    assert_eq!(citizen(&eating, buyer).berries_units(), 0);
    assert_eq!(citizen(&eating, buyer).coins(), 0);
    assert_eq!(citizen(&eating, seller).coins(), meal_cost);
    assert_eq!(source.market().listed_units(seller, Good::Berries), 155);
}

#[test]
fn simultaneous_buyers_compete_deterministically_and_partial_fills_conserve_stock() {
    let (universe, seller) = supply(150);
    let (universe, a) = universe
        .with_citizen("A", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let (universe, b) = universe
        .with_citizen("B", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let started = universe
        .start_action(
            a,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100,
            )),
        )
        .unwrap()
        .start_action(
            b,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100,
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
    assert_eq!(citizen(&direct, first).berries_units(), 100);
    assert_eq!(citizen(&direct, second).berries_units(), 50);
    let quote = started.market().orders().next().unwrap().quoted_price;
    assert_eq!(
        citizen(&direct, seller).coins(),
        (quote * 0.1).ceil() as i64 + (quote * 0.05).ceil() as i64
    );
    assert_eq!(
        citizen(&direct, a).coins()
            + citizen(&direct, b).coins()
            + citizen(&direct, seller).coins(),
        200
    );
    assert_eq!(direct.market().orders().count(), 0);
    assert_eq!(direct.market().trades().len(), 2);
}

#[test]
fn withdrawal_is_partial_owned_and_requires_a_market_action() {
    let (universe, seller) = supply(100);
    let (universe, stranger) = universe
        .with_citizen(
            "Stranger",
            Citizen::new(0.0).unwrap().with_coins(100).unwrap(),
        )
        .unwrap();
    let no_stock = universe
        .start_action(stranger, CitizenAction::Withdraw(Good::Berries, 100))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&no_stock, stranger).berries_units(), 0);
    let self_buy = no_stock
        .start_action(
            seller,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100,
            )),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&self_buy, seller).berries_units(), 0);
    let done = self_buy
        .start_action(seller, CitizenAction::Withdraw(Good::Berries, 40))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&done, seller).berries_units(), 40);
    assert_eq!(done.market().listed_units(seller, Good::Berries), 60);
    close(
        citizen(&done, seller).wealth().unwrap(),
        citizen(&universe, seller).wealth().unwrap(),
    );
}

#[test]
fn buyers_use_cheapest_orders_first_between_daily_price_updates() {
    let (universe, cheap) = supply(40);
    let universe = universe.with_prices(Prices::new(200.0).unwrap());
    let (universe, expensive) = universe
        .with_citizen(
            "Expensive",
            Citizen::new(0.0).unwrap().with_berries(100).unwrap(),
        )
        .unwrap();
    let universe = universe
        .start_action(expensive, CitizenAction::List(Good::Berries, 100))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let done = universe
        .start_action(
            buyer,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100,
            )),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let cost = |seller, units: u64| {
        let quote = universe
            .market()
            .orders()
            .find(|order| order.seller.citizen() == Some(seller))
            .unwrap()
            .quoted_price;
        (quote * units as f64 / 1000.0).ceil() as i64
    };
    assert_eq!(citizen(&done, cheap).coins(), cost(cheap, 40));
    assert_eq!(citizen(&done, expensive).coins(), cost(expensive, 60));
    assert_eq!(
        citizen(&done, buyer).coins(),
        100 - cost(cheap, 40) - cost(expensive, 60)
    );
    assert_eq!(done.market().listed_units(expensive, Good::Berries), 40);
    assert_eq!(done.market().trades()[0].seller.citizen(), Some(cheap));
}

#[test]
fn seller_coin_overflow_preserves_the_source() {
    let source = Citizen::new(0.0).unwrap();
    let universe = Universe::with_map(Map::default()).with_prices(Prices::new(100.0).unwrap());
    let (universe, seller) = universe
        .with_citizen(
            "Seller",
            source
                .with_berries(1000)
                .unwrap()
                .with_coins(i64::MAX)
                .unwrap(),
        )
        .unwrap();
    let universe = universe
        .start_action(seller, CitizenAction::List(Good::Berries, 1000))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let started = universe
        .start_action(
            buyer,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                1000,
            )),
        )
        .unwrap();
    assert_eq!(
        started.advance(TRADE_DURATION_MS),
        Err(SimulationError::WealthOverflow)
    );
    assert_eq!(started.market().listed_units(seller, Good::Berries), 1000);
    assert!(started.market().trades().is_empty());
    assert_eq!(citizen(&started, buyer).coins(), 100);
}

#[test]
fn hunger_planning_quotes_live_orders_after_explicit_reference_price_setup() {
    let (universe, _) = supply(155);
    let meal_cost = (universe.market().orders().next().unwrap().quoted_price * 0.155).ceil() as i64;
    let (universe, buyer) = universe
        .with_prices(Prices::new(200.0).unwrap())
        .with_citizen(
            "Buyer",
            Citizen::with_needs(80.0, -100.0)
                .unwrap()
                .with_thirst(-100.0)
                .unwrap()
                .with_coins(meal_cost)
                .unwrap(),
        )
        .unwrap();
    let universe = universe
        .with_stored_good(
            citizen(&universe, buyer).home(),
            learning_lord_simulation::storage::GoodsOwner::Agent(buyer),
            Good::Water,
            1200,
        )
        .unwrap();
    let universe = universe.with_prices(universe.prices().with_price(Good::Water, 0.1).unwrap());
    let chosen = learning_lord_simulation::planning::plan(citizen(&universe, buyer)).unwrap();
    assert!(
        matches!(chosen.actions()[0], CitizenAction::BuyAt { place, list } if place == universe.map().public_place(learning_lord_simulation::locations::Location::Market) && list.units(Good::Berries) == 155),
        "{:?}",
        chosen.actions()
    );
    assert!(
        matches!(chosen.actions()[1], CitizenAction::DepositStored { list, .. } if list.units(Good::Berries) == 155),
        "{:?}",
        chosen.actions()
    );
    assert!(chosen.actions().contains(&CitizenAction::Eat));
    assert!(universe.market().trades().is_empty());
}

#[test]
fn earlier_completions_take_stock_before_later_completions_regardless_of_id_order() {
    let (universe, _) = supply(100);
    let (universe, a) = universe
        .with_citizen("A", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let (universe, b) = universe
        .with_citizen("B", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let early = if a.0 > b.0 { a } else { b };
    let late = if early == a { b } else { a };
    let source = universe
        .start_action(
            early,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100,
            )),
        )
        .unwrap()
        .advance(60_000)
        .unwrap()
        .start_action(
            late,
            CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                Good::Berries,
                100,
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
    assert_eq!(citizen(&direct, early).berries_units(), 100);
    assert_eq!(citizen(&direct, late).berries_units(), 0);
    assert_eq!(direct.market().trades()[0].buyer.citizen(), Some(early));
    assert_eq!(direct.market().trades()[0].time_ms, 2 * TRADE_DURATION_MS);
}

#[test]
fn listing_identity_and_local_predictions_are_independent_of_tick_partitioning() {
    let source = Citizen::new(0.0)
        .unwrap()
        .with_berries(100)
        .unwrap()
        .start_action(CitizenAction::List(Good::Berries, 100))
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
        .with_map(direct.map())
        .unwrap()
        .with_coins(100)
        .unwrap()
        .with_market(direct.market().clone());
    let done = buyer
        .start_action(CitizenAction::Buy(
            learning_lord_simulation::marketplace::ShoppingList::single(Good::Berries, 100),
        ))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(done.berries_units(), 100);
    assert!(done.market().trades().is_empty());
    assert_eq!(buyer.market().orders().count(), 1);
    assert_eq!(direct.coins(), 0);
}

fn production_supply() -> (Universe, AgentId) {
    let seller = Citizen::new(0.0)
        .unwrap()
        .with_good(Good::Flour, 300)
        .unwrap()
        .with_good(Good::Wood, 100)
        .unwrap()
        .with_good(Good::Water, 200)
        .unwrap();
    let (mut universe, id) = Universe::with_map(Map::default())
        .with_prices(
            learning_lord_simulation::marketplace::Prices::default()
                .with_price(Good::Berries, 100.0)
                .unwrap()
                .with_price(Good::Flour, 100.0)
                .unwrap()
                .with_price(Good::Wood, 50.0)
                .unwrap()
                .with_price(Good::Water, 10.0)
                .unwrap(),
        )
        .with_citizen("Supplier", seller)
        .unwrap();
    for (good, grams) in [(Good::Flour, 300), (Good::Wood, 100), (Good::Water, 200)] {
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
        .with_citizen("Baker", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let list =
        ShoppingList::new([(Good::Water, 100), (Good::Wood, 25), (Good::Flour, 100)]).unwrap();
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
        assert_eq!(citizen(&partial, buyer).units(good), 0);
    }
    let done = partial.advance(1).unwrap();
    assert_eq!(citizen(&done, buyer).units(Good::Flour), 100);
    assert_eq!(citizen(&done, buyer).units(Good::Wood), 25);
    assert_eq!(citizen(&done, buyer).units(Good::Water), 100);
    let cost: i64 = list
        .items()
        .map(|(good, units)| {
            let quote = universe
                .market()
                .orders()
                .find(|order| order.good == good)
                .unwrap()
                .quoted_price;
            (quote * units as f64 / 1000.0).ceil() as i64
        })
        .sum();
    assert_eq!(citizen(&done, buyer).coins(), 100 - cost);
    assert_eq!(citizen(&done, seller).coins(), cost);
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
        .with_citizen("Baker", Citizen::new(0.0).unwrap().with_coins(13).unwrap())
        .unwrap();
    let list =
        ShoppingList::new([(Good::Water, 100), (Good::Wood, 100), (Good::Flour, 100)]).unwrap();
    let done = universe
        .start_action(buyer, CitizenAction::Buy(list))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&done, buyer).units(Good::Flour), 100);
    let flour_quote = universe
        .market()
        .orders()
        .find(|order| order.good == Good::Flour)
        .unwrap()
        .quoted_price;
    let wood_quote = universe
        .market()
        .orders()
        .find(|order| order.good == Good::Wood)
        .unwrap()
        .quoted_price;
    let remaining_budget = 13 - (flour_quote * 0.1).ceil() as i64;
    assert_eq!(
        citizen(&done, buyer).units(Good::Wood),
        (remaining_budget as f64 * 1000.0 / wood_quote).floor() as u64
    );
    assert_eq!(citizen(&done, buyer).units(Good::Water), 0);
    assert_eq!(citizen(&done, buyer).coins(), 0);
    assert_eq!(citizen(&done, seller).coins(), 13);
    assert_eq!(done.market().trades().len(), 2);
    let remaining = ShoppingList::new([(Good::Flour, 500), (Good::Water, 500)]).unwrap();
    let (done, rich) = done
        .with_citizen("Rich", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
        .unwrap();
    let remaining_cost: i64 = done
        .market()
        .orders()
        .filter(|order| matches!(order.good, Good::Flour | Good::Water))
        .map(|order| (order.quoted_price * order.units as f64 / 1000.0).ceil() as i64)
        .sum();
    let done = done
        .start_action(rich, CitizenAction::Buy(remaining))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&done, rich).units(Good::Flour), 200);
    assert_eq!(citizen(&done, rich).units(Good::Water), 200);
    assert_eq!(citizen(&done, rich).coins(), 100 - remaining_cost);
}

#[test]
fn shopping_lists_combine_duplicate_requirements_and_reject_integer_overflow() {
    use learning_lord_simulation::marketplace::ShoppingList;
    let list =
        ShoppingList::new([(Good::Flour, 30), (Good::Flour, 70), (Good::Water, 50)]).unwrap();
    assert_eq!(
        list.items().collect::<Vec<_>>(),
        vec![(Good::Flour, 100), (Good::Water, 50)]
    );
    assert_eq!(
        ShoppingList::new([(Good::Flour, u64::MAX), (Good::Flour, u64::MAX)]),
        Err(SimulationError::InvalidQuantity)
    );
}

#[test]
fn settlement_rounds_once_per_seller_and_good_across_orders() {
    use learning_lord_simulation::marketplace::ShoppingList;
    for same_seller in [true, false] {
        let universe = Universe::with_map(Map::default()).with_prices(Prices::new(450.0).unwrap());
        let (universe, first) = universe
            .with_citizen("First", Citizen::new(0.0).unwrap().with_berries(2).unwrap())
            .unwrap();
        let (universe, second) = universe
            .with_citizen(
                "Second",
                Citizen::new(0.0).unwrap().with_berries(1).unwrap(),
            )
            .unwrap();
        let universe = universe
            .start_action(first, CitizenAction::List(Good::Berries, 1))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let seller = if same_seller { first } else { second };
        let universe = universe
            .start_action(seller, CitizenAction::List(Good::Berries, 1))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let (universe, buyer) = universe
            .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(2).unwrap())
            .unwrap();
        let bought = universe
            .start_action(
                buyer,
                CitizenAction::Buy(ShoppingList::single(Good::Berries, 2)),
            )
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let cost = if same_seller { 1 } else { 2 };
        assert_eq!(citizen(&bought, buyer).berries_units(), 2);
        assert_eq!(citizen(&bought, buyer).coins(), 2 - cost);
        assert_eq!(
            citizen(&bought, first).coins() + citizen(&bought, second).coins(),
            cost
        );
        assert_eq!(
            bought
                .market()
                .trades()
                .iter()
                .map(|trade| trade.units)
                .sum::<u64>(),
            2
        );
        assert_eq!(
            bought
                .market()
                .trades()
                .iter()
                .map(|trade| trade.coins)
                .sum::<i64>(),
            cost
        );
    }
}

#[test]
fn purchases_cannot_buy_sub_coin_value_without_a_coin_and_preserve_stock() {
    use learning_lord_simulation::marketplace::ShoppingList;
    let (universe, seller) = supply(1);
    let (universe, buyer) = universe
        .with_citizen("Buyer", Citizen::new(0.0).unwrap())
        .unwrap();
    let bought = universe
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 1)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&bought, buyer).berries_units(), 0);
    assert_eq!(citizen(&bought, buyer).coins(), 0);
    assert_eq!(bought.market().listed_units(seller, Good::Berries), 1);
    assert!(bought.market().trades().is_empty());
}

#[test]
fn owned_workshop_sells_incidental_goods_and_customers_visit_without_production_access() {
    use learning_lord_simulation::locations::{Location, Position};
    use learning_lord_simulation::marketplace::ShoppingList;
    use learning_lord_simulation::production::Recipe;
    let (universe, seller) = Universe::with_map(Map::default())
        .with_prices(
            learning_lord_simulation::marketplace::Prices::default()
                .with_price(learning_lord_simulation::marketplace::Good::Berries, 5.0)
                .unwrap(),
        )
        .with_citizen(
            "Miller",
            Citizen::new(0.0).unwrap().with_berries(100).unwrap(),
        )
        .unwrap();
    let (universe, mill) = universe.with_property(seller, Location::Mill).unwrap();
    let (universe, buyer) = universe
        .with_citizen(
            "Customer",
            Citizen::new(0.0).unwrap().with_coins(100).unwrap(),
        )
        .unwrap();
    assert_eq!(citizen(&universe, seller).selling_place(), mill);
    let listed = universe
        .start_action(seller, CitizenAction::List(Good::Berries, 100))
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(listed.market().orders().next().unwrap().place, mill);
    let customer = citizen(&listed, buyer);
    assert!(customer.start_action(CitizenAction::Travel(mill)).is_ok());
    assert_eq!(
        customer.start_action(CitizenAction::Travel(citizen(&listed, seller).home())),
        Err(SimulationError::PrivateProperty)
    );
    assert_eq!(
        customer.production_place(Recipe::MillFlour),
        Err(SimulationError::MissingProperty)
    );
    assert_eq!(
        customer.start_action(CitizenAction::BuyAt {
            place: customer.home(),
            list: ShoppingList::single(Good::Berries, 1),
        }),
        Err(SimulationError::PrivateProperty)
    );
    assert_eq!(
        customer.start_action(CitizenAction::BuyAt {
            place: learning_lord_simulation::locations::PlaceId(uuid::Uuid::nil()),
            list: ShoppingList::single(Good::Berries, 1),
        }),
        Err(SimulationError::PlaceNotFound)
    );
    let market_only = listed
        .start_action(
            buyer,
            CitizenAction::Buy(ShoppingList::single(Good::Berries, 100)),
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&market_only, buyer).berries_units(), 0);
    let bought = market_only
        .start_action(
            buyer,
            CitizenAction::BuyAt {
                place: mill,
                list: ShoppingList::single(Good::Berries, 100),
            },
        )
        .unwrap()
        .advance(TRADE_DURATION_MS)
        .unwrap();
    assert_eq!(citizen(&bought, buyer).berries_units(), 100);
    assert_eq!(citizen(&bought, seller).coins(), 1);
    let away = citizen(&bought, buyer)
        .with_position(Position { x: 1.0, y: 1.0 })
        .unwrap();
    assert_eq!(
        away.start_action(CitizenAction::BuyAt {
            place: mill,
            list: ShoppingList::single(Good::Berries, 1)
        }),
        Err(SimulationError::WrongLocation)
    );
}
