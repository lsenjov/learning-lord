use crate::{
    DisplaySnapshot, MUTED, TEXT, citizen_history, citizens,
    floating_ui::{Bounds, WindowKind, Windows},
    locations, market, market_history, text,
};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use learning_lord_simulation::{
    AgentId, AgentKind,
    locations::PlaceId,
    marketplace::Good,
    taxation::{TaxKind, TaxPayer},
};

#[derive(Component)]
pub(crate) struct CitizenText(AgentId);
#[derive(Component)]
pub(crate) struct LocationText(PlaceId);
#[derive(Component)]
pub(crate) struct GoodText(Good);
#[derive(Component, Default)]
pub(crate) struct CitizenHistory {
    day: Option<u64>,
    generation: u64,
}
#[derive(Component, Default)]
pub(crate) struct GoodPeriod {
    previous: bool,
    generation: u64,
}
#[derive(Component)]
pub(crate) struct DetailRoot(WindowKind);
#[derive(Component)]
pub(crate) enum Action {
    CitizenDay(AgentId, bool),
    GoodPeriod(Good, bool),
    EditLocation(PlaceId),
    EditGood(Good),
}

fn action(parent: &mut ChildSpawnerCommands, label: &str, action: Action) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::axes(px(12), px(8)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.16, 0.23, 0.26)),
        ))
        .with_children(|b| {
            b.spawn(text(label, 14.0, TEXT));
        });
}

fn scroll(parent: &mut ChildSpawnerCommands, content: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                min_height: px(0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            RelativeCursorPosition::default(),
        ))
        .with_children(content);
}

pub fn open_windows(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    snapshot: Res<DisplaySnapshot>,
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
        let (title, bounds) = match kind {
            WindowKind::Citizens => (
                "Citizens".into(),
                Bounds {
                    position: Vec2::new(32.0, 80.0),
                    size: Vec2::new(960.0, 600.0),
                },
            ),
            WindowKind::Locations => (
                "Locations · inventory & taxes".into(),
                Bounds {
                    position: Vec2::new(100.0, 60.0),
                    size: Vec2::new(960.0, 600.0),
                },
            ),
            WindowKind::Market => (
                "Market · overview & caravan controls".into(),
                Bounds {
                    position: Vec2::new(80.0, 50.0),
                    size: Vec2::new(1000.0, 620.0),
                },
            ),
            WindowKind::Citizen(id) => (
                snapshot
                    .0
                    .universe
                    .agents()
                    .get(&id)
                    .map_or_else(|| "Citizen".into(), |agent| agent.name.clone()),
                Bounds {
                    position: Vec2::new(60.0, 60.0),
                    size: Vec2::new(560.0, 600.0),
                },
            ),
            WindowKind::Location(id) => (
                snapshot
                    .0
                    .universe
                    .map()
                    .place(id)
                    .map_or_else(|_| "Location".into(), |place| place.name.clone()),
                Bounds {
                    position: Vec2::new(180.0, 90.0),
                    size: Vec2::new(560.0, 560.0),
                },
            ),
            WindowKind::Good(good) => (
                good.name().into(),
                Bounds {
                    position: Vec2::new(260.0, 70.0),
                    size: Vec2::new(680.0, 600.0),
                },
            ),
        };
        let bounds = primary.single().map_or(bounds, |window| {
            bounds.constrain_minimum(
                Vec2::new(window.width(), window.height()),
                kind.minimum_size(),
            )
        });
        let entity =
            crate::floating_ui::spawn_window(&mut commands, kind, &title, bounds, |parent| {
                match kind {
                    WindowKind::Citizens => {
                        citizens::spawn_roster(parent);
                        citizens::spawn_details(parent);
                    }
                    WindowKind::Locations => locations::spawn(parent),
                    WindowKind::Market => market::spawn(parent),
                    WindowKind::Citizen(id) => scroll(parent, |content| {
                        content.spawn((text("", 15.0, TEXT), CitizenText(id)));
                        content.spawn(text("DAILY HISTORY", 16.0, MUTED));
                        content
                            .spawn(Node {
                                column_gap: px(8),
                                ..default()
                            })
                            .with_children(|row| {
                                action(row, "Previous day", Action::CitizenDay(id, true));
                                action(row, "Next day", Action::CitizenDay(id, false));
                            });
                        content.spawn((
                            text("", 14.0, TEXT),
                            DetailRoot(kind),
                            CitizenHistory::default(),
                        ));
                    }),
                    WindowKind::Location(id) => scroll(parent, |content| {
                        action(
                            content,
                            "Inventory transfers & tax editor",
                            Action::EditLocation(id),
                        );
                        content.spawn((text("", 15.0, TEXT), LocationText(id)));
                    }),
                    WindowKind::Good(good) => scroll(parent, |content| {
                        action(
                            content,
                            "Caravan policies & market overview",
                            Action::EditGood(good),
                        );
                        content
                            .spawn(Node {
                                column_gap: px(8),
                                ..default()
                            })
                            .with_children(|row| {
                                action(row, "Current market day", Action::GoodPeriod(good, false));
                                action(row, "Previous market day", Action::GoodPeriod(good, true));
                            });
                        content.spawn((
                            text("", 15.0, TEXT),
                            GoodText(good),
                            GoodPeriod::default(),
                        ));
                        market_history::spawn_scoped(content, Some(good));
                    }),
                }
            });
        let order = windows.register(kind, entity, bounds);
        commands.entity(entity).insert(GlobalZIndex(order));
    }
    windows.sync(&mut roots);
}

