use crate::{MUTED, PANEL, TEXT, locations, text};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    window::{CursorIcon, SystemCursorIcon},
};
use learning_lord_simulation::{AgentId, locations::PlaceId, marketplace::Good};
use std::collections::HashMap;

const HUD_HEIGHT: f32 = 32.0;
const TITLE_HEIGHT: f32 = 28.0;
const RAIL_WIDTH: f32 = 144.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowKind {
    Map,
    Citizens,
    Collectives,
    Locations,
    Market,
    Saves,
    Citizen(AgentId),
    Household(AgentId),
    Guild(AgentId),
    Location(PlaceId),
    Good(Good),
    Carpentry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub position: Vec2,
    pub size: Vec2,
}

impl Bounds {
    fn contains(self, position: Vec2) -> bool {
        Rect::from_corners(self.position, self.position + self.size).contains(position)
    }

    fn below(self, height: f32) -> Self {
        let height = height.min(self.size.y);
        Self {
            position: self.position + Vec2::Y * height,
            size: self.size - Vec2::Y * height,
        }
    }
}

#[derive(Component)]
pub struct FloatingWindow(pub WindowKind);

#[derive(Component)]
pub struct Open(pub WindowKind);

#[derive(Component)]
pub struct Hud;

#[derive(Component)]
pub(crate) struct MapTitle;

#[derive(Component)]
pub(crate) struct PaneTitle(pub WindowKind);

#[derive(Resource, Default)]
pub struct InputCapture {
    pub blocked: bool,
    pub hovered: Option<WindowKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Close(WindowKind),
    Minimize(WindowKind),
    RestoreManual(WindowKind),
    RestoreDisplaced(WindowKind),
    Dismiss(WindowKind),
    Clear,
}

#[derive(Component)]
pub(crate) struct WorkspaceButton(pub Action);

#[derive(Resource, Default)]
pub struct Windows {
    left: Option<WindowKind>,
    right: Option<WindowKind>,
    manual: Vec<WindowKind>,
    displaced: Vec<(Slot, WindowKind)>,
    entries: HashMap<WindowKind, Entity>,
    viewport: Vec2,
    requested_focus: Option<WindowKind>,
    selected_household: Option<AgentId>,
    focused: Option<WindowKind>,
}

impl Windows {
    pub(crate) fn implicit_map(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }

    pub(crate) fn visible(&self) -> [Option<WindowKind>; 2] {
        if self.implicit_map() {
            return [Some(WindowKind::Map), None];
        }
        [self.left, self.right]
    }

    pub(crate) fn is_open(&self, kind: WindowKind) -> bool {
        self.visible().contains(&Some(kind))
    }

    fn remove_parked(&mut self, kind: WindowKind) {
        self.manual.retain(|entry| *entry != kind);
        self.displaced.retain(|(_, entry)| *entry != kind);
    }

    fn pop_active(&mut self, kind: WindowKind) -> Option<Slot> {
        if self.left == Some(kind) {
            self.left = None;
            Some(Slot::Left)
        } else if self.right == Some(kind) {
            self.right = None;
            Some(Slot::Right)
        } else {
            None
        }
    }

    fn place(&mut self, slot: Slot, kind: WindowKind) {
        let active = match slot {
            Slot::Left => &mut self.left,
            Slot::Right => &mut self.right,
        };
        if let Some(previous) = active.replace(kind) {
            self.displaced.push((slot, previous));
        }
    }

    pub fn open(&mut self, kind: WindowKind) {
        self.requested_focus = Some(kind);
        if self.left == Some(kind) {
            return;
        }
        self.remove_parked(kind);
        self.pop_active(kind);
        if let Some(previous) = self.left.take() {
            self.place(Slot::Right, previous);
        }
        self.left = Some(kind);
    }

    pub fn open_link(&mut self, source: Option<WindowKind>, destination: WindowKind) {
        if let Some(source) = source.filter(|source| self.is_open(*source)) {
            self.link(source, destination);
        } else {
            self.open(destination);
        }
    }

    pub fn link(&mut self, source: WindowKind, destination: WindowKind) {
        if !self.is_open(source) {
            self.open(destination);
            return;
        }
        self.requested_focus = Some(destination);
        if source == destination {
            return;
        }
        for kind in [source, destination] {
            self.remove_parked(kind);
            self.pop_active(kind);
        }
        self.place(Slot::Left, source);
        self.place(Slot::Right, destination);
    }

