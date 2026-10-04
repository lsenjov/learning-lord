use crate::{
    DisplaySnapshot, MUTED, PANEL, TEXT,
    citizens::{self, Selection},
    text,
};
use bevy::prelude::*;
use learning_lord_simulation::{
    AgentKind, CitizenAction,
    locations::{Location, MAP_HALF_SIZE_METRES, Position},
};

#[derive(Component)]
pub struct MapLabel(usize);

const MAP_SIZE: f32 = 300.0;

#[derive(Component)]
pub enum MapItem {
    Site(Location),
    Citizen(usize),
    Route,
}

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: px(MAP_SIZE + 24.0),
                flex_shrink: 0.0,
                padding: UiRect::all(px(12)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|panel| {
            panel.spawn(text("Map | 2 km x 2 km", 18.0, TEXT));
            panel
                .spawn((
                    Node {
                        width: px(MAP_SIZE),
                        height: px(MAP_SIZE),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.055, 0.085, 0.095)),
                ))
                .with_children(|map| {
                    map.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            height: px(2),
                            ..default()
                        },
                        UiTransform::default(),
                        BackgroundColor(Color::srgb(0.85, 0.66, 0.25)),
                        MapItem::Route,
                    ));
                    for location in Location::ALL {
                        let color = match location {
                            Location::House => Color::srgb(0.85, 0.75, 0.60),
                            Location::Forest => Color::srgb(0.25, 0.70, 0.40),
                            Location::River => Color::srgb(0.30, 0.65, 0.95),
                            Location::Market => Color::srgb(0.80, 0.50, 0.80),
                        };
                        map.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: px(10),
                                height: px(10),
                                border_radius: BorderRadius::all(px(2)),
                                ..default()
                            },
                            BackgroundColor(color),
                            UiTransform::default(),
                            MapItem::Site(location),
                        ))
                        .with_children(|marker| {
                            marker.spawn((
                                text(location.name(), 12.0, TEXT),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(-14),
                                    top: px(11),
                                    ..default()
                                },
                            ));
                        });
                    }
                    for slot in 0..4 {
                        map.spawn((Node {
                            position_type: PositionType::Absolute, width: px(10), height: px(10),
                            border_radius: BorderRadius::MAX, ..default()
                        }, BackgroundColor(Color::srgb(1.0, 0.75, 0.20)), UiTransform::default(), MapItem::Citizen(slot)))
                        .with_children(|marker| {
                            marker.spawn((text("", 12.0, TEXT), MapLabel(slot), Node {
                                position_type: PositionType::Absolute, left: px(12), top: px(-3),
                                padding: UiRect::horizontal(px(3)), ..default()
                            }, BackgroundColor(Color::srgb(0.055, 0.085, 0.095))));
                        });
                    }
                });
            panel.spawn(text("Walking: 1 km in 10 minutes\nShared positions: labels are offset\n1-4: select citizen", 13.0, MUTED));
        });
}

fn project(position: Position) -> Vec2 {
    Vec2::new(
        ((position.x / MAP_HALF_SIZE_METRES + 1.0) * 0.5) as f32 * MAP_SIZE,
        ((1.0 - position.y / MAP_HALF_SIZE_METRES) * 0.5) as f32 * MAP_SIZE,
    )
}