pub fn handle(
    snapshot: Res<DisplaySnapshot>,
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut histories: Query<(&DetailRoot, &mut CitizenHistory)>,
    mut periods: Query<(&GoodText, &mut GoodPeriod)>,
    mut windows: ResMut<Windows>,
    mut locations: ResMut<locations::State>,
    mut market: ResMut<market::Selection>,
) {
    for (_, mut history) in &mut histories {
        if history.generation != snapshot.0.generation {
            history.generation = snapshot.0.generation;
            history.day = None;
        }
    }
    for (_, mut period) in &mut periods {
        if period.generation != snapshot.0.generation {
            period.generation = snapshot.0.generation;
            period.previous = false;
        }
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            Action::CitizenDay(id, previous) => {
                let Some(history) = snapshot.0.universe.citizen_history(id) else {
                    continue;
                };
                let starts = history
                    .completed
                    .iter()
                    .chain(std::iter::once(&history.current))
                    .map(|day| day.start_ms)
                    .collect::<Vec<_>>();
                for (root, mut history) in &mut histories {
                    if root.0 == WindowKind::Citizen(id) {
                        history.day = citizen_history::navigate(history.day, &starts, previous);
                    }
                }
            }
            Action::GoodPeriod(good, previous) => {
                for (scope, mut period) in &mut periods {
                    if scope.0 == good {
                        period.previous = previous;
                    }
                }
            }
            Action::EditLocation(id) => {
                locations::select_place(&mut locations, id);
                market.view = market::View::Locations;
                windows.open(WindowKind::Locations);
            }
            Action::EditGood(good) => {
                market::select_good(&mut market, good);
                windows.open(WindowKind::Market);
            }
        }
    }
}

fn citizen_text(snapshot: &DisplaySnapshot, id: AgentId) -> String {
    let Some(agent) = snapshot.0.universe.agents().get(&id) else {
        return "Citizen no longer exists in this universe.".into();
    };
    let AgentKind::Citizen(citizen) = &agent.kind;
    format!(
        "{}\n\nACTIVE PLAN · durations are planning estimates\n{}\n\n{}",
        crate::citizen_readout(&snapshot.0.universe, Some(id)),
        citizens::plan_text(citizen),
        crate::decision_readout(&snapshot.0.universe, Some(id))
    )
}

fn location_text(snapshot: &DisplaySnapshot, id: PlaceId) -> String {
    let universe = &snapshot.0.universe;
    let map = universe.map();
    let Ok(place) = map.place(id) else {
        return "Location no longer exists in this universe.".into();
    };
    let mut value = format!(
        "{}\nOwner: {}\n\nSTOCK BY OWNER\n{}\n\nLOCATION TAXES\n",
        place.name,
        locations::place_owner(universe, place),
        locations::stocks(universe, id)
    );
    let mut rules = universe
        .tax_rules()
        .values()
        .filter(|rule| rule.active && universe.tax_scope_matches(rule.scope, id))
        .collect::<Vec<_>>();
    rules.sort_by(|a, b| a.name.cmp(&b.name));
    if rules.is_empty() {
        value.push_str("No active tax rules.\n");
    }
    for rule in rules {
        let detail = match &rule.kind {
            TaxKind::FlatFee { payer, coins } => format!(
                "{coins} coins weekly · {}",
                match payer {
                    TaxPayer::Agent(id) => universe
                        .agents()
                        .get(id)
                        .map_or("Unknown citizen", |agent| agent.name.as_str()),
                    TaxPayer::LocationOwner => "Property owner",
                }
            ),
            TaxKind::Socage { rates } | TaxKind::Asset { rates } | TaxKind::Income { rates } => {
                let mut rates = rates
                    .iter()
                    .map(|(good, rate)| {
                        format!(
                            "{}: {:.2}%",
                            good.name(),
                            f64::from(rate.basis_points()) / 100.0
                        )
                    })
                    .collect::<Vec<_>>();
                rates.sort();
                rates.join(" · ")
            }
        };
        value.push_str(&format!(
            "{} · {}\n{}\n",
            rule.name,
            locations::kind_label(&rule.kind),
            detail
        ));
    }
    value.push_str(&format!(
        "\nTREASURY & COLLECTIONS\n{}",
        locations::treasury(universe)
    ));
    value
}

