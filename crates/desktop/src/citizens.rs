use crate::{DisplaySnapshot, MUTED, PANEL, Readout, SELECTED, TEXT, action_label, text};
use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    ui::RelativeCursorPosition,
};
use learning_lord_simulation::{
    Agent, AgentId, AgentKind, Citizen, Universe, planning::goals::Effect,
};

#[derive(Resource, Default)]
pub struct Selection(pub Option<AgentId>);

#[derive(Resource, Default)]
pub struct PlanDisplay {
    selected: Option<AgentId>,
    rows: Vec<PlanRow>,
}

#[derive(Component)]
pub struct Card(usize);
#[derive(Component)]
pub struct Summary(usize);
#[derive(Component)]
pub struct NeedLabel(usize, bool);
#[derive(Component)]
pub struct NeedFill(usize, bool);
#[derive(Component)]
pub struct PlanContent;
#[derive(Component)]
pub struct DetailScroll;

#[derive(Clone, PartialEq, Debug)]
struct PlanRow {
    label: String,
    state: RowState,
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum RowState {
    Heading,
    Complete,
    Current,
    Upcoming,
}

pub fn sorted_agents(universe: &Universe) -> Vec<(AgentId, &Agent)> {
    let mut agents: Vec<_> = universe
        .agents()
        .iter()
        .map(|(&id, agent)| (id, agent))
        .collect();
    agents.sort_by(|(a_id, a), (b_id, b)| a.name.cmp(&b.name).then_with(|| a_id.0.cmp(&b_id.0)));
    agents
}

pub fn selected_agent(universe: &Universe, id: Option<AgentId>) -> Option<&Agent> {
    id.and_then(|id| universe.agents().get(&id))
        .or_else(|| sorted_agents(universe).first().map(|(_, agent)| *agent))
}

pub fn spawn_roster(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            width: percent(100),
            column_gap: px(10),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for slot in 0..4 {
                row.spawn((
                    Button,
                    Card(slot),
                    Node {
                        flex_basis: px(0),
                        flex_grow: 1.0,
                        min_width: px(0),
                        padding: UiRect::all(px(10)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        border_radius: BorderRadius::all(px(8)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                ))
                .with_children(|card| {
                    card.spawn((text("", 15.0, TEXT), Summary(slot)));
                    for sleep in [false, true] {
                        card.spawn((text("", 12.0, TEXT), NeedLabel(slot, sleep)));
                        card.spawn((
                            Node {
                                width: percent(100),
                                height: px(7),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.04, 0.06, 0.08)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    height: percent(100),
                                    ..default()
                                },
                                BackgroundColor(SELECTED),
                                NeedFill(slot, sleep),
                            ));
                            bar.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: percent(50),
                                    width: px(1),
                                    height: percent(100),
                                    ..default()
                                },
                                BackgroundColor(MUTED),
                            ));
                        });
                    }
                });
            }
        });
}

pub fn spawn_details(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_basis: px(0),
                flex_grow: 1.0,
                min_width: px(0),
                min_height: px(0),
                padding: UiRect::all(px(16)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                overflow: Overflow::scroll_y(),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(PANEL),
            ScrollPosition::default(),
            RelativeCursorPosition::default(),
            DetailScroll,
        ))
        .with_children(|panel| {
            panel.spawn((text("", 17.0, TEXT), Readout::Citizen));
            panel.spawn(text(
                "ACTIVE PLAN  |  durations are planning estimates",
                14.0,
                MUTED,
            ));
            panel.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3),
                    flex_shrink: 0.0,
                    ..default()
                },
                PlanContent,
            ));
            panel.spawn((text("", 14.0, MUTED), Readout::Decision));
        });
}

pub fn handle_selection(
    snapshot: Res<DisplaySnapshot>,
    mut selection: ResMut<Selection>,
    cards: Query<(&Interaction, &Card), Changed<Interaction>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let agents = sorted_agents(&snapshot.0.universe);
    if selection
        .0
        .is_none_or(|id| !snapshot.0.universe.agents().contains_key(&id))
    {
        let next = agents.first().map(|(id, _)| *id);
        if selection.0 != next {
            selection.0 = next;
        }
    }
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            let next = agents.get(card.0).map(|(id, _)| *id);
            if selection.0 != next {
                selection.0 = next;
            }
        }
    }
    for (slot, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keyboard.just_pressed(key) {
            let next = agents.get(slot).map(|(id, _)| *id);
            if selection.0 != next {
                selection.0 = next;
            }
        }
    }
}

