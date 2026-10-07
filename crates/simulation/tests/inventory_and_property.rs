use learning_lord_simulation::{
    AgentKind, Citizen, CitizenAction, SimulationError, Universe,
    locations::{Location, Map, PlaceId, Position},
    marketplace::{Good, Prices},
    planning::{self, goals::Effect},
};
use uuid::Uuid;

fn citizen(universe: &Universe, id: learning_lord_simulation::AgentId) -> &Citizen {
    let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
    citizen
}

#[test]
fn every_good_validates_weights_and_inventory_branches_are_independent() {
    let empty = Citizen::new(40.0).unwrap();
    let source = empty.clone();
    for good in Good::ALL {
        assert_eq!(empty.units(good), 0);
        let full = empty.with_good(good, 125).unwrap();
        let sibling = empty.with_good(good, 20).unwrap();
        assert_eq!(full.units(good), 125);
        assert_eq!(sibling.units(good), 20);
        assert_eq!(empty, source);
        let busy = full.start_action(CitizenAction::Wait).unwrap();
        assert_eq!(busy.with_good(good, 1), Err(SimulationError::CitizenBusy));
    }
}

#[test]
fn goods_prices_and_food_availability_preserve_unconsumed_inventory() {
    let mut stocked = Citizen::new(40.0)
        .unwrap()
        .with_prices(Prices::new(5.0).unwrap())
        .with_berries(200)
        .unwrap()
        .with_coins(200)
        .unwrap();
    for good in [
        Good::Wheat,
        Good::Flour,
        Good::Wood,
        Good::Water,
        Good::Bread,
    ] {
        assert!(
            Prices::default()
                .price(good)
                .is_some_and(|price| price > 0.0)
        );
        stocked = stocked.with_good(good, 1000).unwrap();
    }
    let expected = stocked.coins() as f64
        + Good::ALL
            .into_iter()
            .map(|good| stocked.prices().value(good, stocked.units(good)).unwrap())
            .sum::<f64>();
    assert!((stocked.wealth().unwrap() - expected).abs() < 1e-12);
    let eaten = stocked
        .start_action(CitizenAction::Eat)
        .unwrap()
        .advance(50_000)
        .unwrap();
    assert_eq!(eaten.berries_units(), 45);
    assert_eq!(eaten.units(Good::Bread), 1000);
    let sold = stocked
        .start_action(CitizenAction::List(Good::Berries, 50))
        .unwrap()
        .advance(300_000)
        .unwrap();
    assert_eq!(sold.berries_units(), 150);
    assert_eq!(sold.market().listed_units(sold.id(), Good::Berries), 50);
    assert_eq!(sold.coins(), 200);
    assert!((sold.wealth().unwrap() - stocked.wealth().unwrap()).abs() < 1e-12);
    for good in [
        Good::Wheat,
        Good::Flour,
        Good::Wood,
        Good::Water,
        Good::Bread,
    ] {
        assert_eq!(eaten.units(good), stocked.units(good));
        assert_eq!(sold.units(good), stocked.units(good));
    }
    let bread_only = Citizen::new(40.0)
        .unwrap()
        .with_good(Good::Bread, 10)
        .unwrap();
    let bread_meal = bread_only.start_action(CitizenAction::Eat).unwrap();
    assert_eq!(bread_meal.active_action().unwrap().duration_ms(), 100_000);
    assert_eq!(bread_meal.advance(100_000).unwrap().units(Good::Bread), 9);
    assert_eq!(
        bread_only.wealth().unwrap(),
        Good::Bread.core_price() * 10.0
    );
}