    fn remove_active(&mut self, kind: WindowKind, minimize: bool) {
        let Some(slot) = self.pop_active(kind) else {
            return;
        };
        if minimize {
            self.manual.push(kind);
        }
        if let Some((_, previous)) = self.displaced.pop() {
            self.place(slot, previous);
        }
        self.requested_focus = Some(self.left.or(self.right).unwrap_or(WindowKind::Map));
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Close(kind) => self.remove_active(kind, false),
            Action::Minimize(kind) => self.remove_active(kind, true),
            Action::RestoreManual(kind) => self.open(kind),
            Action::RestoreDisplaced(kind) => {
                if let Some(&(slot, _)) = self.displaced.iter().find(|(_, entry)| *entry == kind) {
                    self.remove_parked(kind);
                    self.place(slot, kind);
                    self.requested_focus = Some(kind);
                }
            }
            Action::Dismiss(kind) => self.displaced.retain(|(_, entry)| *entry != kind),
            Action::Clear => self.displaced.clear(),
        }
    }

    pub(crate) fn manual(&self) -> impl Iterator<Item = WindowKind> + '_ {
        self.manual.iter().rev().copied()
    }

    pub(crate) fn displaced(&self) -> impl Iterator<Item = WindowKind> + '_ {
        self.displaced.iter().rev().map(|(_, kind)| *kind)
    }

    pub(crate) fn set_viewport(&mut self, viewport: Vec2) {
        self.viewport = viewport;
    }

    fn workspace_bounds(&self) -> Bounds {
        Bounds {
            position: Vec2::new(0.0, HUD_HEIGHT),
            size: Vec2::new(
                self.viewport.x,
                (self.viewport.y - HUD_HEIGHT * 2.0).max(0.0),
            ),
        }
    }

    fn rail_width(&self, slot: Slot) -> f32 {
        let present = match slot {
            Slot::Left => !self.manual.is_empty(),
            Slot::Right => !self.displaced.is_empty(),
        };
        if present {
            RAIL_WIDTH.min(self.viewport.x / 3.0)
        } else {
            0.0
        }
    }

    pub(crate) fn pane_bounds(&self, kind: WindowKind) -> Option<Bounds> {
        if !self.is_open(kind) {
            return None;
        }
        let mut bounds = self.workspace_bounds();
        let left_rail = self.rail_width(Slot::Left);
        bounds.position.x += left_rail;
        bounds.size.x -= left_rail + self.rail_width(Slot::Right);
        if self.left.is_some() && self.right.is_some() {
            let left_width = (bounds.size.x / 2.0).floor();
            if self.right == Some(kind) {
                bounds.position.x += left_width;
                bounds.size.x -= left_width;
            } else {
                bounds.size.x = left_width;
            }
        }
        Some(bounds)
    }

    pub(crate) fn bounds(&self, kind: WindowKind) -> Option<Bounds> {
        self.pane_bounds(kind).map(|bounds| {
            if kind == WindowKind::Map && !self.implicit_map() {
                bounds.below(TITLE_HEIGHT)
            } else {
                bounds
            }
        })
    }

    pub(crate) fn register(&mut self, kind: WindowKind, entity: Entity) {
        self.entries.insert(kind, entity);
    }

    pub(crate) fn take_focus(&mut self) -> Option<WindowKind> {
        let kind = self.requested_focus.take()?;
        self.focus(kind);
        Some(kind)
    }

    pub(crate) fn focused(&self) -> WindowKind {
        self.focused.unwrap_or(WindowKind::Map)
    }

    fn focus(&mut self, kind: WindowKind) {
        self.focused = Some(kind);
        if let WindowKind::Household(id) = kind {
            self.selected_household = Some(id);
        }
    }

    pub(crate) fn selected_household(&self) -> Option<AgentId> {
        self.selected_household
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(WindowKind) -> bool) {
        self.manual.retain(|&kind| keep(kind));
        self.displaced.retain(|&(_, kind)| keep(kind));
        for kind in [self.left, self.right].into_iter().flatten() {
            if !keep(kind) {
                self.remove_active(kind, false);
            }
        }
        self.requested_focus = self.requested_focus.filter(|&kind| keep(kind));
        self.selected_household = self
            .selected_household
            .filter(|&id| keep(WindowKind::Household(id)));
    }

    pub(crate) fn tracked(&self, kind: WindowKind) -> bool {
        self.is_open(kind)
            || self.manual.contains(&kind)
            || self.displaced.iter().any(|&(_, entry)| entry == kind)
    }

    pub(crate) fn take_inactive_entries(&mut self) -> Vec<(WindowKind, Entity)> {
        let inactive = self
            .entries
            .iter()
            .filter_map(|(&kind, &entity)| (!self.tracked(kind)).then_some((kind, entity)))
            .collect::<Vec<_>>();
        for (kind, _) in &inactive {
            self.entries.remove(kind);
        }
        inactive
    }

    pub(crate) fn missing(&self) -> Vec<WindowKind> {
        self.visible()
            .into_iter()
            .flatten()
            .filter(|kind| !self.entries.contains_key(kind))
            .collect()
    }

    pub(crate) fn sync(
        &self,
        roots: &mut Query<(&mut Node, &mut Visibility), With<FloatingWindow>>,
        titles: &mut Query<&mut Node, (With<MapTitle>, Without<FloatingWindow>)>,
    ) {
        for (kind, entity) in &self.entries {
            if let Ok((mut node, mut visibility)) = roots.get_mut(*entity) {
                let bounds = self.pane_bounds(*kind);
                if let Some(bounds) = bounds {
                    node.set_if_neq(window_node(*kind, bounds));
                } else {
                    node.map_unchanged(|node| &mut node.display)
                        .set_if_neq(Display::None);
                }
                visibility.set_if_neq(if bounds.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
        for node in titles.iter_mut() {
            let display = if self.implicit_map() {
                Display::None
            } else {
                Display::Flex
            };
            node.map_unchanged(|node| &mut node.display)
                .set_if_neq(display);
        }
    }
}

fn window_node(kind: WindowKind, bounds: Bounds) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(bounds.position.x),
        top: px(bounds.position.y),
        width: px(bounds.size.x),
        height: px(bounds.size.y),
        flex_direction: FlexDirection::Column,
        border: UiRect::all(px(if kind == WindowKind::Map { 0 } else { 1 })),
        overflow: Overflow::clip(),
        ..default()
    }
}