pub fn duration(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

pub fn location_label(citizen: &Citizen) -> String {
    use learning_lord_simulation::CitizenAction;
    let map = citizen.map();
    if let Some(active) = citizen.active_action()
        && let CitizenAction::Travel(destination) = active.action()
    {
        return format!("Travelling to {}", map.place(destination).unwrap().name);
    }
    map.places()
        .values()
        .filter(|place| place.position == citizen.position())
        .min_by_key(|place| (place.id != citizen.home(), place.name.clone()))
        .map_or_else(
            || {
                format!(
                    "({:.0}, {:.0}) m",
                    citizen.position().x,
                    citizen.position().y
                )
            },
            |place| place.name.clone(),
        )
}

fn need_label(value: f64, sleep: bool) -> &'static str {
    if value < -100.0 {
        "Overfull"
    } else if value < 0.0 {
        if sleep { "Rested" } else { "Satiated" }
    } else if value == 0.0 {
        "Balanced"
    } else if value <= 50.0 {
        if sleep { "Tiring" } else { "Hungry" }
    } else if value <= 100.0 {
        if sleep { "Tired" } else { "Very hungry" }
    } else {
        "URGENT >100"
    }
}

fn need_geometry(value: f64) -> (f32, f32) {
    let value = if value.is_finite() {
        value.clamp(-100.0, 100.0) as f32
    } else {
        0.0
    };
    (50.0 + value.min(0.0) / 2.0, value.abs() / 2.0)
}

fn goal_label(goal: Effect) -> &'static str {
    match goal {
        Effect::ReduceHunger => "Reduce Hunger",
        Effect::ReduceTiredness => "Reduce Sleep Need",
        Effect::IncreaseWealth => "Increase Wealth",
        Effect::Production => "Production",
        Effect::ReplenishReserves => "Replenish reserves",
        Effect::ListExcess => "List excess",
        _ => unreachable!("plan boundaries contain goals"),
    }
}

fn plan_rows(citizen: &Citizen) -> Vec<PlanRow> {
    let Some(active) = citizen.active_plan() else {
        return vec![PlanRow {
            label: "No active plan".into(),
            state: RowState::Upcoming,
        }];
    };
    let plan = active.plan();
    let mut rows = Vec::new();
    for boundary in plan.goals() {
        let total: u64 = plan.action_durations_ms()[boundary.actions.clone()]
            .iter()
            .sum();
        rows.push(PlanRow {
            label: format!("{}  |  {}", goal_label(boundary.goal), duration(total)),
            state: RowState::Heading,
        });
        for index in boundary.actions.clone() {
            let state = if index < active.action_index() {
                RowState::Complete
            } else if index == active.action_index() {
                RowState::Current
            } else {
                RowState::Upcoming
            };
            let status = match state {
                RowState::Complete => "done",
                RowState::Current => "NOW",
                _ => "next",
            };
            let timing = if state == RowState::Current {
                citizen
                    .active_action()
                    .map(|a| format!("{} left", duration(a.remaining_ms())))
                    .unwrap_or_default()
            } else {
                duration(plan.action_durations_ms()[index])
            };
            rows.push(PlanRow {
                label: format!(
                    "    {status:4}  {}  |  {timing}",
                    action_label(plan.actions()[index], citizen)
                ),
                state,
            });
        }
    }
    rows
}

