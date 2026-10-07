use crate::{DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, citizens, text};
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use learning_lord_simulation::AgentId;

#[derive(Clone, Copy, Default, PartialEq)]
pub enum View {
    #[default]
    Current,
    History,
}

#[derive(Resource, Default)]
pub struct Selection {
    view: View,
    day: Option<u64>,
    citizen: Option<AgentId>,
    generation: u64,
}

#[derive(Component)]
pub struct ViewPanel(pub View);
#[derive(Component, Clone, Copy)]
pub enum Choice {
    View(View),
    Previous,
    Next,
}
#[derive(Component)]
pub struct HistoryText;

fn button(choice: Choice) -> impl Bundle {
    (
        Button,
        choice,
        Node {
            min_height: px(32),
            padding: UiRect::axes(px(12), px(6)),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(PANEL),
    )
}

pub fn spawn_tabs(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            column_gap: px(8),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (view, label) in [(View::Current, "Current"), (View::History, "Daily history")] {
                row.spawn(button(Choice::View(view)))
                    .with_children(|button| {
                        button.spawn(text(label, 14.0, TEXT));
                    });
            }
        });
}

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            ViewPanel(View::History),
            Visibility::Hidden,
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|panel| {
            panel.spawn(text(
                "DAILY HISTORY  |  days close at 04:00; latest 30 completed days",
                14.0,
                MUTED,
            ));
            panel
                .spawn(Node {
                    column_gap: px(8),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|row| {
                    for (choice, label) in [
                        (Choice::Previous, "Previous day"),
                        (Choice::Next, "Next day"),
                    ] {
                        row.spawn(button(choice)).with_children(|button| {
                            button.spawn(text(label, 14.0, TEXT));
                        });
                    }
                });
            panel.spawn((
                text("", 14.0, TEXT),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                HistoryText,
            ));
        });
}

pub(crate) fn selected_day(selected: Option<u64>, starts: &[u64]) -> Option<usize> {
    if starts.is_empty() {
        return None;
    }
    Some(selected.map_or(starts.len() - 1, |day| {
        starts.iter().position(|start| *start == day).unwrap_or(0)
    }))
}

pub(crate) fn navigate(selected: Option<u64>, starts: &[u64], previous: bool) -> Option<u64> {
    let index = selected_day(selected, starts)?;
    let next = if previous {
        index.saturating_sub(1)
    } else {
        (index + 1).min(starts.len() - 1)
    };
    (next != starts.len() - 1).then_some(starts[next])
}

pub fn handle_selection(
    snapshot: Res<DisplaySnapshot>,
    citizen: Res<citizens::Selection>,
    mut selection: ResMut<Selection>,
    buttons: Query<(Entity, &Interaction, &Choice), Changed<Interaction>>,
    mut focus: ResMut<InputFocus>,
    mut scroll: Query<&mut ScrollPosition, With<citizens::DetailScroll>>,
) {
    let mut reset_scroll = false;
    if selection.generation != snapshot.0.generation || selection.citizen != citizen.0 {
        selection.day = None;
        selection.citizen = citizen.0;
        selection.generation = snapshot.0.generation;
        reset_scroll = true;
    }
    let starts = citizen
        .0
        .and_then(|id| snapshot.0.universe.citizen_history(id))
        .map_or_else(Vec::new, |history| {
            history
                .completed
                .iter()
                .chain(std::iter::once(&history.current))
                .map(|day| day.start_ms)
                .collect::<Vec<_>>()
        });
    for (entity, interaction, choice) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        focus.set(entity, FocusCause::Pressed);
        match choice {
            Choice::View(view) => selection.view = *view,
            Choice::Previous => selection.day = navigate(selection.day, &starts, true),
            Choice::Next => selection.day = navigate(selection.day, &starts, false),
        }
        reset_scroll = true;
    }
    if reset_scroll {
        for mut position in &mut scroll {
            position.0 = Vec2::ZERO;
        }
    }
}

fn goods_label(day: &learning_lord_simulation::history::DailyCitizenHistory) -> String {
    use learning_lord_simulation::marketplace::Good;
    let mut rows = Vec::new();
    for good in Good::ALL {
        let i = good as usize;
        if [day.produced[i], day.bought[i], day.sold[i], day.consumed[i]]
            .iter()
            .all(|units| *units == 0)
        {
            continue;
        }
        rows.push(format!(
            "{}  |  produced {} · bought {} · sold {} · consumed {}",
            good.name(),
            crate::quantity_label(good, day.produced[i]),
            crate::quantity_label(good, day.bought[i]),
            crate::quantity_label(good, day.sold[i]),
            crate::quantity_label(good, day.consumed[i])
        ));
    }
    if rows.is_empty() {
        "No goods produced, traded or consumed yet.".into()
    } else {
        rows.join("\n")
    }
}

