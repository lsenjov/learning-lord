use crate::{
    citizens,
    floating_ui::{Bounds, WindowKind, Windows},
    locations, market,
};
use bevy::prelude::*;

pub fn open_windows(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    snapshot: Res<crate::DisplaySnapshot>,
    mut roots: Query<
        (&mut Node, &mut GlobalZIndex, &mut Visibility),
        With<crate::floating_ui::FloatingWindow>,
    >,
    mut editors: crate::floating_ui::EditorFocus,
    primary: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if let Some(kind) = windows.requested_focus() {
        editors.activate(kind);
    }
    for kind in windows.take_new() {
        let (title, position, size) = match kind {
            WindowKind::Citizens => ("Citizens", Vec2::new(32.0, 80.0), Vec2::new(960.0, 600.0)),
            WindowKind::Locations => (
                "Locations · inventory & taxes",
                Vec2::new(100.0, 60.0),
                Vec2::new(960.0, 600.0),
            ),
            WindowKind::Market => (
                "Market · overview & caravan controls",
                Vec2::new(80.0, 50.0),
                Vec2::new(1000.0, 620.0),
            ),
        };
        let bounds = Bounds { position, size };
        let bounds = primary.single().map_or(bounds, |window| {
            bounds.constrain_minimum(
                Vec2::new(window.width(), window.height()),
                kind.minimum_size(),
            )
        });
        let entity = crate::floating_ui::spawn_window(
            &mut commands,
            kind,
            title,
            bounds,
            |parent| match kind {
                WindowKind::Citizens => {
                    parent
                        .spawn(Node {
                            width: percent(100),
                            flex_grow: 1.0,
                            min_height: px(0),
                            column_gap: px(14),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Node {
                                    width: px(250),
                                    flex_shrink: 0.0,
                                    min_height: px(0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(10),
                                    overflow: Overflow::scroll_y(),
                                    ..default()
                                },
                                ScrollPosition::default(),
                                bevy::ui::RelativeCursorPosition::default(),
                            ))
                            .with_children(|list| {
                                citizens::spawn_roster(list, snapshot.0.universe.agents().len())
                            });
                            citizens::spawn_details(row);
                        });
                }
                WindowKind::Locations => locations::spawn(parent),
                WindowKind::Market => market::spawn(parent),
            },
        );
        let order = windows.register(kind, entity, bounds);
        commands.entity(entity).insert(GlobalZIndex(order));
    }
    windows.sync(&mut roots);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Snapshot;
    use crate::{DisplaySnapshot, market_history};

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
        .init_resource::<crate::floating_ui::InputCapture>()
        .init_resource::<citizens::Selection>()
        .init_resource::<citizens::RosterState>()
        .init_resource::<citizens::PlanDisplay>()
        .init_resource::<locations::State>()
        .init_resource::<crate::caravan_controls::State>()
        .init_resource::<market::Selection>()
        .init_resource::<market::OrderDisplay>()
        .init_resource::<market_history::ChartDisplay>()
        .add_systems(
            Update,
            (
                open_windows,
                citizens::refresh_plan,
                market::refresh,
                locations::refresh,
                market_history::refresh,
            )
                .chain(),
        );
        app.world_mut().spawn((
            Window {
                resolution: (800, 600).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.update();
        app.update();
        app
    }

    #[test]
    fn opening_market_releases_location_editor_focus() {
        let mut app = app();
        app.insert_resource(locations::State::focused_fixture());
        assert!(app.world().resource::<locations::State>().editing());
        app.world_mut()
            .resource_mut::<Windows>()
            .open(WindowKind::Market);
        app.update();
        assert!(!app.world().resource::<locations::State>().editing());
        assert_eq!(
            app.world().resource::<market::Selection>().view,
            market::View::Market
        );
    }

    #[test]
    fn first_open_after_idle_populates_overviews_and_constrains_bounds() {
        let mut app = app();
        {
            let mut windows = app.world_mut().resource_mut::<Windows>();
            windows.open(WindowKind::Citizens);
            windows.open(WindowKind::Locations);
            windows.open(WindowKind::Market);
        }
        app.update();
        for node in app
            .world_mut()
            .query_filtered::<&Node, With<crate::floating_ui::FloatingWindow>>()
            .iter(app.world())
        {
            let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
                (node.left, node.top, node.width, node.height)
            else {
                panic!("pixel window bounds");
            };
            assert!(left >= 0.0 && top >= 0.0 && left + width <= 800.0 && top + height <= 600.0);
        }
        assert!(
            app.world_mut()
                .query_filtered::<&Children, With<citizens::PlanContent>>()
                .iter(app.world())
                .all(|children| !children.is_empty())
        );
        assert!(
            app.world_mut()
                .query_filtered::<&Children, With<market_history::Plot>>()
                .iter(app.world())
                .all(|children| !children.is_empty())
        );
        assert!(
            app.world_mut()
                .query_filtered::<&Children, With<locations::Content>>()
                .iter(app.world())
                .all(|children| !children.is_empty())
        );
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "LOCATION TAXES")
        );
    }
    #[test]
    fn sidebar_selection_and_reopening_keep_exactly_three_category_windows() {
        let mut app = app();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input_focus::InputFocus>()
            .add_systems(Update, citizens::handle_selection);
        for kind in [
            WindowKind::Citizens,
            WindowKind::Locations,
            WindowKind::Market,
        ] {
            app.world_mut().resource_mut::<Windows>().open(kind);
        }
        app.update();
        let ids = citizens::sorted_agents(&app.world().resource::<DisplaySnapshot>().0.universe)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let cards = app
            .world_mut()
            .query_filtered::<Entity, With<citizens::Card>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        assert_eq!(cards.len(), ids.len());
        for (index, card) in cards.iter().enumerate() {
            app.world_mut()
                .entity_mut(*card)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(
                app.world().resource::<citizens::Selection>().0,
                Some(ids[index])
            );
            app.world_mut().entity_mut(*card).insert(Interaction::None);
            app.update();
            app.world_mut()
                .resource_mut::<Windows>()
                .open(WindowKind::Citizens);
            app.update();
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<crate::floating_ui::FloatingWindow>>()
                    .iter(app.world())
                    .count(),
                3
            );
        }
    }
}
