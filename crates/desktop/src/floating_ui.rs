use crate::{MUTED, PANEL, TEXT, text};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    window::{CursorIcon, SystemCursorIcon},
};
use std::collections::HashMap;

const TITLE_HEIGHT: f32 = 36.0;
const EDGE: f32 = 7.0;
const MIN_SIZE: Vec2 = Vec2::new(320.0, 240.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Citizens,
    Locations,
    Market,
}

impl WindowKind {
    pub fn minimum_size(self) -> Vec2 {
        Vec2::new(420.0, 280.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub position: Vec2,
    pub size: Vec2,
}

impl Bounds {
    pub fn constrain(self, viewport: Vec2) -> Self {
        self.constrain_minimum(viewport, MIN_SIZE)
    }

    pub fn constrain_minimum(self, viewport: Vec2, minimum: Vec2) -> Self {
        let size = self
            .size
            .clamp(minimum.min(viewport), viewport.max(Vec2::ONE));
        Self {
            position: self
                .position
                .clamp(Vec2::ZERO, (viewport - size).max(Vec2::ZERO)),
            size,
        }
    }

    fn contains(self, position: Vec2) -> bool {
        Rect::from_corners(self.position, self.position + self.size).contains(position)
    }
}

#[derive(Component)]
pub struct FloatingWindow(pub WindowKind);

#[derive(Component)]
pub(crate) struct Close(WindowKind);

#[derive(Component)]
pub struct Open(pub WindowKind);

#[derive(Component)]
pub struct Hud;

#[derive(Resource, Default)]
pub struct InputCapture {
    pub blocked: bool,
    pub hovered: Option<WindowKind>,
}

#[derive(Clone, Copy)]
struct Entry {
    entity: Entity,
    bounds: Bounds,
    order: i32,
    open: bool,
}

#[derive(Clone, Copy)]
struct Gesture {
    kind: WindowKind,
    start: Vec2,
    original: Bounds,
    edges: BVec4,
}

#[derive(Resource, Default)]
pub struct Windows {
    entries: HashMap<WindowKind, Entry>,
    pending: Vec<WindowKind>,
    order: i32,
    gesture: Option<Gesture>,
}

impl Windows {
    pub fn open(&mut self, kind: WindowKind) {
        if !self.pending.contains(&kind) {
            self.pending.push(kind);
        }
    }

    pub fn register(&mut self, kind: WindowKind, entity: Entity, bounds: Bounds) -> i32 {
        self.order += 1;
        self.entries.insert(
            kind,
            Entry {
                entity,
                bounds,
                order: self.order,
                open: true,
            },
        );
        self.order
    }

    pub fn requested_focus(&self) -> Option<WindowKind> {
        self.pending.last().copied()
    }

    pub fn take_new(&mut self) -> Vec<WindowKind> {
        let requests = std::mem::take(&mut self.pending);
        let mut new = Vec::new();
        for kind in requests {
            if self.entries.contains_key(&kind) {
                self.focus(kind);
            } else {
                new.push(kind);
            }
        }
        new
    }

    pub fn sync(
        &self,
        roots: &mut Query<(&mut Node, &mut GlobalZIndex, &mut Visibility), With<FloatingWindow>>,
    ) {
        for entry in self.entries.values() {
            if let Ok((mut node, mut z, mut visibility)) = roots.get_mut(entry.entity) {
                let next = Node {
                    left: px(entry.bounds.position.x),
                    top: px(entry.bounds.position.y),
                    width: px(entry.bounds.size.x),
                    height: px(entry.bounds.size.y),
                    display: if entry.open {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..node.clone()
                };
                node.set_if_neq(next);
                z.set_if_neq(GlobalZIndex(entry.order));
                visibility.set_if_neq(if entry.open {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }

    fn focus(&mut self, kind: WindowKind) {
        self.order += 1;
        if let Some(entry) = self.entries.get_mut(&kind) {
            entry.open = true;
            entry.order = self.order;
        }
    }
}

pub fn spawn_window(
    commands: &mut Commands,
    kind: WindowKind,
    title: &str,
    bounds: Bounds,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    commands
        .spawn((
            FloatingWindow(kind),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                left: px(bounds.position.x),
                top: px(bounds.position.y),
                width: px(bounds.size.x),
                height: px(bounds.size.y),

                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(Color::srgb(0.28, 0.36, 0.39)),
            GlobalZIndex(1),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    height: px(TITLE_HEIGHT),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.19, 0.22)),
            ))
            .with_children(|bar| {
                bar.spawn(text(title, 16.0, TEXT));
                bar.spawn((
                    Button,
                    Close(kind),
                    Node {
                        width: px(30),
                        height: px(28),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|button| {
                    button.spawn(text("X", 18.0, MUTED));
                });
            });
            root.spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_x(),
                    ..default()
                },
                ScrollPosition::default(),
                bevy::ui::RelativeCursorPosition::default(),
            ))
            .with_children(|viewport| {
                viewport
                    .spawn(Node {
                        width: percent(100),
                        height: percent(100),
                        min_width: px(720),
                        min_height: px(0),
                        padding: UiRect::all(px(10)),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .with_children(content);
            });
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: px(1),
                    bottom: px(1),
                    width: px(20),
                    height: px(20),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.19, 0.27, 0.30)),
            ))
            .with_children(|grip| {
                grip.spawn(text("//", 16.0, TEXT));
            });
        })
        .id()
}

