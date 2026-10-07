use crate::{
    DisplaySnapshot, TEXT,
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

#[derive(Component)]
pub struct SiteLabel(usize);

const MAP_SIZE: f32 = 300.0;
const ROUTE_DOT_SPACING: f32 = 4.0;
const ROUTE_DOTS: usize = (MAP_SIZE * std::f32::consts::SQRT_2 / ROUTE_DOT_SPACING) as usize + 2;
const SITE_LABEL_WIDTH: f32 = 80.0;
const SITE_LABEL_HEIGHT: f32 = 32.0;

#[derive(Component)]
pub enum MapItem {
    Site(usize),
    Citizen(usize),
    Route(usize),
}

#[derive(Resource)]
pub struct MapViewport {
    size: Vec2,
    pan: Vec2,
    zoom: f32,
    drag: Option<Vec2>,
}

impl Default for MapViewport {
    fn default() -> Self {
        Self {
            size: Vec2::splat(MAP_SIZE),
            pan: Vec2::ZERO,
            zoom: 1.0,
            drag: None,
        }
    }
}

impl MapViewport {
    fn project(&self, position: Position) -> Vec2 {
        let scale = self.size.min_element() * 0.5 * self.zoom / MAP_HALF_SIZE_METRES as f32;
        self.size / 2.0 + self.pan + Vec2::new(position.x as f32, -position.y as f32) * scale
    }

    fn zoom_at(&mut self, pointer: Vec2, amount: f32) {
        let zoom = (self.zoom * amount).clamp(0.5, 12.0);
        self.pan =
            pointer - self.size / 2.0 - (pointer - self.size / 2.0 - self.pan) * (zoom / self.zoom);
        self.zoom = zoom;
    }
}

pub fn spawn_background(commands: &mut Commands, sites: usize, citizens: usize) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                overflow: Overflow::clip(),
                ..default()
            },
            GlobalZIndex(-10),
            BackgroundColor(Color::srgb(0.055, 0.085, 0.095)),
        ))
        .with_children(|map| {
            // Unrotated dots respect the scroll viewport's clipping rectangle.
            for slot in 0..ROUTE_DOTS {
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(2),
                        height: px(2),
                        ..default()
                    },
                    UiTransform::default(),
                    BackgroundColor(Color::srgb(0.85, 0.66, 0.25)),
                    MapItem::Route(slot),
                ));
            }
            for slot in 0..sites {
                let color = Color::srgb(0.85, 0.75, 0.60);
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
                    MapItem::Site(slot),
                ))
                .with_children(|marker| {
                    marker.spawn((
                        text("", 12.0, TEXT),
                        SiteLabel(slot),
                        TextLayout::no_wrap(),
                        Visibility::Inherited,
                        Node {
                            position_type: PositionType::Absolute,
                            width: px(SITE_LABEL_WIDTH),
                            height: px(SITE_LABEL_HEIGHT),
                            ..default()
                        },
                    ));
                });
            }
            for slot in 0..citizens {
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(10),
                        height: px(10),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.75, 0.20)),
                    UiTransform::default(),
                    MapItem::Citizen(slot),
                ))
                .with_children(|marker| {
                    marker.spawn((
                        text("", 12.0, TEXT),
                        MapLabel(slot),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(12),
                            top: px(-3),
                            padding: UiRect::horizontal(px(3)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.055, 0.085, 0.095)),
                    ));
                });
            }
        });
}

