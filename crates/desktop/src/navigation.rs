use crate::{
    DisplaySnapshot, citizens, collectives,
    floating_ui::{WindowKind, Windows},
    locations, market, text,
};
use bevy::prelude::*;
use learning_lord_simulation::{AgentId, locations::PlaceId, marketplace::Good};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Citizen(AgentId),
    Household(AgentId),
    Good(Good),
    Location(PlaceId),
}

pub(crate) fn link(parent: &mut ChildSpawnerCommands, label: impl Into<String>, target: Target) {
    parent
        .spawn((
            Button,
            target,
            Node {
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|button| {
            button.spawn((text(label, 14.0, Color::srgb(0.40, 0.80, 0.91)), Underline));
        });
}

type DetailScroll = (
    Entity,
    &'static mut ScrollPosition,
    Option<&'static citizens::DetailScroll>,
    Option<&'static collectives::ScrollPanel>,
    Option<&'static locations::ScrollPanel>,
);
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Input<'w, 's> {
    links: Query<
        'w,
        's,
        (
            Entity,
            &'static Interaction,
            &'static Target,
            Option<&'static InheritedVisibility>,
        ),
        Changed<Interaction>,
    >,
    parents: Query<'w, 's, &'static ChildOf>,
    roots: Query<'w, 's, &'static crate::floating_ui::FloatingWindow>,
    capture: Option<Res<'w, crate::floating_ui::InputCapture>>,
    scrolls: Query<'w, 's, DetailScroll>,
}

type Destinations<'w> = (
    ResMut<'w, citizens::Selection>,
    ResMut<'w, collectives::State>,
    ResMut<'w, locations::State>,
    ResMut<'w, market::Selection>,
    ResMut<'w, Windows>,
);

pub(crate) fn handle(
    input: Input,
    snapshot: Res<DisplaySnapshot>,
    mut generation: Local<u64>,
    (mut citizens, mut collectives, mut locations, mut market, mut windows): Destinations,
) {
    let Input {
        links,
        parents,
        roots,
        capture,
        mut scrolls,
    } = input;
    if *generation != snapshot.0.generation {
        *generation = snapshot.0.generation;
        return;
    }
    for (entity, interaction, target, visibility) in &links {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if visibility.is_some_and(|visibility| !visibility.get()) {
            continue;
        }
        let source = window_kind(entity, &parents, &roots);
        if let Some(capture) = capture.as_ref()
            && source.is_some()
            && source != capture.hovered
        {
            continue;
        }
        let universe = &snapshot.0.universe;
        let changed = match *target {
            Target::Citizen(id) => citizens.0 != Some(id),
            Target::Household(id) => collectives.selected() != Some(id),
            Target::Location(id) => locations.selected_place() != Some(id),
            Target::Good(good) => market.good != Some(good),
        };
        let kind = match *target {
            Target::Citizen(id) if universe.agents().contains_key(&id) => {
                citizens.0 = Some(id);
                WindowKind::Citizens
            }
            Target::Household(id) if universe.households().contains_key(&id) => {
                collectives.select(id);
                WindowKind::Collectives
            }
            Target::Location(id) if universe.map().place(id).is_ok() => {
                locations.select_place(id);
                WindowKind::Locations
            }
            Target::Good(good) => {
                market.good = Some(good);
                market.tab = market::Tab::Overview;
                WindowKind::Market
            }
            _ => continue,
        };
        if changed {
            for (entity, mut scroll, citizen, collective, location) in &mut scrolls {
                let detail = citizen.is_some()
                    || collective.is_some_and(|panel| panel.0)
                    || location
                        .is_some_and(|panel| matches!(panel, locations::ScrollPanel::Details));
                if detail && window_kind(entity, &parents, &roots) == Some(kind) {
                    scroll.0 = Vec2::ZERO;
                }
            }
        }
        windows.open(kind);
    }
}

pub(crate) fn window_kind(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    roots: &Query<&crate::floating_ui::FloatingWindow>,
) -> Option<WindowKind> {
    loop {
        if let Ok(root) = roots.get(entity) {
            return Some(root.0);
        }
        entity = parents.get(entity).ok()?.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        let worker = crate::simulation::SimulationWorker::spawn(
            crate::simulation::new_universe().unwrap(),
            60.try_into().unwrap(),
        );
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .init_resource::<Windows>()
            .init_resource::<citizens::Selection>()
            .init_resource::<collectives::State>()
            .init_resource::<locations::State>()
            .init_resource::<market::Selection>()
            .add_systems(
                Update,
                (
                    handle,
                    crate::inspectors::open_windows,
                    collectives::refresh,
                )
                    .chain(),
            );
        app
    }

    fn click(app: &mut App, target: Target) {
        let entity = app.world_mut().spawn((target, Interaction::Pressed)).id();
        app.update();
        app.world_mut().despawn(entity);
    }

    #[test]
    fn typed_links_select_targets_reuse_windows_and_keep_source_open() {
        let mut app = app();
        let universe = &app.world().resource::<DisplaySnapshot>().0.universe;
        let household = *universe.households().keys().next().unwrap();
        let member = universe.households()[&household].members[0];
        let home = universe.households()[&household].home;
        click(&mut app, Target::Citizen(member));
        click(&mut app, Target::Household(household));
        assert_eq!(
            app.world().resource::<collectives::State>().selected(),
            Some(household)
        );
        click(&mut app, Target::Good(Good::Wheat));
        assert_eq!(
            app.world().resource::<market::Selection>().good,
            Some(Good::Wheat)
        );
        assert_eq!(
            app.world().resource::<market::Selection>().tab,
            market::Tab::Overview
        );
        click(&mut app, Target::Location(home));
        assert_eq!(
            app.world().resource::<locations::State>().selected_place(),
            Some(home)
        );
        for _ in 0..3 {
            click(&mut app, Target::Household(household));
        }
        let windows = app
            .world_mut()
            .query::<(&crate::floating_ui::FloatingWindow, &Node)>()
            .iter(app.world())
            .map(|(window, node)| (window.0, node.display))
            .collect::<Vec<_>>();
        assert_eq!(windows.len(), 4);
        assert!(windows.iter().all(|(_, display)| *display == Display::Flex));
        assert_eq!(
            windows
                .iter()
                .filter(|(kind, _)| *kind == WindowKind::Collectives)
                .count(),
            1
        );
    }

    #[test]
    fn changing_universes_preserves_good_selection_and_discards_stale_presses() {
        let mut app = app();
        click(&mut app, Target::Good(Good::Wheat));
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        click(&mut app, Target::Good(Good::BreadRoll));
        assert_eq!(
            app.world().resource::<market::Selection>().good,
            Some(Good::Wheat)
        );
    }
}