pub fn spawn_window(
    commands: &mut Commands,
    kind: WindowKind,
    title: &str,
    bounds: Bounds,
    implicit_map: bool,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    commands
        .spawn((
            FloatingWindow(kind),
            window_node(kind, bounds),
            BackgroundColor(if kind == WindowKind::Map {
                Color::NONE
            } else {
                PANEL
            }),
            BorderColor::all(Color::srgb(0.28, 0.36, 0.39)),
            GlobalZIndex(1),
            Visibility::Inherited,
        ))
        .with_children(|root| {
            let mut bar = root.spawn((
                Node {
                    width: percent(100),
                    height: px(TITLE_HEIGHT),
                    display: if implicit_map {
                        Display::None
                    } else {
                        Display::Flex
                    },
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.19, 0.22)),
            ));
            if kind == WindowKind::Map {
                bar.insert(MapTitle);
            }
            bar.with_children(|bar| {
                bar.spawn(Node {
                    min_width: px(0),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_child((
                    text(title, 16.0, TEXT),
                    TextLayout::no_wrap(),
                    PaneTitle(kind),
                ));
                bar.spawn(Node {
                    flex_shrink: 0.0,
                    height: percent(100),
                    ..default()
                })
                .with_children(|buttons| {
                    action_button(buttons, Action::Minimize(kind), "_");
                    action_button(buttons, Action::Close(kind), "X");
                });
            });
            if kind == WindowKind::Map {
                return;
            }
            root.spawn((
                bevy::ui::FocusPolicy::Block,
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
                        min_width: if matches!(
                            kind,
                            WindowKind::Market
                                | WindowKind::Locations
                                | WindowKind::Household(_)
                                | WindowKind::Guild(_)
                                | WindowKind::Location(_)
                        ) {
                            px(720)
                        } else {
                            px(0)
                        },
                        min_height: px(0),
                        padding: UiRect::all(px(10)),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .with_children(content);
            });
        })
        .id()
}

fn action_button(parent: &mut ChildSpawnerCommands, action: Action, label: &str) {
    parent
        .spawn((
            Button,
            WorkspaceButton(action),
            Node {
                min_width: px(26),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|button| {
            button.spawn(text(label, 14.0, MUTED));
        });
}

pub(crate) fn spawn_rail(
    commands: &mut Commands,
    kinds: &[(WindowKind, String)],
    automatic: bool,
    scroll: ScrollPosition,
) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(HUD_HEIGHT),
                bottom: px(HUD_HEIGHT),
                left: if automatic { Val::Auto } else { px(0) },
                right: if automatic { px(0) } else { Val::Auto },
                width: px(RAIL_WIDTH),
                max_width: percent(100.0 / 3.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                row_gap: px(3),
                padding: UiRect::all(px(4)),
                ..default()
            },
            BackgroundColor(PANEL),
            GlobalZIndex(1),
            scroll,
            bevy::ui::RelativeCursorPosition::default(),
        ))
        .with_children(|rail| {
            if automatic {
                action_button(rail, Action::Clear, "Clear");
            }
            for (kind, label) in kinds {
                let kind = *kind;
                rail.spawn((
                    Node {
                        min_height: px(26),
                        flex_shrink: 0.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.19, 0.22)),
                ))
                .with_children(|item| {
                    let action = if automatic {
                        Action::RestoreDisplaced(kind)
                    } else {
                        Action::RestoreManual(kind)
                    };
                    item.spawn((
                        Button,
                        WorkspaceButton(action),
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            padding: UiRect::horizontal(px(5)),
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|button| {
                        button.spawn((text(label, 14.0, TEXT), TextLayout::no_wrap()));
                    });
                    if automatic {
                        action_button(item, Action::Dismiss(kind), "X");
                    }
                });
            }
        })
        .id()
}