pub fn interaction(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    capture: Res<crate::floating_ui::InputCapture>,
    state: (Res<DisplaySnapshot>, ResMut<Selection>),
    mut floating: ResMut<crate::floating_ui::Windows>,
    mut viewport: ResMut<MapViewport>,
) {
    let (snapshot, mut selection) = state;
    let Ok(window) = windows.single() else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    if viewport.size != size {
        viewport.size = size;
    }
    let scroll: f32 = wheel
        .read()
        .map(|event| match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.12,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.002,
        })
        .sum();
    let Some(pointer) = window.cursor_position() else {
        if viewport.drag.is_some() {
            viewport.drag = None;
        }
        return;
    };
    if capture.blocked {
        if viewport.drag.is_some() {
            viewport.drag = None;
        }
        return;
    }
    if scroll != 0.0 {
        viewport.zoom_at(pointer, scroll.exp());
    }
    if mouse.just_pressed(MouseButton::Left) {
        let universe = &snapshot.0.universe;
        let agents = citizens::sorted_agents(universe);
        if let Some((id, _)) = agents.iter().find(|(id, agent)| {
            let AgentKind::Citizen(citizen) = &agent.kind;
            citizen_point(&viewport, &agents, *id, citizen.position()).distance(pointer) <= 10.0
        }) {
            selection.0 = Some(*id);
            floating.open(crate::floating_ui::WindowKind::Citizen(*id));
            return;
        }
        if let Some(place) = universe
            .map()
            .places()
            .values()
            .find(|place| viewport.project(place.position).distance(pointer) <= 10.0)
        {
            floating.open(crate::floating_ui::WindowKind::Location(place.id));
            return;
        }
        viewport.drag = Some(pointer);
    }
    if mouse.just_pressed(MouseButton::Middle) {
        viewport.drag = Some(pointer);
    }
    if mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Middle) {
        if let Some(previous) = viewport.drag
            && previous != pointer
        {
            viewport.pan += pointer - previous;
            viewport.drag = Some(pointer);
        }
    } else if viewport.drag.is_some() {
        viewport.drag = None;
    }
}

fn citizen_point(
    viewport: &MapViewport,
    agents: &[(
        learning_lord_simulation::AgentId,
        &learning_lord_simulation::Agent,
    )],
    id: learning_lord_simulation::AgentId,
    position: Position,
) -> Vec2 {
    let overlapping: Vec<_> = agents
        .iter()
        .filter(|(_, agent)| {
            let AgentKind::Citizen(other) = &agent.kind;
            viewport
                .project(other.position())
                .distance(viewport.project(position))
                < 16.0
        })
        .map(|(id, _)| *id)
        .collect();
    let offset = overlapping
        .iter()
        .position(|other| *other == id)
        .unwrap_or(0) as f32;
    viewport.project(position)
        + Vec2::new(0.0, (offset - (overlapping.len() - 1) as f32 / 2.0) * 18.0)
}

fn site_label_position(point: Vec2, occupied: &[Rect], bounds: Vec2) -> Vec2 {
    let size = Vec2::new(SITE_LABEL_WIDTH, SITE_LABEL_HEIGHT);
    let candidates = [
        point + Vec2::new(-size.x / 2.0, 12.0),
        point + Vec2::new(-size.x / 2.0, -size.y - 12.0),
        point + Vec2::new(12.0, -size.y / 2.0),
        point + Vec2::new(-size.x - 12.0, -size.y / 2.0),
    ]
    .map(|position| {
        position.clamp(
            Vec2::splat(2.0),
            (bounds - Vec2::splat(2.0) - size).max(Vec2::splat(2.0)),
        )
    });
    candidates
        .into_iter()
        .find(|position| {
            let end = *position + size;
            occupied.iter().all(|area| {
                end.x <= area.min.x
                    || position.x >= area.max.x
                    || end.y <= area.min.y
                    || position.y >= area.max.y
            })
        })
        .unwrap_or(candidates[0])
}

