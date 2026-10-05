use learning_lord_simulation::locations::{Location, Map, Position};
use learning_lord_simulation::{AgentKind, Citizen, CitizenAction, SimulationError, Universe};

fn map() -> Map {
    Map::new(
        Position { x: 300.0, y: 400.0 },
        Position { x: -400.0, y: 0.0 },
        Position { x: 0.0, y: 600.0 },
    )
    .unwrap()
}

#[test]
fn travel_moves_continuously_and_arrives_exactly_without_mutating_the_source() {
    let source = Citizen::new(0.0).unwrap().with_map(map()).unwrap();
    let walking = source
        .start_action(CitizenAction::Travel(
            source.map().public_place(Location::Forest),
        ))
        .unwrap();
    assert_eq!(walking.active_action().unwrap().duration_ms(), 300_000);
    let halfway = walking.advance(150_000).unwrap();
    assert_eq!(halfway.position(), Position { x: 150.0, y: 200.0 });
    assert!(halfway.hunger() > 0.0);
    assert!(halfway.tiredness() > 0.0);
    let arrival = halfway.advance(150_000).unwrap();
    assert_eq!(arrival.position(), Position { x: 300.0, y: 400.0 });
    assert_eq!(arrival.active_action(), None);
    assert_eq!(
        arrival.position(),
        walking.advance(300_000).unwrap().position()
    );
    let mut split = walking.clone();
    for _ in 0..300 {
        split = split.advance(1000).unwrap();
    }
    assert_eq!(split.position(), arrival.position());
    assert!((split.hunger() - arrival.hunger()).abs() < 1e-10);
    assert_eq!(source.position(), Position::default());
    assert_eq!(
        walking.advance(400_000).unwrap().position(),
        arrival.position()
    );
    assert_eq!(
        arrival
            .start_action(CitizenAction::Travel(
                source.map().public_place(Location::Forest)
            ))
            .unwrap(),
        arrival
    );
}

#[test]
fn site_actions_require_arrival_but_eating_and_waiting_do_not() {
    let home = Citizen::new(0.0)
        .unwrap()
        .with_map(map())
        .unwrap()
        .with_berries(100.0)
        .unwrap();
    for (action, location) in [
        (CitizenAction::Sleep, Location::Home),
        (CitizenAction::Forage, Location::Forest),
        (CitizenAction::BuyBerries, Location::Market),
        (
            CitizenAction::List(learning_lord_simulation::marketplace::Good::Berries, 10.0),
            Location::Market,
        ),
    ] {
        let at_site = home
            .with_position(home.map().position(if location == Location::Home {
                home.home()
            } else {
                home.map().public_place(location)
            }))
            .unwrap();
        assert!(at_site.start_action(action).is_ok());
        let away = home.with_position(Position { x: 1.0, y: 1.0 }).unwrap();
        assert_eq!(
            away.start_action(action),
            Err(SimulationError::WrongLocation)
        );
        assert!(away.start_action(CitizenAction::Eat).is_ok());
        assert!(away.start_action(CitizenAction::Wait).is_ok());
    }
    let busy = home
        .start_action(CitizenAction::Travel(
            home.map().public_place(Location::Forest),
        ))
        .unwrap();
    assert_eq!(
        busy.with_position(Position::default()),
        Err(SimulationError::CitizenBusy)
    );
    assert_eq!(
        busy.with_map(Map::default()),
        Err(SimulationError::CitizenBusy)
    );
    for position in [
        Position {
            x: f64::NAN,
            y: 0.0,
        },
        Position { x: 0.0, y: 1001.0 },
    ] {
        assert_eq!(
            home.with_position(position),
            Err(SimulationError::InvalidPosition)
        );
        assert!(Map::new(position, Position::default(), Position::default()).is_err());
    }
}

#[test]
fn each_universe_has_a_distinct_map_and_citizens_start_at_home() {
    let first = Universe::default();
    let second = Universe::default();
    assert_ne!(first.map(), second.map());
    for _ in 0..50 {
        let universe = Universe::default();
        let (inhabited, id) = universe
            .with_citizen("Ada", Citizen::new(0.0).unwrap())
            .unwrap();
        let map = inhabited.map();
        let places: Vec<_> = map.places().values().collect();
        for (index, place) in places.iter().enumerate() {
            assert!(place.position.x.abs() <= 1000.0 && place.position.y.abs() <= 1000.0);
            for other in &places[index + 1..] {
                assert!(place.position.distance(other.position) >= 200.0);
            }
        }
        let AgentKind::Citizen(citizen) = &inhabited.agents()[&id].kind;
        assert_eq!(citizen.position(), map.position(citizen.home()));
        assert_eq!(citizen.map(), map);
        assert_eq!(inhabited.advance(123).unwrap().map(), map);
    }
}

#[test]
fn travel_is_consistent_across_daily_price_updates_and_snapshot_branches() {
    let universe = Universe::with_map(map())
        .advance(4 * 3_600_000 - 120_000)
        .unwrap();
    let (universe, id) = universe
        .with_citizen("Ada", Citizen::new(0.0).unwrap())
        .unwrap();
    let walking = universe
        .start_action(
            id,
            CitizenAction::Travel(universe.map().public_place(Location::Forest)),
        )
        .unwrap();
    let direct = walking.advance(300_000).unwrap();
    let split = walking.advance(120_000).unwrap().advance(180_000).unwrap();
    let AgentKind::Citizen(a) = &direct.agents()[&id].kind;
    let AgentKind::Citizen(b) = &split.agents()[&id].kind;
    assert_eq!(a.position(), b.position());
    assert_eq!(a.position(), Position { x: 300.0, y: 400.0 });
    assert_eq!(direct.prices(), split.prices());
    assert_eq!(walking.map(), universe.map());
    let AgentKind::Citizen(source) = &walking.agents()[&id].kind;
    assert_eq!(source.position(), Position::default());
}