#[derive(SystemParam)]
pub(crate) struct EditorFocus<'w> {
    focus: Option<ResMut<'w, bevy::input_focus::InputFocus>>,
    citizen: Option<ResMut<'w, crate::citizens::Selection>>,
    dev: Option<ResMut<'w, crate::dev_controls::State>>,
    locations: Option<ResMut<'w, crate::locations::State>>,
    caravan: Option<ResMut<'w, crate::caravan_controls::State>>,
}

impl EditorFocus<'_> {
    pub fn activate(&mut self, kind: WindowKind) {
        if let Some(dev) = self.dev.as_mut() {
            dev.stop_editing();
        }
        if let WindowKind::Citizen(id) = kind
            && let Some(selection) = self.citizen.as_mut()
            && selection.0 != Some(id)
        {
            selection.0 = Some(id);
        }
        if let Some(state) = self.locations.as_mut() {
            match kind {
                WindowKind::Locations => locations::focus_place(state, None),
                WindowKind::Location(place) => locations::focus_place(state, Some(place)),
                _ => locations::stop_editing(state),
            }
        }
        if let Some(state) = self.caravan.as_mut() {
            crate::caravan_controls::focus_pane(state, kind);
        }
    }

    pub(crate) fn retain_citizen(&mut self, keep: impl FnOnce(AgentId) -> bool) {
        if let Some(selection) = self.citizen.as_mut()
            && selection.0.is_some_and(|id| !keep(id))
        {
            selection.0 = None;
        }
    }

    fn close(&mut self, kind: WindowKind) {
        if matches!(kind, WindowKind::Locations | WindowKind::Location(_))
            && let Some(state) = self.locations.as_mut()
        {
            crate::locations::stop_editing(state);
        }
        if matches!(kind, WindowKind::Market | WindowKind::Good(_))
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
    links: Query<
        'w,
        's,
        (Entity, &'static Interaction, &'static InheritedVisibility),
        With<crate::navigation::Target>,
    >,
    parents: Query<'w, 's, &'static ChildOf>,
    roots: Query<'w, 's, &'static FloatingWindow>,
    open: Query<'w, 's, (&'static Interaction, &'static Open), Changed<Interaction>>,
    actions: Query<'w, 's, (&'static Interaction, &'static WorkspaceButton), Changed<Interaction>>,
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
) {
    let Ok((window_entity, window, cursor_icon)) = input.primary.single() else {
        return;
    };
    let viewport = Vec2::new(window.width(), window.height());
    windows.set_viewport(viewport);
    let cursor = window.cursor_position();
    let hud_hit = cursor.is_some_and(|cursor| {
        cursor.y < HUD_HEIGHT
            || cursor.y >= viewport.y - HUD_HEIGHT
            || input.hud.iter().any(|(computed, transform, visibility)| {
                visibility.get()
                    && Rect::from_center_size(
                        transform.translation * computed.inverse_scale_factor(),
                        computed.size() * computed.inverse_scale_factor(),
                    )
                    .contains(cursor)
            })
    });
    let rail_hit = cursor.is_some_and(|position| {
        position.x < windows.rail_width(Slot::Left)
            || position.x >= viewport.x - windows.rail_width(Slot::Right)
    });
    let hit = cursor
        .filter(|_| !hud_hit && !rail_hit)
        .and_then(|position| {
            windows.visible().into_iter().rev().flatten().find(|&kind| {
                windows
                    .pane_bounds(kind)
                    .is_some_and(|bounds| bounds.contains(position))
            })
        });
    let map_hit = hit == Some(WindowKind::Map)
        && cursor.is_some_and(|position| {
            windows
                .bounds(WindowKind::Map)
                .is_some_and(|bounds| bounds.contains(position))
        });
    capture.blocked = hud_hit || rail_hit || (hit.is_some() && !map_hit);
    capture.hovered = hit;
    if input.mouse.just_pressed(MouseButton::Left)
        && let Some(kind) = hit
    {
        editors.activate(kind);
        windows.focus(kind);
    }
    for (interaction, button) in &input.actions {
        if *interaction == Interaction::Pressed {
            if let Action::Close(kind) | Action::Minimize(kind) = button.0
                && (windows.left == Some(kind) || windows.right == Some(kind))
            {
                editors.close(kind);
            }
            windows.apply(button.0);
        }
    }
    for (interaction, open) in &input.open {
        if *interaction == Interaction::Pressed {
            windows.open(open.0);
        }
    }
    let pointer = !hud_hit
        && input.links.iter().any(|(entity, interaction, visibility)| {
            visibility.get()
                && *interaction != Interaction::None
                && crate::navigation::window_kind(entity, &input.parents, &input.roots)
                    == capture.hovered
        });
    let icon = CursorIcon::System(if pointer {
        SystemCursorIcon::Pointer
    } else {
        SystemCursorIcon::Default
    });
    if cursor_icon != Some(&icon) {
        commands.entity(window_entity).insert(icon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_open_replaces_implicit_map_and_shifts_left_to_right() {
        let mut windows = Windows::default();
        windows.set_viewport(Vec2::new(801.0, 600.0));
        assert_eq!(windows.visible(), [Some(WindowKind::Map), None]);
        assert_eq!(windows.bounds(WindowKind::Map).unwrap().size.x, 801.0);

        windows.open(WindowKind::Citizens);
        assert_eq!(windows.visible(), [Some(WindowKind::Citizens), None]);
        windows.open(WindowKind::Market);
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Market), Some(WindowKind::Citizens)]
        );
        let left = windows.bounds(WindowKind::Market).unwrap();
        let right = windows.bounds(WindowKind::Citizens).unwrap();
        assert_eq!(left.size.x, 400.0);
        assert_eq!(right.position.x, 400.0);
        assert_eq!(right.size.x, 401.0);
        assert_eq!(left.position.y, HUD_HEIGHT);
        assert_eq!(left.size.y, 600.0 - 2.0 * HUD_HEIGHT);
        windows.open(WindowKind::Locations);
        assert_eq!(
            windows.displaced().collect::<Vec<_>>(),
            [WindowKind::Citizens]
        );
        windows.open(WindowKind::Citizens);
        assert_eq!(
            windows.displaced().collect::<Vec<_>>(),
            [WindowKind::Market]
        );
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Citizens), Some(WindowKind::Locations)]
        );
    }

    #[test]
    fn right_source_moves_left_and_displaced_pane_restores_to_its_slot() {
        let mut windows = Windows::default();
        windows.open(WindowKind::Citizens);
        windows.link(WindowKind::Citizens, WindowKind::Market);
        windows.link(WindowKind::Market, WindowKind::Locations);
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Market), Some(WindowKind::Locations)]
        );
        windows.apply(Action::RestoreDisplaced(WindowKind::Citizens));
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Citizens), Some(WindowKind::Locations)]
        );
        assert_eq!(
            windows.displaced().collect::<Vec<_>>(),
            [WindowKind::Market]
        );
        windows.link(WindowKind::Locations, WindowKind::Citizens);
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Locations), Some(WindowKind::Citizens)]
        );
        windows.link(WindowKind::Citizens, WindowKind::Citizens);
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Locations), Some(WindowKind::Citizens)]
        );
    }

    #[test]
    fn vacancies_restore_latest_automatic_and_manual_restore_uses_menu_order() {
        let mut windows = Windows::default();
        windows.open(WindowKind::Citizens);
        windows.link(WindowKind::Citizens, WindowKind::Market);
        windows.link(WindowKind::Citizens, WindowKind::Locations);
        windows.link(WindowKind::Citizens, WindowKind::Collectives);
        windows.apply(Action::Minimize(WindowKind::Citizens));
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Locations), Some(WindowKind::Collectives)]
        );
        assert_eq!(windows.manual().collect::<Vec<_>>(), [WindowKind::Citizens]);
        windows.apply(Action::Dismiss(WindowKind::Market));
        windows.apply(Action::Close(WindowKind::Collectives));
        assert_eq!(windows.visible(), [Some(WindowKind::Locations), None]);
        windows.apply(Action::RestoreManual(WindowKind::Citizens));
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Citizens), Some(WindowKind::Locations)]
        );
        assert!(windows.manual().next().is_none());
        windows.open(WindowKind::Market);
        assert_eq!(
            windows.displaced().collect::<Vec<_>>(),
            [WindowKind::Locations]
        );
        windows.apply(Action::Clear);
        windows.apply(Action::Close(WindowKind::Citizens));
        windows.apply(Action::Close(WindowKind::Market));
        assert_eq!(windows.visible(), [Some(WindowKind::Map), None]);
    }

    #[test]
    fn close_and_minimize_restore_the_latest_displaced_pane_into_the_vacated_slot() {
        use WindowKind::{Citizens, Collectives, Locations, Market};
        for (kind, expected) in [
            (Citizens, [Some(Locations), Some(Collectives)]),
            (Collectives, [Some(Citizens), Some(Locations)]),
        ] {
            for action in [Action::Close(kind), Action::Minimize(kind)] {
                let mut windows = Windows::default();
                windows.open(Citizens);
                windows.link(Citizens, Market);
                windows.link(Citizens, Locations);
                windows.link(Citizens, Collectives);

                windows.apply(action);

                assert_eq!(windows.visible(), expected);
                assert_eq!(windows.displaced().collect::<Vec<_>>(), [Market]);
                assert_eq!(
                    windows.manual().next(),
                    match action {
                        Action::Minimize(kind) => Some(kind),
                        _ => None,
                    }
                );
            }
        }
    }

    #[test]
    fn active_rails_reserve_only_their_side_in_the_workspace() {
        let mut windows = Windows::default();
        windows.set_viewport(Vec2::new(800.0, 600.0));
        windows.open(WindowKind::Citizens);
        windows.link(WindowKind::Citizens, WindowKind::Market);
        windows.link(WindowKind::Citizens, WindowKind::Locations);
        let right = windows.bounds(WindowKind::Locations).unwrap();
        assert_eq!(right.position.x + right.size.x, 800.0 - RAIL_WIDTH);
        assert_eq!(
            windows.bounds(WindowKind::Citizens).unwrap().position.x,
            0.0
        );
        windows.apply(Action::Minimize(WindowKind::Citizens));
        let recovered = windows.bounds(WindowKind::Market).unwrap();
        assert_eq!(recovered.position.x, RAIL_WIDTH);
        let right = windows.bounds(WindowKind::Locations).unwrap();
        assert_eq!(right.position.x + right.size.x, 800.0);
    }

    #[test]
    fn implicit_map_becomes_an_explicit_source_for_map_links() {
        let mut windows = Windows::default();
        windows.set_viewport(Vec2::new(800.0, 600.0));
        let full_map = windows.bounds(WindowKind::Map).unwrap();
        windows.link(WindowKind::Map, WindowKind::Locations);
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Map), Some(WindowKind::Locations)]
        );
        let split_map = windows.bounds(WindowKind::Map).unwrap();
        assert_eq!(split_map.size.x, full_map.size.x / 2.0);
        assert_eq!(split_map.position.y, full_map.position.y + TITLE_HEIGHT);
        windows.apply(Action::Close(WindowKind::Locations));
        assert!(!windows.implicit_map());
        windows.apply(Action::Close(WindowKind::Map));
        assert!(windows.implicit_map());
        assert_eq!(windows.bounds(WindowKind::Map).unwrap(), full_map);
    }
}