#[test]
fn ownership_is_multiple_homes_are_private_and_workplaces_allow_customers() {
    let original = Citizen::new(0.0).unwrap();
    let original_id = original.id();
    let (universe, ada) = Universe::default()
        .with_citizen("Ada", original.clone())
        .unwrap();
    assert_eq!(ada, original_id);
    assert_eq!(
        universe.with_citizen("Again", original),
        Err(SimulationError::AgentAlreadyExists)
    );
    let (universe, bram) = universe
        .with_citizen("Bram", Citizen::new(0.0).unwrap())
        .unwrap();
    let before = universe.clone();
    let (universe, field) = universe.with_property(ada, Location::Field).unwrap();
    let (universe, mill) = universe.with_property(ada, Location::Mill).unwrap();
    assert_eq!(citizen(&universe, ada).owned_properties().count(), 3);
    assert_eq!(citizen(&before, ada).owned_properties().count(), 1);
    let ada_citizen = citizen(&universe, ada);
    assert_ne!(ada_citizen.home(), citizen(&universe, bram).home());
    assert_eq!(
        ada_citizen.selling_place(),
        if field.0 < mill.0 { field } else { mill }
    );
    for id in [ada_citizen.home(), field, mill] {
        assert!(ada_citizen.start_action(CitizenAction::Travel(id)).is_ok());
        let action = CitizenAction::Travel(id);
        let visitor = citizen(&universe, bram);
        if id == ada_citizen.home() {
            assert_eq!(
                visitor.start_action(action),
                Err(SimulationError::PrivateProperty)
            );
            assert_eq!(
                visitor.action_duration_ms(action),
                Err(SimulationError::PrivateProperty)
            );
        } else {
            assert!(visitor.start_action(action).is_ok());
            assert!(visitor.action_duration_ms(action).is_ok());
        }
    }
    let invalid = CitizenAction::Travel(PlaceId(Uuid::new_v4()));
    assert_eq!(
        ada_citizen.start_action(invalid),
        Err(SimulationError::PlaceNotFound)
    );
    assert_eq!(
        ada_citizen.action_duration_ms(invalid),
        Err(SimulationError::PlaceNotFound)
    );
    assert!(ada_citizen.start_action(CitizenAction::Sleep).is_ok());
    let foreign_home = universe.map().position(citizen(&universe, bram).home());
    assert_eq!(
        ada_citizen
            .with_position(foreign_home)
            .unwrap()
            .start_action(CitizenAction::Sleep),
        Err(SimulationError::WrongLocation)
    );
    for kind in [Location::Forest, Location::River, Location::Market] {
        let action = CitizenAction::Travel(universe.map().public_place(kind));
        assert!(ada_citizen.start_action(action).is_ok());
        assert!(citizen(&universe, bram).start_action(action).is_ok());
    }
    assert_eq!(ada_citizen.wealth(), citizen(&before, ada).wealth());
    let map = Map::default();
    assert!(
        map.with_place(Location::Home, Position::default(), None, "Invalid")
            .is_err()
    );
    assert!(
        map.with_place(Location::Forest, Position::default(), Some(ada), "Invalid")
            .is_err()
    );
}

#[test]
fn planning_routes_sleep_to_own_home_with_correct_durations_and_goal_metadata() {
    let (universe, ada) = Universe::default()
        .with_citizen("Ada", Citizen::with_needs(-50.0, 100.0).unwrap())
        .unwrap();
    let (universe, bram) = universe
        .with_citizen("Bram", Citizen::new(0.0).unwrap())
        .unwrap();
    let home = citizen(&universe, ada).home();
    let market = universe.map().public_place(Location::Market);
    let away = citizen(&universe, ada)
        .with_position(universe.map().position(market))
        .unwrap();
    let chosen = planning::plan(&away).unwrap();
    assert_eq!(
        &chosen.actions()[chosen.goals()[0].actions.clone()],
        &[CitizenAction::Travel(home), CitizenAction::Sleep]
    );
    assert_eq!(
        chosen.action_durations_ms()[0],
        away.action_duration_ms(CitizenAction::Travel(home))
            .unwrap()
    );
    assert_eq!(chosen.goals()[0].goal, Effect::ReduceTiredness);
    assert_eq!(chosen.goals()[0].actions, 0..2);
    assert!(
        !chosen
            .actions()
            .contains(&CitizenAction::Travel(citizen(&universe, bram).home()))
    );
    let started = away.start_planning().unwrap();
    let at_home = started.advance(chosen.action_durations_ms()[0]).unwrap();
    assert_eq!(at_home.position(), universe.map().position(home));
    assert_eq!(
        at_home.active_action().unwrap().action(),
        CitizenAction::Sleep
    );
    assert_eq!(started.home(), home);
    assert_eq!(away.position(), universe.map().position(market));
}