pub fn refresh_cards(
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    mut summaries: Query<(&Summary, &mut Text), Without<NeedLabel>>,
    mut labels: Query<(&NeedLabel, &mut Text), Without<Summary>>,
    mut fills: Query<(&NeedFill, &mut Node, &mut BackgroundColor), Without<Card>>,
    mut cards: Query<(&Card, &Interaction, &mut BackgroundColor), Without<NeedFill>>,
) {
    let agents = sorted_agents(&snapshot.0.universe);
    for (summary, mut value) in &mut summaries {
        if let Some((id, agent)) = agents.get(summary.0) {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let action = citizen.active_action().map_or_else(
                || "Idle".into(),
                |active| {
                    format!(
                        "{} | {} left",
                        action_label(active.action(), citizen),
                        duration(active.remaining_ms())
                    )
                },
            );
            let wellbeing = citizen
                .personal_wellbeing()
                .map_or_else(|e| e.to_string(), |v| format!("{v:.1}"));
            let wealth = citizen
                .wealth()
                .map_or_else(|e| e.to_string(), |v| format!("{v:.3}"));
            let next = format!(
                "{}{}\nCoins {}  |  Wealth {}\nWellbeing {}  |  {}",
                agent.name,
                if Some(*id) == selection.0 {
                    "  [selected]"
                } else {
                    ""
                },
                citizen.coins(),
                wealth,
                wellbeing,
                action
            );
            if value.0 != next {
                value.0 = next;
            }
        } else {
            let next = "No citizen".into();
            if value.0 != next {
                value.0 = next;
            }
        }
    }
    for (label, mut value) in &mut labels {
        if let Some((_, agent)) = agents.get(label.0) {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let next = format!(
                "{}: {}   (- < 0 > +)",
                if label.1 { "Sleep" } else { "Hunger" },
                need_label(
                    if label.1 {
                        citizen.tiredness()
                    } else {
                        citizen.hunger()
                    },
                    label.1
                )
            );
            if value.0 != next {
                value.0 = next;
            }
        }
    }
    for (fill, mut node, mut color) in &mut fills {
        if let Some((_, agent)) = agents.get(fill.0) {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let value = if fill.1 {
                citizen.tiredness()
            } else {
                citizen.hunger()
            };
            let (left, width) = need_geometry(value);
            let next = percent(left);
            if node.left != next {
                node.left = next;
            }
            let next = percent(width);
            if node.width != next {
                node.width = next;
            }
            let next = if !(-100.0..=100.0).contains(&value) {
                Color::srgb(0.95, 0.35, 0.25)
            } else if value > 50.0 {
                Color::srgb(0.88, 0.65, 0.23)
            } else {
                Color::srgb(0.24, 0.67, 0.53)
            };
            if color.0 != next {
                color.0 = next;
            }
        }
    }
    for (card, interaction, mut color) in &mut cards {
        let next = if agents
            .get(card.0)
            .is_some_and(|(id, _)| Some(*id) == selection.0)
        {
            SELECTED
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.16, 0.23, 0.26)
        } else {
            PANEL
        };
        if color.0 != next {
            color.0 = next;
        }
    }
}

