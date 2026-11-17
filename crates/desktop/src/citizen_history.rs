use crate::{DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, citizens, text};
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use learning_lord_simulation::marketplace::Good;

#[derive(Clone, Copy, Default, PartialEq)]
pub enum View {
    #[default]
    Needs,
    Belongings,
    Planning,
    History,
}

#[derive(Default)]
pub struct Selection {
    view: View,
    day: Option<u64>,
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

#[derive(Component, Default)]
pub struct HistoryGoods(Option<Vec<(Good, String)>>);

fn button(choice: Choice) -> impl Bundle {
    (
        Button,
        choice,
        Node {
            min_height: px(27),
            padding: UiRect::axes(px(9), px(4)),
            align_items: AlignItems::Center,

            ..default()
        },
        BackgroundColor(PANEL),
    )
}

pub fn spawn_tabs(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            column_gap: px(8),
            row_gap: px(4),
            flex_wrap: FlexWrap::Wrap,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (view, label) in [
                (View::Needs, "Needs"),
                (View::Belongings, "Belongings"),
                (View::Planning, "Planning"),
                (View::History, "Daily history"),
            ] {
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
                row_gap: px(6),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|panel| {
            panel.spawn((text("Daily history (i)", 14.0, MUTED), Interaction::None,
                crate::tooltip::Tooltip("Food goals include meals and overlap activity times. Days close at 04:00; latest 30 completed days. Historical average wellbeing includes personal wealth. Cash totals are actual trades; price changes do not affect them. Background planning is real wall time attributed to plan adoption day; initial startup plans are excluded.".into())));
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
            panel.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    flex_shrink: 0.0,
                    ..default()
                },
                HistoryGoods::default(),
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
    mut panes: Query<(Entity, &mut citizens::Detail)>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    buttons: Query<
        (Entity, &Interaction, Option<&Choice>),
        (
            Changed<Interaction>,
            Or<(With<Choice>, With<citizens::SkillsToggle>)>,
        ),
    >,
    mut focus: ResMut<InputFocus>,
    mut scroll: Query<&mut ScrollPosition, With<citizens::DetailScroll>>,
) {
    let mut reset_scroll = |pane| {
        if let Ok(children) = children.get(pane) {
            for child in children.iter() {
                if let Ok(mut position) = scroll.get_mut(child) {
                    position.0 = Vec2::ZERO;
                }
            }
        }
    };
    for (pane, mut detail) in &mut panes {
        if detail.history.generation != snapshot.0.generation {
            detail.history.day = None;
            detail.history.generation = snapshot.0.generation;
            reset_scroll(pane);
        }
    }
    for (entity, interaction, choice) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pane) = parents
            .iter_ancestors(entity)
            .find(|id| panes.contains(*id))
        else {
            continue;
        };
        let (_, mut detail) = panes.get_mut(pane).expect("ancestor has detail");
        let Some(choice) = choice else {
            detail.skills = !detail.skills;
            continue;
        };
        focus.set(entity, FocusCause::Pressed);
        match choice {
            Choice::View(view) => detail.history.view = *view,
            Choice::Previous | Choice::Next => {
                let starts = snapshot
                    .0
                    .universe
                    .citizen_history(detail.citizen)
                    .map_or_else(Vec::new, |history| {
                        history
                            .completed
                            .iter()
                            .chain(std::iter::once(&history.current))
                            .map(|day| day.start_ms)
                            .collect()
                    });
                detail.history.day = navigate(
                    detail.history.day,
                    &starts,
                    matches!(choice, Choice::Previous),
                );
            }
        }
        reset_scroll(pane);
    }
}