fn resize_edges(bounds: Bounds, cursor: Vec2) -> BVec4 {
    let local = cursor - bounds.position;
    let corner = local.x >= bounds.size.x - 20.0 && local.y >= bounds.size.y - 20.0;
    BVec4::new(
        local.x <= EDGE,
        local.x >= bounds.size.x - EDGE || corner,
        local.y <= EDGE,
        local.y >= bounds.size.y - EDGE || corner,
    )
}

fn resize_cursor(edges: BVec4) -> SystemCursorIcon {
    match (edges.x || edges.y, edges.z || edges.w) {
        (true, true) if edges.x == edges.z => SystemCursorIcon::NwseResize,
        (true, true) => SystemCursorIcon::NeswResize,
        (true, false) => SystemCursorIcon::EwResize,
        (false, true) => SystemCursorIcon::NsResize,
        _ => SystemCursorIcon::Default,
    }
}

fn resize(original: Bounds, delta: Vec2, edges: BVec4, viewport: Vec2, minimum: Vec2) -> Bounds {
    let mut min = original.position;
    let mut max = min + original.size;
    if edges.x {
        min.x = (min.x + delta.x).clamp(0.0, max.x - minimum.x.min(viewport.x));
    }
    if edges.y {
        max.x = (max.x + delta.x).clamp(min.x + minimum.x.min(viewport.x), viewport.x);
    }
    if edges.z {
        min.y = (min.y + delta.y).clamp(0.0, max.y - minimum.y.min(viewport.y));
    }
    if edges.w {
        max.y = (max.y + delta.y).clamp(min.y + minimum.y.min(viewport.y), viewport.y);
    }
    Bounds {
        position: min,
        size: max - min,
    }
    .constrain_minimum(viewport, minimum)
}

#[derive(SystemParam)]
pub(crate) struct EditorFocus<'w> {
    focus: Option<ResMut<'w, bevy::input_focus::InputFocus>>,
    locations: Option<ResMut<'w, crate::locations::State>>,
    caravan: Option<ResMut<'w, crate::caravan_controls::State>>,
    market: Option<ResMut<'w, crate::market::Selection>>,
}