pub fn refresh(
    viewport: Option<Res<MapViewport>>,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    mut labels: Query<(&MapLabel, &mut Text), Without<SiteLabel>>,
    mut site_labels: Query<(&SiteLabel, &mut Text, &mut Node, &mut Visibility), Without<MapItem>>,
    mut items: Query<(
        &MapItem,
        &mut Node,
        &mut Visibility,
        Option<&mut BackgroundColor>,
    )>,
) {
    if !snapshot.is_changed()
        && !selection.is_changed()
        && !viewport.as_ref().is_some_and(|view| view.is_changed())
    {
        return;
    }
    let fallback = MapViewport::default();
    let viewport = viewport.as_deref().unwrap_or(&fallback);
    let universe = &snapshot.0.universe;
    let map = universe.map();
    let mut sites: Vec<_> = map.places().values().collect();
    sites.sort_by(|a, b| a.name.cmp(&b.name));
    let agents = citizens::sorted_agents(universe);
    let citizen = citizens::selected_agent(universe, selection.0).map(|agent| {
        let AgentKind::Citizen(citizen) = &agent.kind;
        citizen
    });
    for (label, mut value) in &mut labels {
        let next = agents.get(label.0).map_or_else(String::new, |(id, agent)| {
            format!(
                "{}{}",
                agent.name,
                if Some(*id) == selection.0 { " *" } else { "" }
            )
        });
        if value.0 != next {
            value.0 = next;
        }
    }
    let mut occupied = Vec::new();
    let site_layout: Vec<_> = sites
        .iter()
        .map(|place| {
            if place.owner.is_some() && place.owner != selection.0 {
                return None;
            }
            let label = if let Some(owner) = place.owner {
                format!(
                    "{}'s\n{}",
                    universe.agents()[&owner].name,
                    place.kind.name()
                )
            } else {
                place.kind.name().to_string()
            };
            let position =
                site_label_position(viewport.project(place.position), &occupied, viewport.size);
            occupied.push(Rect::from_corners(
                position,
                position + Vec2::new(SITE_LABEL_WIDTH, SITE_LABEL_HEIGHT),
            ));
            Some((label, position))
        })
        .collect();
    for (label, mut value, mut node, mut visibility) in &mut site_labels {
        let Some(Some((name, position))) = site_layout.get(label.0) else {
            let next = Visibility::Hidden;
            if *visibility != next {
                *visibility = next;
            }
            if !value.0.is_empty() {
                value.0.clear();
            }
            continue;
        };
        let next = Visibility::Inherited;
        if *visibility != next {
            *visibility = next;
        }
        if &value.0 != name {
            value.0.clone_from(name);
        }
        let marker = viewport.project(sites[label.0].position) - Vec2::splat(5.0);
        let next = px(position.x - marker.x);
        if node.left != next {
            node.left = next;
        }
        let next = px(position.y - marker.y);
        if node.top != next {
            node.top = next;
        }
    }
    for (item, mut node, mut visibility, color) in &mut items {
        match item {
            MapItem::Site(slot) => {
                let Some(place) = sites.get(*slot) else {
                    let next = Visibility::Hidden;
                    if *visibility != next {
                        *visibility = next;
                    }
                    continue;
                };
                let next = Visibility::Inherited;
                if *visibility != next {
                    *visibility = next;
                }
                let p = viewport.project(place.position);
                let next = px(p.x - 5.0);
                if node.left != next {
                    node.left = next;
                }
                let next = px(p.y - 5.0);
                if node.top != next {
                    node.top = next;
                }
                if let Some(mut color) = color {
                    let next = if place.owner.is_some() && place.owner == selection.0 {
                        Color::srgb(1.0, 0.75, 0.20)
                    } else {
                        match place.kind {
                            Location::Warehouse => Color::srgb(0.45, 0.75, 0.95),
                            Location::Forest => Color::srgb(0.25, 0.70, 0.40),
                            Location::River => Color::srgb(0.30, 0.65, 0.95),
                            Location::Market => Color::srgb(0.80, 0.50, 0.80),
                            Location::Weavery => Color::srgb(0.55, 0.65, 0.85),
                            Location::Tailory => Color::srgb(0.85, 0.55, 0.65),
                            _ => Color::srgb(0.85, 0.75, 0.60),
                        }
                    };
                    if color.0 != next {
                        color.0 = next;
                    }
                }
            }
            MapItem::Citizen(slot) => {
                let next = if agents.get(*slot).is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *visibility != next {
                    *visibility = next;
                }
                if let Some((id, agent)) = agents.get(*slot) {
                    let AgentKind::Citizen(marker) = &agent.kind;
                    let p = citizen_point(viewport, &agents, *id, marker.position());
                    let next = px(p.x - 5.0);
                    if node.left != next {
                        node.left = next;
                    }
                    let next = px(p.y - 5.0);
                    if node.top != next {
                        node.top = next;
                    }
                    if let Some(mut color) = color {
                        let next = if Some(*id) == selection.0 {
                            Color::srgb(1.0, 0.75, 0.20)
                        } else {
                            Color::srgb(0.45, 0.68, 0.75)
                        };
                        if color.0 != next {
                            color.0 = next;
                        }
                    }
                }
            }
            MapItem::Route(slot) => {
                let mut next_visibility = Visibility::Hidden;
                if let Some(citizen) = citizen
                    && let Some(active) = citizen.active_action()
                    && let CitizenAction::Travel(destination) = active.action()
                {
                    let from = viewport.project(citizen.position());
                    let to = viewport.project(citizen.map().position(destination));
                    let delta = to - from;
                    let length = delta.length();
                    let intervals =
                        ((length / ROUTE_DOT_SPACING).ceil().max(1.0) as usize).min(ROUTE_DOTS - 1);
                    if *slot <= intervals {
                        let point = from + delta * (*slot as f32 / intervals as f32);
                        let next = px(point.x - 1.0);
                        if node.left != next {
                            node.left = next;
                        }
                        let next = px(point.y - 1.0);
                        if node.top != next {
                            node.top = next;
                        }
                        next_visibility = Visibility::Inherited;
                    }
                }
                visibility.set_if_neq(next_visibility);
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
    fn map_click_opens_mapped_citizen_and_capture_blocks_it() {
        let universe = crate::simulation::new_universe().unwrap();
        let agents = citizens::sorted_agents(&universe);
        let id = agents[0].0;
        let AgentKind::Citizen(citizen) = &agents[0].1.kind;
        let mut window = Window::default();
        let viewport = MapViewport {
            size: Vec2::new(window.width(), window.height()),
            ..default()
        };
        window.set_cursor_position(Some(citizen_point(
            &viewport,
            &agents,
            id,
            citizen.position(),
        )));
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe,
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
        }))
        .insert_resource(viewport)
        .init_resource::<Selection>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<crate::floating_ui::Windows>()
        .insert_resource(crate::floating_ui::InputCapture {
            blocked: true,
            ..default()
        })
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, interaction);
        app.world_mut().spawn((window, bevy::window::PrimaryWindow));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<crate::floating_ui::Windows>()
                .take_new()
                .is_empty()
        );
        app.world_mut()
            .resource_mut::<crate::floating_ui::InputCapture>()
            .blocked = false;
        app.update();
        assert_eq!(app.world().resource::<Selection>().0, Some(id));
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::floating_ui::Windows>()
                .take_new(),
            vec![crate::floating_ui::WindowKind::Citizen(id)]
        );
    }

    #[test]
    fn projection_preserves_distances_on_wide_and_tall_windows() {
        for size in [Vec2::new(1200.0, 600.0), Vec2::new(600.0, 1200.0)] {
            let viewport = MapViewport { size, ..default() };
            let origin = viewport.project(Position::default());
            assert_eq!(origin, size / 2.0);
            let east = viewport.project(Position { x: 100.0, y: 0.0 });
            let north = viewport.project(Position { x: 0.0, y: 100.0 });
            assert_eq!(east.distance(origin), north.distance(origin));
        }
    }

    #[test]
    fn zoom_keeps_pointer_world_position_and_pan_moves_every_point_equally() {
        let mut viewport = MapViewport {
            size: Vec2::new(1200.0, 600.0),
            ..default()
        };
        let position = Position {
            x: 300.0,
            y: -400.0,
        };
        let pointer = viewport.project(position);
        viewport.zoom_at(pointer, 2.0);
        assert!(viewport.project(position).distance(pointer) < 0.001);
        viewport.pan += Vec2::new(80.0, -30.0);
        assert!(
            viewport
                .project(position)
                .distance(pointer + Vec2::new(80.0, -30.0))
                < 0.001
        );
        viewport.zoom_at(pointer, 1000.0);
        assert_eq!(viewport.zoom, 12.0);
        viewport.zoom_at(pointer, 0.0001);
        assert_eq!(viewport.zoom, 0.5);
    }

    #[test]
    fn site_labels_stay_bounded_and_show_public_or_selected_properties() {
        for point in [
            Vec2::ZERO,
            Vec2::splat(MAP_SIZE),
            Vec2::new(0.0, MAP_SIZE),
            Vec2::new(MAP_SIZE, 0.0),
        ] {
            let position = site_label_position(point, &[], Vec2::splat(MAP_SIZE));
            assert!(position.x >= 0.0 && position.y >= 0.0);
            assert!(position.x + SITE_LABEL_WIDTH <= MAP_SIZE);
            assert!(position.y + SITE_LABEL_HEIGHT <= MAP_SIZE);
        }
        let point = Vec2::splat(150.0);
        let first = site_label_position(point, &[], Vec2::splat(MAP_SIZE));
        let occupied = Rect::from_corners(
            first,
            first + Vec2::new(SITE_LABEL_WIDTH, SITE_LABEL_HEIGHT),
        );
        assert_ne!(
            first,
            site_label_position(point, &[occupied], Vec2::splat(MAP_SIZE))
        );

        let universe = crate::simulation::new_universe().unwrap();
        let place_count = universe.map().places().len();
        let agents = citizens::sorted_agents(&universe);
        let ada = agents[0].0;
        let bram = agents[1].0;
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe,
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
        }))
        .insert_resource(Selection(Some(ada)))
        .add_systems(Update, refresh);
        let labels: Vec<_> = (0..place_count)
            .map(|slot| {
                app.world_mut()
                    .spawn((
                        SiteLabel(slot),
                        Text::new(""),
                        Node::default(),
                        Visibility::Inherited,
                    ))
                    .id()
            })
            .collect();
        let markers: Vec<_> = (0..place_count)
            .map(|slot| {
                app.world_mut()
                    .spawn((
                        MapItem::Site(slot),
                        Node::default(),
                        UiTransform::default(),
                        Visibility::Inherited,
                    ))
                    .id()
            })
            .collect();
        app.update();
        let visible_names = |app: &App| {
            labels
                .iter()
                .filter(|entity| {
                    app.world().get::<Visibility>(**entity) == Some(&Visibility::Inherited)
                })
                .map(|entity| app.world().get::<Text>(*entity).unwrap().0.clone())
                .collect::<Vec<_>>()
        };
        let names = visible_names(&app);
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"Ada's\nhome".into()));
        assert!(names.contains(&"Ada's\nfield".into()));
        for public in ["Forest", "River", "Market"] {
            assert!(names.contains(&public.into()));
        }
        app.world_mut().resource_mut::<Selection>().0 = Some(bram);
        app.update();
        let names = visible_names(&app);
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"Bram's\nhome".into()));
        assert!(names.contains(&"Bram's\nmill".into()));
        assert!(names.iter().all(|name| !name.starts_with("Ada")));
        for marker in markers {
            assert_eq!(
                app.world().get::<Visibility>(marker),
                Some(&Visibility::Inherited)
            );
        }
    }

    #[test]
    fn four_overlapping_markers_stay_distinct_and_highlight_selection() {
        let mut universe = Universe::with_map(Map::default());
        for name in ["Ada", "Bram", "Cleo", "Dara"] {
            universe = universe
                .with_citizen(name, Citizen::new(0.0).unwrap())
                .unwrap()
                .0;
        }
        let agents = citizens::sorted_agents(&universe);
        let selected = agents[2].0;
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe,
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
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
                        BackgroundColor(crate::PANEL),
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
        let (universe, id) = Universe::with_map(map.clone())
            .with_citizen("Ada", Citizen::new(0.0).unwrap())
            .unwrap();
        let travelling = universe
            .start_action(
                id,
                CitizenAction::Travel(map.public_place(Location::Forest)),
            )
            .unwrap();
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe: travelling.advance(150_000).unwrap(),
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
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
                MapItem::Route(0),
                Node::default(),
                UiTransform::default(),
                Visibility::Inherited,
            ))
            .id();
        let endpoint = app
            .world_mut()
            .spawn((
                MapItem::Route(10),
                Node::default(),
                UiTransform::default(),
                Visibility::Inherited,
            ))
            .id();
        let unused = app
            .world_mut()
            .spawn((
                MapItem::Route(11),
                Node::default(),
                UiTransform::default(),
                Visibility::Inherited,
            ))
            .id();
        let forest = app
            .world_mut()
            .spawn((
                MapItem::Site(1),
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
        assert_eq!(app.world().get::<Node>(route).unwrap().left, px(171.5));
        assert_eq!(app.world().get::<Node>(route).unwrap().top, px(119.0));
        assert_eq!(
            app.world().get::<UiTransform>(route).unwrap(),
            &UiTransform::default()
        );
        assert_eq!(app.world().get::<Node>(endpoint).unwrap().left, px(194.0));
        assert_eq!(app.world().get::<Node>(endpoint).unwrap().top, px(89.0));
        assert_eq!(
            app.world().get::<Visibility>(unused),
            Some(&Visibility::Hidden)
        );
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