fn good_text(snapshot: &DisplaySnapshot, good: Good, previous: bool) -> String {
    let universe = &snapshot.0.universe;
    let period = if previous {
        universe.market().previous_period()
    } else {
        Some(universe.market().current_period())
    };
    let metrics = period.map_or_else(
        || "No closed market day yet.".into(),
        |period| {
            format!(
                "{}\n{}",
                market::period_label(
                    &period,
                    if previous {
                        market::Period::Previous
                    } else {
                        market::Period::Current
                    },
                    universe.current_time_ms()
                ),
                market::metrics(good, period.goods[good as usize])
            )
        },
    );
    format!(
        "{}\n\n{}\n{}\n\n{}\n\nCURRENT BUY REQUESTS\n{}\n\nCURRENT SELL ORDERS\n{}",
        good.name(),
        market::price_details(universe, good),
        market::material_margins(universe, good, true),
        metrics,
        market::buyer_requests(universe, good),
        market::order_text(universe, good)
    )
}

type CitizenFilter = (
    Without<LocationText>,
    Without<GoodText>,
    Without<CitizenHistory>,
);
type LocationFilter = (
    Without<CitizenText>,
    Without<GoodText>,
    Without<CitizenHistory>,
);
type GoodFilter = (
    Without<CitizenText>,
    Without<LocationText>,
    Without<CitizenHistory>,
);
type HistoryFilter = (
    Without<CitizenText>,
    Without<LocationText>,
    Without<GoodText>,
);

