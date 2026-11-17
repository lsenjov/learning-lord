use crate::{
    citizens, collectives,
    floating_ui::{self, FloatingWindow, MapTitle, PaneTitle, WindowKind, Windows},
    locations, market,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use learning_lord_simulation::Universe;

impl WindowKind {
    fn entity_detail(self) -> bool {
        matches!(
            self,
            Self::Citizen(_) | Self::Household(_) | Self::Guild(_) | Self::Location(_)
        )
    }

    fn valid(self, universe: &Universe) -> bool {
        match self {
            Self::Citizen(id) => universe.agents().contains_key(&id),
            Self::Household(id) => universe.households().contains_key(&id),
            Self::Guild(id) => universe.guilds().contains_key(&id),
            Self::Location(id) => universe.map().place(id).is_ok(),
            _ => true,
        }
    }

    fn title(self, universe: &Universe, collectives: &str) -> String {
        match self {
            Self::Map => "Map",
            Self::Citizens => "Citizens",
            Self::Collectives => collectives,
            Self::Locations => "Locations",
            Self::Market => "Market",
            Self::Saves => "Load universe",
            Self::Good(good) => good.name(),
            Self::Citizen(id) => universe
                .agents()
                .get(&id)
                .map_or("Citizen", |agent| agent.name.as_str()),
            Self::Household(id) => {
                return universe.households().get(&id).map_or_else(
                    || "Household".to_owned(),
                    |household| format!("{} household", household.name),
                );
            }
            Self::Guild(id) => universe
                .guilds()
                .get(&id)
                .map_or("Guild", |guild| guild.name.as_str()),
            Self::Location(id) => {
                return universe
                    .map()
                    .place(id)
                    .map_or_else(|_| "Location".to_owned(), |place| place.name.clone());
            }
        }
        .to_owned()
    }
}

#[derive(Default)]
pub(crate) struct InspectorState {
    generation: Option<u64>,
    manual: Rail,
    displaced: Rail,
    collective_title: &'static str,
}

#[derive(Default)]
struct Rail {
    entries: Vec<(WindowKind, String)>,
    entity: Option<Entity>,
}

impl Rail {
    fn update(
        &mut self,
        commands: &mut Commands,
        entries: Vec<(WindowKind, String)>,
        automatic: bool,
        scrolls: &Query<&ScrollPosition>,
    ) {
        if self.entries == entries {
            return;
        }
        let mut offset = ScrollPosition::default();
        if let Some(entity) = self.entity.take() {
            if let Ok(scroll) = scrolls.get(entity) {
                offset.0 = scroll.0;
            }
            commands.entity(entity).despawn();
        }
        if !entries.is_empty() {
            self.entity = Some(floating_ui::spawn_rail(
                commands, &entries, automatic, offset,
            ));
        }
        self.entries = entries;
    }
}

#[derive(SystemParam)]
pub(crate) struct PaneNodes<'w, 's> {
    scrolls: Query<'w, 's, &'static ScrollPosition>,
    roots: Query<'w, 's, (&'static mut Node, &'static mut Visibility), With<FloatingWindow>>,
    map_titles: Query<'w, 's, &'static mut Node, (With<MapTitle>, Without<FloatingWindow>)>,
    titles: Query<'w, 's, (&'static PaneTitle, &'static mut Text)>,
}

pub fn open_windows(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    snapshot: Res<crate::DisplaySnapshot>,
    mut nodes: PaneNodes,
    collectives: Option<Res<collectives::State>>,
    mut editors: floating_ui::EditorFocus,
    primary: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut state: Local<InspectorState>,
) {
    let universe = &snapshot.0.universe;
    let collective_title = collectives
        .as_deref()
        .map_or("Households", collectives::State::title);
    let title_changed = state.collective_title != collective_title;
    state.collective_title = collective_title;
    if snapshot.is_changed() || state.generation.is_none() {
        let reset = state
            .generation
            .is_some_and(|generation| generation != snapshot.0.generation);
        state.generation = Some(snapshot.0.generation);
        windows.retain(|kind| !(reset && kind.entity_detail()) && kind.valid(universe));
        editors.retain_citizen(|id| !reset && WindowKind::Citizen(id).valid(universe));
    }
    if snapshot.is_changed() || title_changed {
        for (title, text) in &mut nodes.titles {
            let label = title.0.title(universe, collective_title);
            text.map_unchanged(|text| &mut text.0).set_if_neq(label);
        }
    }
    for (_, entity) in windows.take_inactive_entries() {
        commands.entity(entity).despawn();
    }
    if let Some(kind) = windows.take_focus() {
        editors.activate(kind);
    }
    if let Ok(window) = primary.single() {
        windows.set_viewport(Vec2::new(window.width(), window.height()));
    }
    for kind in windows.missing() {
        let bounds = windows.pane_bounds(kind).expect("visible pane has bounds");
        let entity = floating_ui::spawn_window(
            &mut commands,
            kind,
            &kind.title(universe, collective_title),
            bounds,
            windows.implicit_map(),
            |parent| match kind {
                WindowKind::Map => (),
                WindowKind::Citizens => citizens::spawn_roster(parent, universe.agents().len()),
                WindowKind::Collectives => collectives::spawn(parent),
                WindowKind::Locations => locations::spawn(parent),
                WindowKind::Market => market::spawn_overview(parent),
                WindowKind::Saves => crate::saves::spawn(parent),
                WindowKind::Citizen(id) => citizens::spawn_details(parent, id),
                WindowKind::Household(_) | WindowKind::Guild(_) => {
                    collectives::spawn_entity(parent, kind);
                }
                WindowKind::Location(id) => locations::spawn_entity(parent, id),
                WindowKind::Good(good) => market::spawn_entity(parent, good),
            },
        );
        windows.register(kind, entity);
    }
    let PaneNodes {
        roots, map_titles, ..
    } = &mut nodes;
    windows.sync(roots, map_titles);
    if !title_changed
        && !snapshot.is_changed()
        && windows
            .manual()
            .eq(state.manual.entries.iter().map(|(kind, _)| *kind))
        && windows
            .displaced()
            .eq(state.displaced.entries.iter().map(|(kind, _)| *kind))
    {
        return;
    }
    let manual = windows
        .manual()
        .map(|kind| (kind, kind.title(universe, collective_title)))
        .collect::<Vec<_>>();
    let displaced = windows
        .displaced()
        .map(|kind| (kind, kind.title(universe, collective_title)))
        .collect::<Vec<_>>();
    state
        .manual
        .update(&mut commands, manual, false, &nodes.scrolls);
    state
        .displaced
        .update(&mut commands, displaced, true, &nodes.scrolls);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DisplaySnapshot, simulation::Snapshot};

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe: crate::simulation::new_universe().unwrap(),
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
        }))
        .init_resource::<Windows>()
        .init_resource::<floating_ui::InputCapture>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<citizens::Selection>()
        .add_systems(Update, (floating_ui::interact, open_windows).chain());
        app.world_mut().spawn((
            Window {
                resolution: (800, 600).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app
    }

    #[test]
    fn identical_page_identity_is_reused_through_navigation() {
        let mut app = app();
        let first = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .agents()
            .keys()
            .copied()
            .next()
            .unwrap();
        let detail = WindowKind::Citizen(first);
        app.world_mut().resource_mut::<Windows>().open(detail);
        app.update();
        let mut panes = app.world_mut().query::<(Entity, &FloatingWindow)>();
        let root = panes
            .iter(app.world())
            .find(|(_, pane)| pane.0 == detail)
            .unwrap()
            .0;
        {
            let mut windows = app.world_mut().resource_mut::<Windows>();
            windows.open(WindowKind::Citizens);
            windows.link(WindowKind::Citizens, detail);
        }
        app.update();
        assert_eq!(
            panes
                .iter(app.world())
                .filter(|(_, pane)| pane.0 == detail)
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>(),
            [root]
        );
        assert_eq!(app.world().resource::<citizens::Selection>().0, Some(first));
        assert_eq!(app.world().get::<Node>(root).unwrap().left, px(400.0));
    }

    #[test]
    fn pane_buttons_drive_recovery_and_closed_roots_despawn() {
        let mut app = app();
        {
            let mut windows = app.world_mut().resource_mut::<Windows>();
            windows.open(WindowKind::Citizens);
            windows.link(WindowKind::Citizens, WindowKind::Market);
            windows.link(WindowKind::Citizens, WindowKind::Locations);
        }
        app.update();
        let mut buttons = app
            .world_mut()
            .query::<(Entity, &floating_ui::WorkspaceButton)>();
        let close = buttons
            .iter(app.world())
            .find(|(_, button)| button.0 == floating_ui::Action::Close(WindowKind::Locations))
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(close)
            .insert(Interaction::Pressed);
        app.update();
        let windows = app.world().resource::<Windows>();
        assert_eq!(
            windows.visible(),
            [Some(WindowKind::Citizens), Some(WindowKind::Market)]
        );
        let mut panes = app.world_mut().query::<&FloatingWindow>();
        assert!(
            panes
                .iter(app.world())
                .all(|pane| pane.0 != WindowKind::Locations)
        );
    }

    #[test]
    fn pointer_capture_distinguishes_map_pane_rails_and_hud() {
        let mut app = app();
        {
            let mut windows = app.world_mut().resource_mut::<Windows>();
            windows.link(WindowKind::Map, WindowKind::Market);
            windows.link(WindowKind::Map, WindowKind::Locations);
        }
        app.update();
        let mut primary = app
            .world_mut()
            .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
        for (cursor, hovered, blocked) in [
            (Vec2::new(50.0, 100.0), Some(WindowKind::Map), false),
            (Vec2::new(50.0, 40.0), Some(WindowKind::Map), true),
            (Vec2::new(400.0, 100.0), Some(WindowKind::Locations), true),
            (Vec2::new(750.0, 100.0), None, true),
            (Vec2::new(50.0, 15.0), None, true),
        ] {
            primary
                .single_mut(app.world_mut())
                .unwrap()
                .set_cursor_position(Some(cursor));
            app.update();
            let capture = app.world().resource::<floating_ui::InputCapture>();
            assert_eq!(capture.hovered, hovered);
            assert_eq!(capture.blocked, blocked);
        }
    }

    #[test]
    fn replacing_universe_prunes_identity_roots_even_when_ids_still_exist() {
        let mut app = app();
        let id = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .agents()
            .keys()
            .copied()
            .next()
            .unwrap();
        let detail = WindowKind::Citizen(id);
        app.world_mut()
            .resource_mut::<Windows>()
            .link(WindowKind::Map, detail);
        app.update();
        let mut panes = app.world_mut().query::<&FloatingWindow>();
        assert!(panes.iter(app.world()).any(|pane| pane.0 == detail));
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert!(!panes.iter(app.world()).any(|pane| pane.0 == detail));
        assert_eq!(app.world().resource::<citizens::Selection>().0, None);
        assert!(app.world().resource::<Windows>().is_open(WindowKind::Map));
    }
}