impl EditorFocus<'_> {
    pub fn activate(&mut self, kind: WindowKind) {
        if kind != WindowKind::Locations
            && let Some(state) = self.locations.as_mut()
        {
            crate::locations::stop_editing(state);
        }
        if kind != WindowKind::Market
            && let Some(state) = self.caravan.as_mut()
        {
            crate::caravan_controls::stop_editing(state);
        }
        if let Some(selection) = self.market.as_mut() {
            selection.view = match kind {
                WindowKind::Citizens => crate::market::View::Citizens,
                WindowKind::Locations => crate::market::View::Locations,
                WindowKind::Market => crate::market::View::Market,
            };
        }
    }

    fn close(&mut self, kind: WindowKind) {
        if kind == WindowKind::Locations
            && let Some(state) = self.locations.as_mut()
        {
            crate::locations::stop_editing(state);
        }
        if kind == WindowKind::Market
            && let Some(state) = self.caravan.as_mut()
        {
            crate::caravan_controls::stop_editing(state);
        }
        if let Some(focus) = self.focus.as_mut() {
            focus.clear();
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct WindowInput<'w, 's> {
    primary: Query<
        'w,
        's,
        (Entity, &'static Window, Option<&'static CursorIcon>),
        With<bevy::window::PrimaryWindow>,
    >,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    buttons: Query<'w, 's, (&'static Interaction, &'static Open), Changed<Interaction>>,
    close: Query<'w, 's, (&'static Interaction, &'static Close), Changed<Interaction>>,
    hud: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static InheritedVisibility,
        ),
        With<Hud>,
    >,
}

pub fn interact(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    mut capture: ResMut<InputCapture>,
    input: WindowInput,
    mut editors: EditorFocus,
    mut roots: Query<(&mut Node, &mut GlobalZIndex, &mut Visibility), With<FloatingWindow>>,
) {
    let Ok((window_entity, window, cursor_icon)) = input.primary.single() else {
        return;
    };
    let viewport = Vec2::new(window.width(), window.height());
    for (interaction, open) in &input.buttons {
        if *interaction == Interaction::Pressed {
            windows.open(open.0);
        }
    }
    for (interaction, close) in &input.close {
        if *interaction == Interaction::Pressed {
            if let Some(entry) = windows.entries.get_mut(&close.0) {
                entry.open = false;
            }
            editors.close(close.0);
        }
    }
    for (kind, entry) in &mut windows.entries {
        entry.bounds = entry
            .bounds
            .constrain_minimum(viewport, kind.minimum_size());
    }
    let cursor = window.cursor_position();
    let hud_hit = cursor.is_some_and(|cursor| {
        input.hud.iter().any(|(computed, transform, visibility)| {
            visibility.get()
                && Rect::from_center_size(
                    transform.translation * computed.inverse_scale_factor(),
                    computed.size() * computed.inverse_scale_factor(),
                )
                .contains(cursor)
        })
    });
    let hit = cursor.and_then(|position| {
        windows
            .entries
            .iter()
            .filter(|(_, entry)| entry.open && entry.bounds.contains(position))
            .max_by_key(|(_, entry)| entry.order)
            .map(|(kind, _)| *kind)
    });
    capture.blocked = hud_hit || hit.is_some() || windows.gesture.is_some();
    capture.hovered = if hud_hit { None } else { hit };
    if input.mouse.just_pressed(MouseButton::Left)
        && !hud_hit
        && let (Some(kind), Some(cursor)) = (hit, cursor)
    {
        windows.focus(kind);
        editors.activate(kind);
        let bounds = windows.entries[&kind].bounds;
        let local = cursor - bounds.position;
        let edges = resize_edges(bounds, cursor);
        if edges.any() || local.y < TITLE_HEIGHT && local.x < bounds.size.x - 44.0 {
            windows.gesture = Some(Gesture {
                kind,
                start: cursor,
                original: bounds,
                edges,
            });
        }
    }
    if !input.mouse.pressed(MouseButton::Left) {
        windows.gesture = None;
    }
    if let (Some(gesture), Some(cursor)) = (windows.gesture, cursor) {
        let delta = cursor - gesture.start;
        let bounds = if gesture.edges.any() {
            resize(
                gesture.original,
                delta,
                gesture.edges,
                viewport,
                gesture.kind.minimum_size(),
            )
        } else {
            Bounds {
                position: gesture.original.position + delta,
                ..gesture.original
            }
            .constrain(viewport)
        };
        if let Some(entry) = windows.entries.get_mut(&gesture.kind) {
            entry.bounds = bounds;
        }
    }
    let edges = windows
        .gesture
        .map(|gesture| gesture.edges)
        .or_else(|| {
            let kind = capture.hovered?;
            Some(resize_edges(windows.entries[&kind].bounds, cursor?))
        })
        .unwrap_or_default();
    let icon = CursorIcon::System(resize_cursor(edges));
    if cursor_icon != Some(&icon) {
        commands.entity(window_entity).insert(icon);
    }
    windows.sync(&mut roots);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_and_resize_stay_inside_viewport() {
        let viewport = Vec2::new(1280.0, 800.0);
        let original = Bounds {
            position: Vec2::new(100.0, 100.0),
            size: Vec2::new(600.0, 400.0),
        };
        assert_eq!(
            Bounds {
                position: Vec2::new(-50.0, 900.0),
                ..original
            }
            .constrain(viewport)
            .position,
            Vec2::new(0.0, 400.0)
        );
        let resized = resize(
            original,
            Vec2::splat(1000.0),
            BVec4::new(true, false, true, false),
            viewport,
            MIN_SIZE,
        );
        assert_eq!(resized.size, MIN_SIZE);
        let resized = resize(
            original,
            Vec2::splat(1000.0),
            BVec4::new(false, true, false, true),
            viewport,
            MIN_SIZE,
        );
        assert_eq!(resized.position + resized.size, viewport);
    }

    #[test]
    fn category_windows_shrink_and_corner_grip_matches_resize_cursor() {
        let original = Bounds {
            position: Vec2::new(100.0, 100.0),
            size: Vec2::new(960.0, 600.0),
        };
        let cursor = original.position + original.size - Vec2::splat(15.0);
        let edges = resize_edges(original, cursor);
        assert_eq!(resize_cursor(edges), SystemCursorIcon::NwseResize);
        for kind in [
            WindowKind::Citizens,
            WindowKind::Locations,
            WindowKind::Market,
        ] {
            let small = resize(
                original,
                Vec2::splat(-1000.0),
                edges,
                Vec2::new(1280.0, 800.0),
                kind.minimum_size(),
            );
            assert_eq!(small.size, Vec2::new(420.0, 280.0));
            assert_eq!(small.position, original.position);
        }
    }

    #[test]
    fn reopening_focuses_the_existing_window_and_remembers_bounds() {
        let mut windows = Windows::default();
        let bounds = Bounds {
            position: Vec2::new(220.0, 120.0),
            size: Vec2::new(600.0, 400.0),
        };
        windows.register(WindowKind::Market, Entity::PLACEHOLDER, bounds);
        windows.entries.get_mut(&WindowKind::Market).unwrap().open = false;
        windows.focus(WindowKind::Market);
        let entry = windows.entries[&WindowKind::Market];
        assert!(entry.open);
        assert_eq!(entry.bounds, bounds);
        windows.open(WindowKind::Market);
        assert!(windows.take_new().is_empty());
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    fn pointer(app: &mut App, position: Vec2) {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
        query
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(position));
    }

    fn frame(app: &mut App) {
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
    }

    #[test]
    fn input_focus_drag_resize_close_and_reopen_use_the_same_bounds() {
        let mut app = App::new();
        app.init_resource::<Windows>()
            .init_resource::<InputCapture>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<bevy::input_focus::InputFocus>()
            .add_systems(Update, interact);
        app.world_mut().spawn((
            Window {
                resolution: (1280, 800).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        let lower = app
            .world_mut()
            .spawn((
                FloatingWindow(WindowKind::Citizens),
                Node::default(),
                GlobalZIndex(1),
                Visibility::Inherited,
            ))
            .id();
        let upper = app
            .world_mut()
            .spawn((
                FloatingWindow(WindowKind::Market),
                Node::default(),
                GlobalZIndex(2),
                Visibility::Inherited,
            ))
            .id();
        app.world_mut().resource_mut::<Windows>().register(
            WindowKind::Citizens,
            lower,
            Bounds {
                position: Vec2::new(100.0, 100.0),
                size: Vec2::new(500.0, 400.0),
            },
        );
        app.world_mut().resource_mut::<Windows>().register(
            WindowKind::Market,
            upper,
            Bounds {
                position: Vec2::new(200.0, 180.0),
                size: Vec2::new(500.0, 400.0),
            },
        );
        pointer(&mut app, Vec2::new(300.0, 300.0));
        frame(&mut app);
        assert_eq!(
            app.world().resource::<InputCapture>().hovered,
            Some(WindowKind::Market)
        );
        assert!(app.world().resource::<InputCapture>().blocked);
        pointer(&mut app, Vec2::new(150.0, 300.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        frame(&mut app);
        assert!(
            app.world().get::<GlobalZIndex>(lower).unwrap().0
                > app.world().get::<GlobalZIndex>(upper).unwrap().0
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        frame(&mut app);
        pointer(&mut app, Vec2::new(150.0, 115.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        frame(&mut app);
        pointer(&mut app, Vec2::new(250.0, 165.0));
        frame(&mut app);
        assert_eq!(
            app.world().resource::<Windows>().entries[&WindowKind::Citizens]
                .bounds
                .position,
            Vec2::new(200.0, 150.0)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        frame(&mut app);
        pointer(&mut app, Vec2::new(698.0, 548.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        frame(&mut app);
        pointer(&mut app, Vec2::new(798.0, 648.0));
        frame(&mut app);
        let remembered = app.world().resource::<Windows>().entries[&WindowKind::Citizens].bounds;
        assert_eq!(remembered.size, Vec2::new(600.0, 500.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        frame(&mut app);
        let close = app
            .world_mut()
            .spawn((Close(WindowKind::Citizens), Interaction::Pressed))
            .id();
        frame(&mut app);
        assert_eq!(
            app.world().get::<Node>(lower).unwrap().display,
            Display::None
        );
        app.world_mut().entity_mut(close).insert(Interaction::None);
        app.world_mut()
            .resource_mut::<Windows>()
            .open(WindowKind::Citizens);
        assert!(
            app.world_mut()
                .resource_mut::<Windows>()
                .take_new()
                .is_empty()
        );
        frame(&mut app);
        assert_eq!(
            app.world().get::<Node>(lower).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().resource::<Windows>().entries[&WindowKind::Citizens].bounds,
            remembered
        );
        pointer(&mut app, Vec2::new(1100.0, 750.0));
        frame(&mut app);
        assert!(!app.world().resource::<InputCapture>().blocked);
    }
}