#[cfg(test)]
fn goods_label(day: &learning_lord_simulation::history::DailyCitizenHistory) -> String {
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

#[cfg(test)]
pub(crate) fn day_label(
    day: &learning_lord_simulation::history::DailyCitizenHistory,
    partial: bool,
) -> String {
    day_label_with_goods(
        day,
        partial,
        &format!(
            "GOODS\n{}\nConsumed includes recipe inputs, food and water reserved when eating or drinking starts, and equipped clothing.\n\n",
            goods_label(day)
        ),
    )
}

fn day_label_with_goods(
    day: &learning_lord_simulation::history::DailyCitizenHistory,
    partial: bool,
    goods: &str,
) -> String {
    let a = day.activity;
    format!(
        "{} to {}\n{}  |  {} observed\n\nACTUAL ACTIVITY TIME\nProduction / gathering {}  |  Travel {}  |  Sleep {}\nMarket activity {}  |  Eating {}  |  Drinking {}  |  Other / idle / waiting {}\nFood goals {}\n\n{}COINS\nEarned {}  |  Spent {}  |  Net {:+}{}\n\nWELLBEING AND NEEDS\nAverage wellbeing {}\nWorst hunger {:.1}  |  Thirst {:.1}  |  Tiredness {:.1}  |  Clothing need {:.1}  |  Discomfort {:.1}",
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
        citizens::duration(a.drinking_ms),
        citizens::duration(a.idle_ms),
        citizens::duration(day.food_goal_ms),
        goods,
        day.coins_earned,
        day.coins_spent,
        day.coins_earned - day.coins_spent,
        crate::construction::cashflow(day.construction)
            .map_or_else(String::new, |line| format!("\n{line}")),
        day.average_wellbeing().map_or_else(
            || "No elapsed time yet".into(),
            |value| format!("{value:.1}")
        ),
        day.worst_hunger,
        day.worst_thirst,
        day.worst_tiredness,
        day.worst_clothing_need,
        day.worst_discomfort
    )
}

pub fn refresh(
    snapshot: Res<DisplaySnapshot>,
    panes: Query<(Entity, Ref<citizens::Detail>)>,
    children: Query<&Children>,
    mut panels: Query<(&ViewPanel, &mut Node, &mut Visibility)>,
    mut buttons: Query<(&Choice, Ref<Interaction>, &mut BackgroundColor)>,
    mut readouts: Query<&mut Text, With<HistoryText>>,
    mut commands: Commands,
    mut goods_panels: Query<&mut HistoryGoods>,
) {
    for (pane, detail) in &panes {
        if !detail.visible {
            continue;
        }
        let selection = &detail.history;
        let citizen = detail.citizen;
        let descendants = || children.iter_descendants(pane);
        if !snapshot.is_changed()
            && !detail.is_changed()
            && !descendants().any(|entity| {
                buttons
                    .get(entity)
                    .is_ok_and(|(_, interaction, _)| interaction.is_changed())
            })
        {
            continue;
        }
        for entity in descendants() {
            let Ok((panel, mut node, mut visibility)) = panels.get_mut(entity) else {
                continue;
            };
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
        let history = snapshot.0.universe.citizen_history(citizen);
        let days = history.map_or_else(Vec::new, |history| {
            history
                .completed
                .iter()
                .chain(std::iter::once(&history.current))
                .collect::<Vec<_>>()
        });
        let starts = days.iter().map(|day| day.start_ms).collect::<Vec<_>>();
        let index = selected_day(selection.day, &starts);
        for entity in descendants() {
            let Ok((choice, interaction, mut color)) = buttons.get_mut(entity) else {
                continue;
            };
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
            continue;
        }
        let mut value = index.map_or_else(
            || "No citizen history available.".into(),
            |index| day_label_with_goods(days[index], index + 1 == days.len(), ""),
        );
        if let Some(index) = index {
            let start = days[index].start_ms;
            let planning = snapshot
                .0
                .planning_history
                .get(&citizen)
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
            value.push_str(&format!("\n\nBACKGROUND PLANNING · REAL WALL TIME\nRequest queue + work {:.3} s  |  {} requests\nBlocked at action boundary {:.3} s", request / 1000.0, requests, wait / 1000.0));
        }
        let goods = index.map_or_else(Vec::new, |index| {
            let day = days[index];
            Good::ALL
                .into_iter()
                .filter_map(|good| {
                    let i = good as usize;
                    let units = [day.produced[i], day.bought[i], day.sold[i], day.consumed[i]];
                    units.iter().any(|units| *units != 0).then(|| {
                        (
                            good,
                            format!(
                                "produced {} · bought {} · sold {} · consumed {}",
                                crate::quantity_label(good, units[0]),
                                crate::quantity_label(good, units[1]),
                                crate::quantity_label(good, units[2]),
                                crate::quantity_label(good, units[3])
                            ),
                        )
                    })
                })
                .collect::<Vec<_>>()
        });
        for entity in descendants() {
            if let Ok(mut readout) = readouts.get_mut(entity) {
                if readout.0 != value {
                    readout.0.clone_from(&value);
                }
            }
            if let Ok(mut cache) = goods_panels.get_mut(entity)
                && cache.0.as_ref() != Some(&goods)
            {
                commands.entity(entity).despawn_children().with_children(|panel| {
                panel.spawn((text("Goods (i)", 14.0, MUTED), Interaction::None, crate::tooltip::Tooltip("Consumed includes recipe inputs, food and water reserved when eating or drinking starts, and equipped clothing.".into())));
                if goods.is_empty() {
                    panel.spawn(text("No goods produced, traded or consumed yet.", 14.0, TEXT));
                }
                for (good, metrics) in &goods {
                    panel.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: px(6), ..default() }).with_children(|row| {
                        crate::navigation::link(row, good.name(), crate::navigation::Target::Good(*good));
                        row.spawn(text(metrics, 14.0, TEXT));
                    });
                }
            });
                cache.0 = Some(goods.clone());
            }
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
            .with_citizen(
                "Ada",
                Citizen::new(0.0).unwrap().with_furnished_home().unwrap(),
            )
            .unwrap();
        let history = universe.citizen_history(id).unwrap();
        let mut day = history.current.clone();
        day.activity.drinking_ms = 60_000;
        day.worst_thirst = 125.0;
        let label = day_label(&day, true);
        assert!(label.contains("Drinking 00:01:00"));
        assert!(label.contains("Thirst 125.0"));
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
        .init_resource::<InputFocus>()
        .add_systems(Startup, move |mut commands: Commands| {
            let mut detail = citizens::Detail::new(id);
            detail.history.day = Some(123);
            detail.history.view = View::History;
            commands
                .spawn((Node::default(), detail))
                .with_children(|parent| {
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
        let detail = app
            .world_mut()
            .query::<&citizens::Detail>()
            .single(app.world())
            .unwrap();
        assert_eq!(detail.history.day, None);
        assert!(detail.history.view == View::History);
    }
}