pub(crate) fn day_label(
    day: &learning_lord_simulation::history::DailyCitizenHistory,
    partial: bool,
) -> String {
    let a = day.activity;
    format!(
        "{} to {}\n{}  |  {} observed\n\nACTUAL ACTIVITY TIME\nProduction / gathering {}  |  Travel {}  |  Sleep {}\nMarket activity {}  |  Eating {}  |  Other / idle / waiting {}\nFood goals (includes meals) {} · overlaps activity times\n\nGOODS\n{}\nConsumed includes recipe inputs, food reserved when eating starts, and equipped clothing.\n\nCOINS\nEarned {}  |  Spent {}  |  Net {:+}\n\nWELLBEING AND NEEDS\nAverage wellbeing {}\nWorst hunger {:.1}  |  Tiredness {:.1}  |  Clothing need {:.1}",
        crate::format_clock(day.start_ms),
        crate::format_clock(day.end_ms),
        if partial {
            "PARTIAL · live totals so far"
        } else {
            "COMPLETED"
        },
        citizens::duration(day.elapsed_ms),
        citizens::duration(a.production_ms),
        citizens::duration(a.travel_ms),
        citizens::duration(a.sleep_ms),
        citizens::duration(a.trade_ms),
        citizens::duration(a.eating_ms),
        citizens::duration(a.idle_ms),
        citizens::duration(day.food_goal_ms),
        goods_label(day),
        day.coins_earned,
        day.coins_spent,
        day.coins_earned - day.coins_spent,
        day.average_wellbeing().map_or_else(
            || "No elapsed time yet".into(),
            |value| format!("{value:.1}")
        ),
        day.worst_hunger,
        day.worst_tiredness,
        day.worst_clothing_need
    )
}

pub fn refresh(
    snapshot: Res<DisplaySnapshot>,
    citizen: Res<citizens::Selection>,
    selection: Res<Selection>,
    mut panels: Query<(&ViewPanel, &mut Node, &mut Visibility)>,
    mut buttons: Query<(&Choice, &Interaction, &mut BackgroundColor)>,
    mut readouts: Query<&mut Text, With<HistoryText>>,
) {
    for (panel, mut node, mut visibility) in &mut panels {
        let visible = panel.0 == selection.view;
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    let history = citizen
        .0
        .and_then(|id| snapshot.0.universe.citizen_history(id));
    let days = history.map_or_else(Vec::new, |history| {
        history
            .completed
            .iter()
            .chain(std::iter::once(&history.current))
            .collect::<Vec<_>>()
    });
    let starts = days.iter().map(|day| day.start_ms).collect::<Vec<_>>();
    let index = selected_day(selection.day, &starts);
    for (choice, interaction, mut color) in &mut buttons {
        let selected = matches!(choice, Choice::View(view) if *view == selection.view);
        let disabled = match choice {
            Choice::Previous => index.is_none_or(|index| index == 0),
            Choice::Next => index.is_none_or(|index| index + 1 == days.len()),
            _ => false,
        };
        let next = if selected {
            SELECTED
        } else if disabled {
            Color::srgb(0.08, 0.10, 0.12)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.16, 0.23, 0.26)
        } else {
            PANEL
        };
        if color.0 != next {
            color.0 = next;
        }
    }
    if selection.view != View::History {
        return;
    }
    let mut value = index.map_or_else(
        || "No citizen history available.".into(),
        |index| day_label(days[index], index + 1 == days.len()),
    );
    if let Some(index) = index {
        let start = days[index].start_ms;
        let planning = citizen
            .0
            .and_then(|id| snapshot.0.planning_history.get(&id))
            .and_then(|history| {
                history
                    .completed
                    .iter()
                    .chain(std::iter::once(&history.current))
                    .find(|day| day.start_ms == start)
            });
        let (request, wait, requests) = planning.map_or((0.0, 0.0, 0), |day| {
            (day.request_ms, day.wait_ms, day.requests)
        });
        value.push_str(&format!("\nCash totals are actual trades; inventory price changes do not affect them.\n\nBACKGROUND PLANNING · REAL WALL TIME\nRequest queue + work {:.3} s  |  {} requests\nBlocked at action boundary {:.3} s\nAttributed to plan adoption day; initial startup plans excluded.", request / 1000.0, requests, wait / 1000.0));
    }
    for mut readout in &mut readouts {
        if readout.0 != value {
            readout.0.clone_from(&value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_stays_in_retained_window_and_latest_follows_live_day() {
        let starts = [10, 20, 30];
        assert_eq!(selected_day(None, &starts), Some(2));
        assert_eq!(navigate(None, &starts, true), Some(20));
        assert_eq!(navigate(Some(10), &starts, true), Some(10));
        assert_eq!(navigate(Some(20), &starts, false), None);
        assert_eq!(navigate(None, &starts, false), None);
        assert_eq!(selected_day(Some(1), &starts), Some(0));
        assert_eq!(navigate(None, &[], true), None);
        assert_eq!(navigate(None, &[10], true), None);
    }

    #[test]
    fn history_formats_empty_and_partial_days_and_resets_selection() {
        use learning_lord_simulation::{Citizen, Universe};
        let (universe, id) = Universe::default()
            .with_citizen("Ada", Citizen::new(0.0).unwrap())
            .unwrap();
        let history = universe.citizen_history(id).unwrap();
        let label = day_label(&history.current, true);
        assert!(label.contains("PARTIAL"));
        assert!(label.contains("No elapsed time yet"));
        assert!(label.contains("No goods produced, traded or consumed yet."));
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(crate::simulation::Snapshot {
            universe,
            planning_history: Default::default(),
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            generation: 0,
            revision: 0,
        }))
        .insert_resource(citizens::Selection(Some(id)))
        .insert_resource(Selection {
            view: View::History,
            day: Some(123),
            citizen: Some(id),
            generation: 0,
        })
        .init_resource::<InputFocus>()
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                spawn_tabs(parent);
                spawn(parent);
            });
        })
        .add_systems(Update, (handle_selection, refresh).chain());
        app.update();
        let value = app
            .world_mut()
            .query_filtered::<&Text, With<HistoryText>>()
            .single(app.world())
            .unwrap();
        assert!(value.0.contains("PARTIAL"));
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation = 1;
        app.update();
        assert_eq!(app.world().resource::<Selection>().day, None);
        assert!(app.world().resource::<Selection>().view == View::History);
    }
}