pub fn refresh(
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    mut labels: Query<(&MapLabel, &mut Text)>,
    mut items: Query<(
        &MapItem,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
        Option<&mut BackgroundColor>,
    )>,
) {
    let universe = &snapshot.0.universe;
    let agents = citizens::sorted_agents(universe);
    let citizen = citizens::selected_agent(universe, selection.0).map(|agent| {
        let AgentKind::Citizen(citizen) = &agent.kind;
        citizen
    });
    for (label, mut value) in &mut labels {
        value.0 = agents.get(label.0).map_or_else(String::new, |(id, agent)| {
            format!(
                "{}{}",
                agent.name,
                if Some(*id) == selection.0 { " *" } else { "" }
            )
        });
    }
    for (item, mut node, mut transform, mut visibility, color) in &mut items {
        match item {
            MapItem::Site(location) => {
                let p = project(universe.map().position(*location));
                node.left = px(p.x - 5.0);
                node.top = px(p.y - 5.0);
            }
            MapItem::Citizen(slot) => {
                *visibility = if agents.get(*slot).is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if let Some((id, agent)) = agents.get(*slot) {
                    let AgentKind::Citizen(marker) = &agent.kind;
                    let overlapping: Vec<_> = agents
                        .iter()
                        .filter(|(_, agent)| {
                            let AgentKind::Citizen(other) = &agent.kind;
                            project(other.position()).distance(project(marker.position())) < 16.0
                        })
                        .map(|(id, _)| *id)
                        .collect();
                    let offset = overlapping
                        .iter()
                        .position(|other| other == id)
                        .unwrap_or(0) as f32;
                    let p = project(marker.position());
                    let shift = (offset - (overlapping.len() - 1) as f32 / 2.0) * 18.0;
                    node.left = px(p.x - 5.0);
                    node.top = px(p.y - 5.0 + shift);
                    if let Some(mut color) = color {
                        color.0 = if Some(*id) == selection.0 {
                            Color::srgb(1.0, 0.75, 0.20)
                        } else {
                            Color::srgb(0.45, 0.68, 0.75)
                        };
                    }
                }
            }
            MapItem::Route => {
                *visibility = Visibility::Hidden;
                if let Some(citizen) = citizen
                    && let Some(active) = citizen.active_action()
                    && let CitizenAction::Travel(destination) = active.action()
                {
                    let from = project(citizen.position());
                    let to = project(citizen.map().position(destination));
                    let delta = to - from;
                    let length = delta.length();
                    let midpoint = (from + to) / 2.0;
                    node.left = px(midpoint.x - length / 2.0);
                    node.top = px(midpoint.y - 1.0);
                    node.width = px(length);
                    *transform = UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x)));
                    *visibility = Visibility::Inherited;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Snapshot;
    use learning_lord_simulation::{Citizen, Universe, locations::Map};

    #[test]
    fn four_overlapping_markers_stay_distinct_and_highlight_selection() {
        let universe = crate::simulation::new_universe().unwrap();
        let agents = citizens::sorted_agents(&universe);
        let selected = agents[2].0;
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe,
            error: None,
            generation: 0,
        }))
        .insert_resource(Selection(Some(selected)))
        .add_systems(Update, refresh);
        let markers: Vec<_> = (0..4)
            .map(|slot| {
                app.world_mut()
                    .spawn((
                        MapItem::Citizen(slot),
                        Node::default(),
                        UiTransform::default(),
                        Visibility::Inherited,
                        BackgroundColor(PANEL),
                    ))
                    .id()
            })
            .collect();
        app.update();
        let positions: Vec<_> = markers
            .iter()
            .map(|&entity| app.world().get::<Node>(entity).unwrap().top)
            .collect();
        for first in 0..4 {
            for second in first + 1..4 {
                assert_ne!(positions[first], positions[second]);
            }
        }
        for entity in &markers {
            assert_eq!(
                app.world().get::<Visibility>(*entity),
                Some(&Visibility::Inherited)
            );
        }
        assert_ne!(
            app.world().get::<BackgroundColor>(markers[2]),
            app.world().get::<BackgroundColor>(markers[0])
        );
    }

    #[test]
    fn markers_follow_travel_and_route_disappears_on_arrival() {
        let map = Map::new(
            Position { x: 300.0, y: 400.0 },
            Position { x: -400.0, y: 0.0 },
            Position { x: 0.0, y: -400.0 },
        )
        .unwrap();
        let (universe, id) =
            Universe::with_map(map).with_citizen("Ada", Citizen::new(0.0).unwrap());
        let travelling = universe
            .start_action(id, CitizenAction::Travel(Location::Forest))
            .unwrap();
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe: travelling.advance(150_000).unwrap(),
            error: None,
            generation: 0,
        }))
        .init_resource::<Selection>()
        .add_systems(Update, refresh);
        let ada = app
            .world_mut()
            .spawn((
                MapItem::Citizen(0),
                Node::default(),
                UiTransform::default(),
                Visibility::Inherited,
            ))
            .id();
        let route = app
            .world_mut()
            .spawn((
                MapItem::Route,
                Node::default(),
                UiTransform::default(),
                Visibility::Inherited,
            ))
            .id();
        let forest = app
            .world_mut()
            .spawn((
                MapItem::Site(Location::Forest),
                Node::default(),
                UiTransform::default(),
                Visibility::Inherited,
            ))
            .id();
        app.update();
        let node = app.world().get::<Node>(ada).unwrap();
        assert_eq!(node.left, px(167.5));
        assert_eq!(node.top, px(115.0));
        let node = app.world().get::<Node>(forest).unwrap();
        assert_eq!(node.left, px(190.0));
        assert_eq!(node.top, px(85.0));
        assert_eq!(
            app.world().get::<Visibility>(route),
            Some(&Visibility::Inherited)
        );
        assert_eq!(app.world().get::<Node>(route).unwrap().width, px(37.5));
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe =
            travelling.advance(300_000).unwrap();
        app.update();
        assert_eq!(app.world().get::<Node>(ada).unwrap().left, px(190.0));
        assert_eq!(
            app.world().get::<Visibility>(route),
            Some(&Visibility::Hidden)
        );
    }
}