pub fn refresh_plan(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    mut cached: ResMut<PlanDisplay>,
    content: Query<Entity, With<PlanContent>>,
    mut scroll: Query<&mut ScrollPosition, With<DetailScroll>>,
) {
    let rows = selected_agent(&snapshot.0.universe, selection.0).map_or_else(Vec::new, |agent| {
        let AgentKind::Citizen(citizen) = &agent.kind;
        plan_rows(citizen)
    });
    if cached.selected != selection.0 {
        for mut position in &mut scroll {
            position.0 = Vec2::ZERO;
        }
    }
    if rows != cached.rows || cached.selected != selection.0 {
        for entity in &content {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|parent| {
                    for row in &rows {
                        let (size, color) = match row.state {
                            RowState::Heading => (17.0, TEXT),
                            RowState::Complete => (14.0, MUTED),
                            RowState::Current => (15.0, Color::srgb(0.40, 0.90, 0.70)),
                            RowState::Upcoming => (14.0, TEXT),
                        };
                        parent.spawn((
                            text(&row.label, size, color),
                            Node {
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                    }
                });
        }
        cached.rows = rows;
        cached.selected = selection.0;
    }
}

pub fn scroll_panels(
    mut wheel: MessageReader<MouseWheel>,
    mut panels: Query<(
        &RelativeCursorPosition,
        &ComputedNode,
        &mut ScrollPosition,
        &InheritedVisibility,
    )>,
) {
    for event in wheel.read() {
        let delta = -event.y
            * if event.unit == MouseScrollUnit::Line {
                30.0
            } else {
                1.0
            };
        for (cursor, computed, mut scroll, visibility) in &mut panels {
            if !visibility.get() {
                continue;
            }
            let max = ((computed.content_size().y - computed.size().y)
                * computed.inverse_scale_factor())
            .max(0.0);
            scroll.0.y = if cursor.cursor_over {
                (scroll.0.y + delta).clamp(0.0, max)
            } else {
                scroll.0.y.min(max)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{self, Snapshot};
    use learning_lord_simulation::{CitizenAction, locations::Map};

    #[test]
    fn signed_need_bars_cover_negative_zero_clamped_and_urgent_values() {
        assert_eq!(need_geometry(-50.0), (25.0, 25.0));
        assert_eq!(need_geometry(0.0), (50.0, 0.0));
        assert_eq!(need_geometry(50.0), (50.0, 25.0));
        assert_eq!(need_geometry(-200.0), (0.0, 50.0));
        assert_eq!(need_geometry(200.0), (50.0, 50.0));
        assert_eq!(need_label(-200.0, false), "Overfull");
        assert_eq!(need_label(-50.0, false), "Satiated");
        assert_eq!(need_label(-50.0, true), "Rested");
        assert_eq!(need_label(101.0, false), "URGENT >100");
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let (left, width) = need_geometry(value);
            assert!(left.is_finite() && width.is_finite());
        }
    }

    #[test]
    fn selection_survives_snapshots_and_repairs_after_restart() {
        let universe = simulation::new_universe().unwrap();
        let sorted = sorted_agents(&universe);
        assert_eq!(
            sorted
                .iter()
                .map(|(_, agent)| agent.name.as_str())
                .collect::<Vec<_>>(),
            ["Ada", "Bram", "Cleo", "Dara"]
        );
        let dara = sorted[3].0;
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe,
            error: None,
            generation: 0,
            revision: 0,
        }))
        .init_resource::<Selection>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, handle_selection);
        let card = app.world_mut().spawn((Card(3), Interaction::Pressed)).id();
        app.update();
        assert_eq!(app.world().resource::<Selection>().0, Some(dara));
        app.world_mut().entity_mut(card).insert(Interaction::None);
        let next = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .advance(1)
            .unwrap();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = next;
        app.update();
        assert_eq!(app.world().resource::<Selection>().0, Some(dara));
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe =
            simulation::new_universe().unwrap();
        app.update();
        let selected = app.world().resource::<Selection>().0.unwrap();
        let snapshot = &app.world().resource::<DisplaySnapshot>().0.universe;
        assert_ne!(selected, dara);
        assert_eq!(snapshot.agents()[&selected].name, "Ada");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit2);
        app.update();
        let selected = app.world().resource::<Selection>().0.unwrap();
        assert_eq!(
            app.world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .agents()[&selected]
                .name,
            "Bram"
        );
    }

    #[test]
    fn details_show_selected_citizen_and_full_plan_with_completed_actions() {
        let (universe, ada) = Universe::with_map(Map::default())
            .with_citizen(
                "Ada",
                Citizen::with_needs(60.0, 100.0)
                    .unwrap()
                    .with_berries(310)
                    .unwrap(),
            )
            .unwrap();
        let (universe, bram) = universe
            .with_citizen("Bram", Citizen::new(-10.0).unwrap())
            .unwrap();
        let started = universe.start_planning(ada).unwrap();
        let progressed = started.advance(155_000).unwrap();
        let AgentKind::Citizen(citizen) = &progressed.agents()[&ada].kind;
        assert_eq!(
            citizen.active_action().unwrap().action(),
            CitizenAction::Sleep
        );
        let rows = plan_rows(citizen);
        assert_eq!(
            rows.iter()
                .filter(|row| row.state != RowState::Heading)
                .count(),
            citizen.active_plan().unwrap().plan().actions().len()
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.state == RowState::Complete)
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.state == RowState::Current)
                .count(),
            1
        );
        assert!(rows.iter().any(|row| row.label.contains("Reduce Hunger")));
        assert!(
            rows.iter()
                .any(|row| row.label.contains("Reduce Sleep Need"))
        );
        let details = crate::citizen_readout(&progressed, Some(bram));
        assert!(details.starts_with("Bram"));
        assert!(details.contains("Hunger: -9.8"));
        assert!(details.contains("Location: Bram's home"));
        assert!(!details.contains("Berries: 0 g"));
    }
}