pub fn refresh(
    snapshot: Res<DisplaySnapshot>,
    windows: Res<Windows>,
    mut citizens: Query<(&CitizenText, &mut Text), CitizenFilter>,
    mut locations: Query<(&LocationText, &mut Text), LocationFilter>,
    mut goods: Query<(&GoodText, Ref<GoodPeriod>, &mut Text), GoodFilter>,
    mut histories: Query<(&DetailRoot, Ref<CitizenHistory>, &mut Text), HistoryFilter>,
) {
    for (scope, mut text) in &mut citizens {
        if !windows.is_open(WindowKind::Citizen(scope.0)) && !snapshot.is_changed() {
            continue;
        }
        if !snapshot.is_changed() && !text.is_added() {
            continue;
        }
        let value = citizen_text(&snapshot, scope.0);
        if text.0 != value {
            text.0 = value;
        }
    }
    for (scope, mut text) in &mut locations {
        if !windows.is_open(WindowKind::Location(scope.0)) && !snapshot.is_changed() {
            continue;
        }
        if !snapshot.is_changed() && !text.is_added() {
            continue;
        }
        let value = location_text(&snapshot, scope.0);
        if text.0 != value {
            text.0 = value;
        }
    }
    for (scope, period, mut text) in &mut goods {
        if !windows.is_open(WindowKind::Good(scope.0)) && !snapshot.is_changed() {
            continue;
        }
        if !snapshot.is_changed() && !period.is_changed() && !text.is_added() {
            continue;
        }
        let value = good_text(&snapshot, scope.0, period.previous);
        if text.0 != value {
            text.0 = value;
        }
    }
    for (scope, selected, mut text) in &mut histories {
        if !windows.is_open(scope.0) && !snapshot.is_changed() {
            continue;
        }
        if !snapshot.is_changed() && !selected.is_changed() && !text.is_added() {
            continue;
        }
        let WindowKind::Citizen(id) = scope.0 else {
            continue;
        };
        let Some(history) = snapshot.0.universe.citizen_history(id) else {
            text.0 = "No history available.".into();
            continue;
        };
        let days = history
            .completed
            .iter()
            .chain(std::iter::once(&history.current))
            .collect::<Vec<_>>();
        let starts = days.iter().map(|day| day.start_ms).collect::<Vec<_>>();
        let index = citizen_history::selected_day(selected.day, &starts);
        let mut value = index.map_or_else(
            || "No history available.".into(),
            |index| citizen_history::day_label(days[index], index + 1 == days.len()),
        );
        if let Some(index) = index {
            let planning = snapshot.0.planning_history.get(&id).and_then(|history| {
                history
                    .completed
                    .iter()
                    .chain(std::iter::once(&history.current))
                    .find(|day| day.start_ms == days[index].start_ms)
            });
            let (request, wait, requests) = planning.map_or((0.0, 0.0, 0), |day| {
                (day.request_ms, day.wait_ms, day.requests)
            });
            value.push_str(&format!("\nCash totals are actual trades; inventory price changes do not affect them.\n\nBACKGROUND PLANNING · REAL WALL TIME\nRequest queue + work {:.3} s | {} requests\nBlocked at action boundary {:.3} s\nAttributed to plan adoption day; initial startup plans excluded.", request / 1000.0, requests, wait / 1000.0));
        }
        if text.0 != value {
            text.0 = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Snapshot;

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
        .init_resource::<citizens::RosterPage>()
        .init_resource::<citizens::PlanDisplay>()
        .init_resource::<locations::State>()
        .init_resource::<crate::caravan_controls::State>()
        .init_resource::<market::Selection>()
        .init_resource::<market::OrderDisplay>()
        .init_resource::<market_history::ChartDisplay>()
        .init_resource::<market_history::ScopedCharts>()
        .add_systems(
            Update,
            (
                handle,
                open_windows,
                citizens::refresh_plan,
                market::refresh,
                locations::refresh,
                market_history::refresh,
                market_history::refresh_scoped,
                refresh,
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
    fn citizen_and_good_windows_keep_independent_content_and_navigation() {
        let mut app = app();
        let agents = citizens::sorted_agents(&app.world().resource::<DisplaySnapshot>().0.universe);
        let first = agents[0].0;
        let second = agents[1].0;
        let names = [agents[0].1.name.clone(), agents[1].1.name.clone()];
        {
            let mut windows = app.world_mut().resource_mut::<Windows>();
            for kind in [
                WindowKind::Citizen(first),
                WindowKind::Citizen(second),
                WindowKind::Good(Good::Berries),
                WindowKind::Good(Good::Bread),
            ] {
                windows.open(kind);
            }
        }
        app.update();
        let values = app
            .world_mut()
            .query::<(&CitizenText, &Text)>()
            .iter(app.world())
            .map(|(scope, text)| (scope.0, text.0.clone()))
            .collect::<Vec<_>>();
        assert!(
            values
                .iter()
                .any(|(id, text)| *id == first && text.contains(&names[0]))
        );
        assert!(
            values
                .iter()
                .any(|(id, text)| *id == second && text.contains(&names[1]))
        );
        let button = app
            .world_mut()
            .query::<(Entity, &Action)>()
            .iter(app.world())
            .find(|(_, action)| matches!(action, Action::GoodPeriod(Good::Berries, true)))
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        for (scope, period, text) in app
            .world_mut()
            .query::<(&GoodText, &GoodPeriod, &Text)>()
            .iter(app.world())
        {
            assert_eq!(period.previous, scope.0 == Good::Berries);
            assert!(text.0.starts_with(scope.0.name()));
        }
        for (_, mut history) in app
            .world_mut()
            .query::<(&DetailRoot, &mut CitizenHistory)>()
            .iter_mut(app.world_mut())
        {
            history.day = Some(123);
        }
        let button = app
            .world_mut()
            .query::<(Entity, &Action)>()
            .iter(app.world())
            .find(|(_, action)| matches!(action, Action::CitizenDay(id, true) if *id == first))
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        for (scope, history) in app
            .world_mut()
            .query::<(&DetailRoot, &CitizenHistory)>()
            .iter(app.world())
        {
            assert_eq!(
                history.day,
                if scope.0 == WindowKind::Citizen(first) {
                    None
                } else {
                    Some(123)
                }
            );
        }
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert!(
            app.world_mut()
                .query::<&CitizenHistory>()
                .iter(app.world())
                .all(|history| history.day.is_none())
        );
        assert!(
            app.world_mut()
                .query::<&GoodPeriod>()
                .iter(app.world())
                .all(|period| !period.previous)
        );
    }
}
