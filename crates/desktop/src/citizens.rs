use crate::{DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, text};
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
pub struct RosterState {
    generation: u64,
    pub(crate) detail: bool,
}

#[derive(Resource, Default)]
pub struct PlanDisplay {
    selected: Option<AgentId>,
    rows: Vec<PlanRow>,
}

#[derive(Component)]
pub struct Card(usize);
#[derive(Component)]
pub struct Summary(usize, usize);
#[derive(Component)]
pub struct Screen(pub bool);
#[derive(Component)]
pub(crate) struct Back;
#[derive(Component)]
pub(crate) struct SkillsToggle;
#[derive(Component)]
pub(crate) struct SkillsLabel;
#[derive(Resource, Default)]
pub struct SkillsExpanded(bool);
#[derive(Component)]
pub(crate) struct RosterContent(usize);
#[derive(Clone, Copy)]
pub enum Need {
    Hunger,
    Thirst,
    Sleep,
    Clothing,
}

impl Need {
    fn value(self, citizen: &Citizen) -> f64 {
        match self {
            Self::Hunger => citizen.hunger(),
            Self::Thirst => citizen.thirst(),
            Self::Sleep => citizen.tiredness(),
            Self::Clothing => citizen.clothing_need(),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Hunger => "Hunger",
            Self::Thirst => "Thirst",
            Self::Sleep => "Sleep",
            Self::Clothing => "Clothing",
        }
    }
}

#[derive(Component)]
pub struct DietLabel(learning_lord_simulation::diet::DietCategory);
#[derive(Component)]
pub struct DietFill(learning_lord_simulation::diet::DietCategory);

#[derive(Component)]
pub struct NeedLabel(Need);
#[derive(Component)]
pub struct NeedFill(Need);
#[derive(Component)]
pub struct PlanContent;
#[derive(Component)]
pub struct DetailScroll;

#[derive(Clone, PartialEq, Debug)]
struct PlanRow {
    label: String,
    fragments: Vec<crate::navigation::Fragment>,
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

const COLUMNS: [(&str, f32); 5] = [
    ("Name", 24.0),
    ("Profession", 23.0),
    ("Coins", 17.0),
    ("Wealth", 18.0),
    ("Wellbeing", 18.0),
];

fn cell(parent: &mut ChildSpawnerCommands, width: f32, label: &str, color: Color) -> Entity {
    parent
        .spawn(Node {
            width: percent(width),
            min_width: px(0),
            padding: UiRect::horizontal(px(6)),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|cell| {
            cell.spawn(text(label, 14.0, color));
        })
        .id()
}

fn roster_rows(parent: &mut ChildSpawnerCommands, count: usize, values: Option<&[[String; 5]]>) {
    if count == 0 {
        parent.spawn(text("No citizens", 14.0, MUTED));
    }
    for slot in 0..count {
        parent
            .spawn((
                Button,
                Card(slot),
                Node {
                    min_height: px(27),
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|row| {
                for (column, (_, width)) in COLUMNS.iter().enumerate() {
                    row.spawn((
                        Node {
                            width: percent(*width),
                            min_width: px(0),
                            padding: UiRect::horizontal(px(6)),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        crate::tooltip::Tooltip(String::new()),
                        Interaction::None,
                        bevy::ui::FocusPolicy::Pass,
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            text(
                                values
                                    .and_then(|rows| rows.get(slot))
                                    .map_or("", |row| row[column].as_str()),
                                14.0,
                                TEXT,
                            ),
                            Summary(slot, column),
                        ));
                    });
                }
            });
    }
}

pub fn spawn_roster(parent: &mut ChildSpawnerCommands, count: usize) {
    parent
        .spawn((
            Screen(false),
            Node {
                width: percent(100),
                min_width: px(0),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_x(),
                ..default()
            },
            ScrollPosition::default(),
            RelativeCursorPosition::default(),
        ))
        .with_children(|table| {
            table
                .spawn(Node {
                    min_height: px(29),
                    min_width: percent(100),
                    width: px(720),
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|row| {
                    for (label, width) in COLUMNS {
                        cell(row, width, label, MUTED);
                    }
                });
            table
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: percent(100),
                        width: px(720),
                        min_height: px(0),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    RosterContent(count),
                ))
                .with_children(|rows| roster_rows(rows, count, None));
        });
}

pub fn spawn_details(parent: &mut ChildSpawnerCommands) {
    use crate::citizen_history::{View, ViewPanel};
    parent.spawn((Screen(true), Visibility::Hidden, Node {
        display: Display::None, width: percent(100), flex_grow: 1.0, min_height: px(0),
        flex_direction: FlexDirection::Column, row_gap: px(6), ..default()
    })).with_children(|detail| {
        detail.spawn(Node { align_items: AlignItems::Center, column_gap: px(10), flex_shrink: 0.0, ..default() })
            .with_children(|header| {
                header.spawn((Button, Back, Node { padding: UiRect::axes(px(7), px(4)), ..default() }, BackgroundColor(PANEL)))
                    .with_children(|button| { button.spawn(text("< Citizens", 14.0, TEXT)); });
                header.spawn(LinkedReadout::bundle(Section::Header));
            });
        crate::citizen_history::spawn_tabs(detail);
        detail.spawn((Node {
            flex_grow: 1.0, min_height: px(0), padding: UiRect::axes(px(6), px(4)),
            flex_direction: FlexDirection::Column, overflow: Overflow::scroll(), ..default()
        }, ScrollPosition::default(), RelativeCursorPosition::default(), DetailScroll))
        .with_children(|body| {
            body.spawn((ViewPanel(View::Needs), Node { flex_direction: FlexDirection::Column,
                row_gap: px(7), flex_shrink: 0.0, ..default() })).with_children(|needs| {
                needs.spawn(LinkedReadout::bundle(Section::Needs));
                for need in [Need::Hunger, Need::Thirst, Need::Sleep, Need::Clothing] {
                    needs.spawn((text("", 14.0, TEXT), NeedLabel(need)));
                    needs.spawn((Node { width: percent(100), height: px(5), flex_shrink: 0.0, ..default() },
                        BackgroundColor(Color::srgb(0.04, 0.06, 0.08))))
                    .with_children(|bar| {
                        bar.spawn((Node { position_type: PositionType::Absolute, height: percent(100), ..default() },
                            BackgroundColor(SELECTED), NeedFill(need)));
                        if !matches!(need, Need::Clothing) {
                            bar.spawn((Node { position_type: PositionType::Absolute, left: percent(50), width: px(1),
                                height: percent(100), ..default() }, BackgroundColor(MUTED)));
                        }
                    });
                }
                for category in learning_lord_simulation::diet::DietCategory::ALL {
                    needs.spawn((text("", 14.0, TEXT), DietLabel(category), Interaction::None, crate::tooltip::Tooltip("Each diet category decays 50/day; +5 wellbeing at 50 and +7 at 100.".into())));
                    needs.spawn((Node { width: percent(100), height: px(5), flex_shrink: 0.0, ..default() },
                        BackgroundColor(Color::srgb(0.04, 0.06, 0.08))))
                    .with_children(|bar| { bar.spawn((Node { height: percent(100), ..default() },
                        BackgroundColor(SELECTED), DietFill(category))); });
                }
                needs.spawn(LinkedReadout::bundle(Section::Reserves));
            });
            body.spawn((ViewPanel(View::Belongings), Node { display: Display::None,
                flex_direction: FlexDirection::Column, row_gap: px(6), flex_shrink: 0.0, ..default() }, Visibility::Hidden))
                .with_children(|panel| { panel.spawn(LinkedReadout::bundle(Section::Belongings)); });
            body.spawn((ViewPanel(View::Planning), Node { display: Display::None,
                flex_direction: FlexDirection::Column, row_gap: px(6), flex_shrink: 0.0, ..default() }, Visibility::Hidden))
                .with_children(|panel| {
                    panel.spawn(LinkedReadout::bundle(Section::Planning));
                    panel.spawn((text("Active plan (i)", 14.0, MUTED), Interaction::None,
                        crate::tooltip::Tooltip("Durations are planning estimates. Later goals are forecasts after replanning.".into())));
                    panel.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), flex_shrink: 0.0, ..default() }, PlanContent));
                    panel.spawn((Button, SkillsToggle, Node { padding: UiRect::vertical(px(4)), ..default() }))
                        .with_children(|button| { button.spawn((text("Skills [+]", 14.0, TEXT), SkillsLabel)); });
                    panel.spawn(LinkedReadout::bundle(Section::Skills));
                    panel.spawn(LinkedReadout::bundle(Section::Decision));
                });
            crate::citizen_history::spawn(body);
        });
    });
}

pub fn refresh_screens(
    state: Res<RosterState>,
    mut screens: Query<(&Screen, &mut Node, &mut Visibility)>,
) {
    for (screen, mut node, mut visibility) in &mut screens {
        let visible = screen.0 == state.detail;
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn handle_selection(
    (snapshot, mut selection, mut page): (
        Res<DisplaySnapshot>,
        ResMut<Selection>,
        ResMut<RosterState>,
    ),
    cards: Query<(&Interaction, &Card), Changed<Interaction>>,
    back: Query<&Interaction, (With<Back>, Changed<Interaction>)>,
    toggles: Query<&Interaction, (With<SkillsToggle>, Changed<Interaction>)>,
    mut skills: Option<ResMut<SkillsExpanded>>,
    shortcuts: crate::KeyboardShortcuts,
    mut windows: Option<ResMut<crate::floating_ui::Windows>>,
) {
    let agents = sorted_agents(&snapshot.0.universe);
    if page.generation != snapshot.0.generation {
        page.generation = snapshot.0.generation;
        selection.0 = agents.first().map(|(id, _)| *id);
        page.detail = false;
    }
    if selection
        .0
        .is_none_or(|id| !snapshot.0.universe.agents().contains_key(&id))
    {
        let next = agents.first().map(|(id, _)| *id);
        if selection.0 != next {
            selection.0 = next;
        }
    }
    for interaction in &back {
        if *interaction == Interaction::Pressed {
            page.detail = false;
        }
    }
    for interaction in &toggles {
        if *interaction == Interaction::Pressed
            && let Some(skills) = skills.as_mut()
        {
            skills.0 = !skills.0;
        }
    }
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            let next = agents.get(card.0).map(|(id, _)| *id);
            if selection.0 != next {
                selection.0 = next;
            }
            page.detail = next.is_some();
            if let (Some(_), Some(windows)) = (next, windows.as_mut()) {
                windows.open(crate::floating_ui::WindowKind::Citizens);
            }
        }
    }
    for (slot, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ]
    .into_iter()
    .enumerate()
    {
        if !shortcuts.editing() && shortcuts.keyboard.just_pressed(key) {
            let next = agents.get(slot).map(|(id, _)| *id);
            if selection.0 != next {
                selection.0 = next;
            }
            page.detail = next.is_some();
            if let (Some(_), Some(windows)) = (next, windows.as_mut()) {
                windows.open(crate::floating_ui::WindowKind::Citizens);
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

fn thirst_label(value: f64) -> &'static str {
    if value < 0.0 {
        "Hydrated"
    } else if value == 0.0 {
        "Balanced"
    } else if value <= 50.0 {
        "Thirsty"
    } else if value <= 100.0 {
        "Very thirsty"
    } else {
        "URGENT >100"
    }
}

fn clothing_geometry(value: f64) -> (f32, f32) {
    let width = if value.is_finite() {
        (value / learning_lord_simulation::CLOTHING_MAX_PENALTY).clamp(0.0, 1.0) as f32 * 100.0
    } else {
        0.0
    };
    (0.0, width)
}

fn need_geometry(value: f64) -> (f32, f32) {
    let value = if value.is_finite() {
        value.clamp(-100.0, 100.0) as f32
    } else {
        0.0
    };
    (50.0 + value.min(0.0) / 2.0, value.abs() / 2.0)
}

pub(crate) fn goal_label(goal: Effect) -> &'static str {
    match goal {
        Effect::ReduceHunger => "Reduce Hunger",
        Effect::ReduceThirst => "Reduce Thirst",
        Effect::ReduceTiredness => "Reduce Sleep Need",
        Effect::ReduceClothingNeed => "Reduce Clothing Need",
        Effect::Production => "Production",
        Effect::ReplenishReserves => "Replenish reserves",
        Effect::ListExcess => "List excess",
        _ => unreachable!("plan boundaries contain goals"),
    }
}

fn plan_rows(citizen: &Citizen) -> Vec<PlanRow> {
    let Some(active) = citizen.active_plan() else {
        return vec![PlanRow {
            fragments: vec![crate::navigation::Fragment::text("No active plan")],
            label: "No active plan".into(),
            state: RowState::Upcoming,
        }];
    };
    let plan = active.plan();
    let mut rows = Vec::new();
    for (goal_index, boundary) in plan.goals().iter().enumerate() {
        let total: u64 = plan.action_durations_ms()[boundary.actions.clone()]
            .iter()
            .sum();
        let label = format!(
            "{}{}  |  {}",
            if goal_index == 0 {
                ""
            } else {
                "Forecast after replanning: "
            },
            goal_label(boundary.goal),
            duration(total)
        );
        rows.push(PlanRow {
            fragments: vec![crate::navigation::Fragment::text(&label)],
            label,
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
                _ if goal_index > 0 => "forecast",
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
            let mut fragments = vec![crate::navigation::Fragment::text(format!(
                "    {status:4}  "
            ))];
            fragments.extend(crate::navigation::action(plan.actions()[index], citizen));
            fragments.push(crate::navigation::Fragment::text(format!("  |  {timing}")));
            rows.push(PlanRow {
                label: crate::navigation::plain(&fragments),
                fragments,
                state,
            });
        }
    }
    rows
}

pub fn overview_values(universe: &Universe, agent: &Agent) -> [String; 5] {
    let AgentKind::Citizen(citizen) = &agent.kind;
    let shared = citizen.household_id();
    let coins = shared.map_or(citizen.coins(), |id| universe.households()[&id].coins);
    let wealth = shared.map_or_else(|| citizen.wealth(), |id| universe.household_wealth(id));
    [
        agent.name.clone(),
        citizen
            .starting_role()
            .map_or("-", |role| role.name())
            .into(),
        format!("{coins}{}", if shared.is_some() { " | H" } else { "" }),
        wealth.map_or_else(
            |error| error.to_string(),
            |value| format!("{value:.3}{}", if shared.is_some() { " | H" } else { "" }),
        ),
        format!("{:.1}", citizen.needs_wellbeing()),
    ]
}

impl Need {
    fn contribution(self, citizen: &Citizen) -> f64 {
        let value = self.value(citizen);
        match self {
            Self::Hunger => {
                -value.max(0.0) - 4.0 * (value - 100.0).max(0.0) - (-value - 100.0).max(0.0)
            }
            Self::Thirst => -value.max(0.0) - 4.0 * (value - 100.0).max(0.0),
            Self::Sleep | Self::Clothing => -value.max(0.0),
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct RosterQueries<'w, 's> {
    contents: Query<'w, 's, (Entity, &'static mut RosterContent)>,
    tips: Query<'w, 's, (&'static Children, &'static mut crate::tooltip::Tooltip)>,
}

pub fn refresh_cards(
    mut commands: Commands,
    (snapshot, selection): (Res<DisplaySnapshot>, Res<Selection>),
    mut summaries: Query<(&Summary, &mut Text), Without<NeedLabel>>,
    mut labels: Query<(&NeedLabel, &mut Text, &mut TextColor), Without<Summary>>,
    mut fills: Query<(&NeedFill, &mut Node, &mut BackgroundColor), Without<Card>>,
    mut cards: Query<(&Card, &Interaction, &mut BackgroundColor), Without<NeedFill>>,
    mut roster: RosterQueries,
) {
    let universe = &snapshot.0.universe;
    let agents = sorted_agents(universe);
    let refresh_values =
        snapshot.is_changed() || summaries.iter().any(|(_, value)| value.0.is_empty());
    let values = if refresh_values {
        agents
            .iter()
            .map(|(_, agent)| overview_values(universe, agent))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    for (entity, mut content) in &mut roster.contents {
        if content.0 != agents.len() {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|parent| roster_rows(parent, agents.len(), Some(&values)));
            content.0 = agents.len();
        }
    }
    if refresh_values {
        for (summary, mut value) in &mut summaries {
            let next = values
                .get(summary.0)
                .map_or_else(String::new, |cells| cells[summary.1].clone());
            if value.0 != next {
                value.0 = next;
            }
        }
    }
    for (children, mut tip) in &mut roster.tips {
        let Some((summary, _)) = children.iter().find_map(|child| summaries.get(child).ok()) else {
            continue;
        };
        let Some((_, agent)) = agents.get(summary.0) else {
            continue;
        };
        let AgentKind::Citizen(citizen) = &agent.kind;
        tip.0 = match summary.1 {
            2 | 3 => citizen.household_id().map_or_else(String::new, |id| format!("H | {}. Coins and wealth belong to the shared household account.", crate::locations::name(universe, id))),
            4 => "Needs wellbeing: hunger, thirst, sleep, clothing, diet and reserve contributions. Excludes wealth.".into(),
            _ => String::new(),
        };
    }
    let citizen = selected_agent(universe, selection.0).map(|agent| {
        let AgentKind::Citizen(citizen) = &agent.kind;
        citizen
    });
    for (label, mut value, mut color) in &mut labels {
        let Some(citizen) = citizen else {
            continue;
        };
        let need = label.0;
        let status = match need {
            Need::Clothing => citizen.garment_condition().map_or_else(
                || "No garment".into(),
                |v| format!("Garment {:.1}%", v * 100.0),
            ),
            Need::Thirst => thirst_label(citizen.thirst()).into(),
            _ => need_label(need.value(citizen), matches!(need, Need::Sleep)).into(),
        };
        let contribution = need.contribution(citizen);
        let next = format!(
            "{} {:.1} | {} | {:+.2} wellbeing",
            need.name(),
            need.value(citizen),
            status,
            if contribution == 0.0 {
                0.0
            } else {
                contribution
            }
        );
        if value.0 != next {
            value.0 = next;
        }
        color.0 = if contribution == 0.0 { MUTED } else { TEXT };
    }
    for (fill, mut node, mut color) in &mut fills {
        let Some(citizen) = citizen else {
            continue;
        };
        let value = fill.0.value(citizen);
        let (left, width) = if matches!(fill.0, Need::Clothing) {
            clothing_geometry(value)
        } else {
            need_geometry(value)
        };
        node.left = percent(left);
        node.width = percent(width);
        color.0 = if fill.0.contribution(citizen) == 0.0 {
            MUTED
        } else if value > 100.0 || value < -100.0 {
            Color::srgb(0.95, 0.35, 0.25)
        } else {
            Color::srgb(0.88, 0.65, 0.23)
        };
    }
    for (card, interaction, mut color) in &mut cards {
        color.0 = if *interaction == Interaction::Hovered {
            Color::srgb(0.16, 0.23, 0.26)
        } else if agents
            .get(card.0)
            .is_some_and(|(id, _)| Some(*id) == selection.0)
        {
            SELECTED
        } else {
            PANEL
        };
    }
}

pub fn refresh_diet(
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    mut labels: Query<(&DietLabel, &mut Text, &mut TextColor)>,
    mut fills: Query<(&DietFill, &mut Node, Option<&mut BackgroundColor>)>,
) {
    let citizen = selected_agent(&snapshot.0.universe, selection.0).map(|agent| {
        let AgentKind::Citizen(citizen) = &agent.kind;
        citizen
    });
    for (label, mut text, mut color) in &mut labels {
        let next = citizen.map_or_else(
            || "No citizen".into(),
            |citizen| {
                let value = citizen.diet(label.0);
                format!(
                    "{}: {value:.1}/100 | +{:.2} wellbeing",
                    label.0.name(),
                    learning_lord_simulation::diet::category_bonus(value)
                )
            },
        );
        if text.0 != next {
            text.0 = next;
        }
        color.0 = if citizen.is_none_or(|citizen| citizen.diet(label.0) == 0.0) {
            MUTED
        } else {
            TEXT
        };
    }
    for (fill, mut node, color) in &mut fills {
        if let Some(mut color) = color {
            color.0 = if citizen.is_none_or(|citizen| citizen.diet(fill.0) == 0.0) {
                MUTED
            } else {
                SELECTED
            };
        }
        let width = percent(citizen.map_or(0.0, |citizen| citizen.diet(fill.0) as f32));
        if node.width != width {
            node.width = width;
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
    if content.is_empty() {
        return;
    }
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
                            RowState::Heading => (14.0, TEXT),
                            RowState::Complete => (14.0, MUTED),
                            RowState::Current => (14.0, Color::srgb(0.40, 0.90, 0.70)),
                            RowState::Upcoming => (14.0, TEXT),
                        };
                        crate::navigation::render(parent, &row.fragments, size, color);
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
        Entity,
        &Node,
        &RelativeCursorPosition,
        &ComputedNode,
        &mut ScrollPosition,
        &InheritedVisibility,
    )>,
    capture: Option<Res<crate::floating_ui::InputCapture>>,
    parents: Query<&ChildOf>,
    windows: Query<&crate::floating_ui::FloatingWindow>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
) {
    for event in wheel.read() {
        let multiplier = if event.unit == MouseScrollUnit::Line {
            30.0
        } else {
            1.0
        };
        let horizontal = keyboard
            .as_ref()
            .is_some_and(|keys| keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
        let delta = Vec2::new(
            -(event.x + if horizontal { event.y } else { 0.0 }),
            if horizontal { 0.0 } else { -event.y },
        ) * multiplier;
        for (entity, node, cursor, computed, mut scroll, visibility) in &mut panels {
            if let Some(capture) = capture.as_ref() {
                let mut ancestor = entity;
                let mut owner = None;
                loop {
                    if let Ok(window) = windows.get(ancestor) {
                        owner = Some(window.0);
                        break;
                    }
                    let Ok(parent) = parents.get(ancestor) else {
                        break;
                    };
                    ancestor = parent.parent();
                }
                if owner != capture.hovered {
                    continue;
                }
            }
            if !visibility.get() {
                continue;
            }
            let max = ((computed.content_size() - computed.size())
                * computed.inverse_scale_factor())
            .max(Vec2::ZERO);
            let mut next = scroll.0;
            if cursor.cursor_over {
                if node.overflow.x == bevy::ui::OverflowAxis::Scroll {
                    next.x += delta.x;
                }
                if node.overflow.y == bevy::ui::OverflowAxis::Scroll {
                    next.y += delta.y;
                }
            }
            next = next.clamp(Vec2::ZERO, max);
            if scroll.0 != next {
                scroll.0 = next;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Header,
    Needs,
    Reserves,
    Belongings,
    Planning,
    Skills,
    Decision,
}

#[derive(Clone, Debug, PartialEq)]
struct ReadoutRow {
    fragments: Vec<crate::navigation::Fragment>,
    muted: bool,
    tooltip: String,
    table: bool,
}
impl ReadoutRow {
    fn text(value: impl Into<String>) -> Self {
        Self::rich(vec![crate::navigation::Fragment::text(value)])
    }
    fn rich(fragments: Vec<crate::navigation::Fragment>) -> Self {
        Self {
            fragments,
            muted: false,
            tooltip: String::new(),
            table: false,
        }
    }
    fn contribution(value: String, contribution: f64) -> Self {
        Self {
            muted: contribution == 0.0,
            ..Self::text(value)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct InventoryRow {
    holder: crate::navigation::Fragment,
    good: learning_lord_simulation::marketplace::Good,
    available: u64,
    tax_reserved: u64,
    work_reserved: u64,
    town_owned: u64,
}

fn inventory_rows(universe: &Universe, citizen: &Citizen) -> Vec<InventoryRow> {
    use crate::navigation::{Fragment as F, Target};
    use learning_lord_simulation::{marketplace::Good, storage::GoodsOwner};
    let members = sorted_agents(universe)
        .into_iter()
        .filter_map(|(_, agent)| {
            let AgentKind::Citizen(member) = &agent.kind;
            (member.economic_id() == citizen.economic_id()).then_some((agent, member))
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for (agent, member) in &members {
        for good in Good::ALL {
            let units = member.units(good);
            let town_owned = member.town_carried_units(good);
            if units == 0 && town_owned == 0 {
                continue;
            }
            let tax_reserved = member.tax_reserved_units(good);
            rows.push(InventoryRow {
                holder: F::entity(
                    format!("Carried | {}", agent.name),
                    Target::Citizen(member.id()),
                ),
                good,
                available: units.saturating_sub(tax_reserved),
                tax_reserved,
                work_reserved: 0,
                town_owned,
            });
        }
    }
    let mut stocks = universe
        .storage()
        .stock()
        .iter()
        .filter(|(key, units)| {
            **units > 0
                && (key.owner == citizen.goods_owner()
                    || key.owner == GoodsOwner::Town
                        && members.iter().any(|(_, member)| {
                            member.owned_properties().any(|place| place.id == key.place)
                        }))
        })
        .collect::<Vec<_>>();
    stocks.sort_by_key(|(key, _)| (key.place.0, key.good as usize));
    for (key, units) in stocks {
        let town = key.owner == GoodsOwner::Town;
        let tax_reserved = if town {
            0
        } else {
            universe
                .storage()
                .reserved_units(key.place, key.owner, key.good)
        };
        let available = if town {
            0
        } else {
            citizen.stored_available_units(key.place, key.good)
        };
        let work_reserved = if town {
            0
        } else {
            units.saturating_sub(tax_reserved).saturating_sub(available)
        };
        if let Some(row) = rows.iter_mut().find(|row| {
            row.holder.target == Some(Target::Location(key.place)) && row.good == key.good
        }) {
            row.town_owned += if town { *units } else { 0 };
            row.available += available;
            row.tax_reserved += tax_reserved;
            row.work_reserved += work_reserved;
        } else {
            rows.push(InventoryRow {
                holder: crate::navigation::place(citizen, key.place),
                good: key.good,
                available,
                tax_reserved,
                work_reserved,
                town_owned: if town { *units } else { 0 },
            });
        }
    }
    rows
}

fn section_rows(
    universe: &Universe,
    selected: Option<AgentId>,
    section: Section,
    skills: bool,
) -> Vec<ReadoutRow> {
    use crate::navigation::{self, Fragment as F, Target};
    use learning_lord_simulation::{marketplace::Good, production::Skill};
    let Some(agent) = selected_agent(universe, selected) else {
        return vec![ReadoutRow::text("No citizens")];
    };
    let AgentKind::Citizen(citizen) = &agent.kind;
    match section {
        Section::Header => {
            let mut row = vec![F::text(format!(
                "{} | {}",
                agent.name,
                citizen
                    .starting_role()
                    .map_or("No profession", |role| role.name())
            ))];
            if let Some(id) = citizen.household_id() {
                row.extend([F::text(" | "), navigation::account(universe, id)]);
            }
            for id in citizen.guild_ids() {
                row.extend([F::text(" | "), navigation::account(universe, id)]);
            }
            vec![ReadoutRow::rich(row)]
        }
        Section::Needs => vec![ReadoutRow {
            tooltip:
                "Includes hunger, thirst, sleep, clothing, diets and reserves. Excludes wealth."
                    .into(),
            ..ReadoutRow::text(format!("Wellbeing {:.1} (i)", citizen.needs_wellbeing()))
        }],
        Section::Reserves => {
            let nutrition = Good::FOOD
                .into_iter()
                .map(|good| {
                    let carried = universe
                        .agents()
                        .values()
                        .filter_map(|agent| {
                            let AgentKind::Citizen(member) = &agent.kind;
                            (member.economic_id() == citizen.economic_id()).then_some(
                                member
                                    .units(good)
                                    .saturating_sub(member.tax_reserved_units(good)),
                            )
                        })
                        .fold(0_u64, u64::saturating_add);
                    citizen
                        .stored_available_units(citizen.home(), good)
                        .saturating_add(carried) as f64
                        * good.nutrition_per_unit().unwrap()
                })
                .sum::<f64>();
            vec![ReadoutRow::contribution(format!("Food reserves {:.1} nutrition | {:+.2} wellbeing (i)", nutrition, citizen.food_reserve_wellbeing()), citizen.food_reserve_wellbeing()),
                ReadoutRow::contribution(format!("Drinking reserves {:.0} g | {:+.2} wellbeing (i)", citizen.drinking_water_reserve(), citizen.water_reserve_wellbeing()), citizen.water_reserve_wellbeing())]
                .into_iter().map(|mut row| { row.tooltip = "Reserve bonuses use usable home supplies and unconsumed active meals/drinks, divided between household members. Food nutrition also includes food carried by household members.".into(); row }).collect()
        }
        Section::Belongings => {
            let values = overview_values(universe, agent);
            let mut rows = vec![ReadoutRow::text(format!(
                "{}coins {} | {}wealth {}",
                if citizen.household_id().is_some() {
                    "Shared "
                } else {
                    ""
                },
                values[2].trim_end_matches(" | H"),
                if citizen.household_id().is_some() {
                    "shared "
                } else {
                    ""
                },
                values[3].trim_end_matches(" | H")
            ))];
            rows.push(ReadoutRow {
                table: true,
                muted: true,
                ..ReadoutRow::rich(
                    [
                        "Location / carrier",
                        "Good",
                        "Available",
                        "Tax reserved",
                        "Town-owned",
                    ]
                    .into_iter()
                    .map(F::text)
                    .collect(),
                )
            });
            let inventory = inventory_rows(universe, citizen);
            if inventory.is_empty() {
                rows.push(ReadoutRow::text("No goods"));
            }
            for item in inventory {
                rows.push(ReadoutRow { table: true,
                    tooltip: format!("{} committed to production; excluded from Available. Available also excludes tax reservations. Town-owned goods are held separately.", crate::quantity_label(item.good, item.work_reserved)),
                    ..ReadoutRow::rich(vec![item.holder, F::good(item.good), F::text(format!("{}{}", crate::quantity_label(item.good, item.available), if item.work_reserved > 0 { " (i)" } else { "" })),
                        F::text(crate::quantity_label(item.good, item.tax_reserved)), F::text(crate::quantity_label(item.good, item.town_owned))]) });
            }
            let mut owned = vec![F::text("Owns: ")];
            let mut properties = citizen.owned_properties().collect::<Vec<_>>();
            properties.sort_by(|a, b| a.name.cmp(&b.name));
            if properties.is_empty() {
                owned.push(F::text("None"));
            }
            for (index, place) in properties.iter().enumerate() {
                if index > 0 {
                    owned.push(F::text(", "));
                }
                owned.push(F::entity(&place.name, Target::Location(place.id)));
            }
            rows.push(ReadoutRow::rich(owned));
            rows.push(ReadoutRow::rich(vec![
                F::text("Designated workplace: "),
                citizen
                    .designated_workplace()
                    .map_or_else(|| F::text("None"), |id| navigation::place(citizen, id)),
            ]));
            rows.push(ReadoutRow::text(format!(
                "Workshop fees owed: {:.3} coins",
                citizen.workshop_fee_liability().max(0.0)
            )));
            rows
        }
        Section::Planning => {
            let mut rows = vec![ReadoutRow::rich(location_fragments(citizen))];
            if let Some(active) = citizen.active_action() {
                let mut row = navigation::active_action(&active, citizen);
                row.push(F::text(format!(
                    " | {} remaining",
                    duration(active.remaining_ms())
                )));
                rows.push(ReadoutRow::rich(row));
            } else {
                rows.push(ReadoutRow::text("Idle"));
            }
            rows.push(ReadoutRow::text(format!(
                "Decision score: {}",
                citizen
                    .decision_score()
                    .map_or_else(|error| error.to_string(), |value| format!("{value:.2}"))
            )));
            if let Some(goal) = citizen
                .active_plan()
                .and_then(|active| active.plan().goals().first())
            {
                rows.push(ReadoutRow::text(format!(
                    "Replan after {} completes",
                    goal_label(goal.goal)
                )));
            }
            let maintenance = citizen.household_id().map_or(citizen.maintenance(), |id| {
                &universe.households()[&id].maintenance
            });
            for (name, task) in [
                ("Replenishment", &maintenance.replenishment),
                ("Listing excess", &maintenance.listing),
            ] {
                rows.push(ReadoutRow { tooltip: "Maintenance has a six-hour cooldown. Claimant is the worker committed to that task.".into(),
                    ..ReadoutRow::rich(vec![F::text(format!("{name}: {} cooldown | Claimant ", duration(task.remaining_ms(universe.current_time_ms())))), task.claimant.map_or_else(|| F::text("None"), |id| navigation::account(universe, id)), F::text(" (i)")]) });
            }
            rows
        }
        Section::Skills => {
            if skills {
                Skill::ALL
                    .into_iter()
                    .filter_map(|skill| {
                        citizen.skill_practice_ms(skill).map(|practice| {
                            ReadoutRow::text(format!(
                                "{}: {:.1} h practiced | {:.2}% less production time",
                                skill.name(),
                                practice as f64 / 3_600_000.0,
                                learning_lord_simulation::production::skill_duration_reduction(
                                    practice
                                ) * 100.0
                            ))
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            }
        }
        Section::Decision => {
            let mut rows: Vec<ReadoutRow> = Vec::new();
            for fragments in crate::decision_rows(universe, selected) {
                let plain = navigation::plain(&fragments);
                if plain.starts_with("Inputs and targets below") || plain.starts_with("\nChosen by")
                {
                    if let Some(last) = rows.last_mut() {
                        last.tooltip = plain;
                        last.fragments.push(F::text(" (i)"));
                    }
                } else if plain.starts_with("Complete-batch method estimates at ") {
                    rows.push(ReadoutRow {
                        tooltip: plain,
                        ..ReadoutRow::rich(fragments.into_iter().take(2).collect())
                    });
                } else {
                    rows.push(ReadoutRow::rich(fragments));
                }
            }
            rows
        }
    }
}

#[derive(Component)]
pub(crate) struct LinkedReadout {
    section: Section,
    rows: Vec<ReadoutRow>,
}
impl LinkedReadout {
    fn bundle(section: Section) -> impl Bundle {
        (
            Self {
                section,
                rows: Vec::new(),
            },
            Node {
                min_width: px(0),
                width: if section == Section::Header {
                    Val::Auto
                } else {
                    percent(100)
                },
                flex_grow: if section == Section::Header { 1.0 } else { 0.0 },
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                flex_shrink: if section == Section::Header { 1.0 } else { 0.0 },
                ..default()
            },
        )
    }
}
pub(crate) fn refresh_linked(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    skills: Option<Res<SkillsExpanded>>,
    panels: Query<(Entity, Ref<LinkedReadout>)>,
    mut skills_labels: Query<&mut Text, With<SkillsLabel>>,
) {
    let expanded = skills.as_ref().is_some_and(|skills| skills.0);
    for mut label in &mut skills_labels {
        let next = if expanded { "Skills [-]" } else { "Skills [+]" };
        if label.0 != next {
            label.0 = next.into();
        }
    }
    if panels.is_empty()
        || !snapshot.is_changed()
            && !selection.is_changed()
            && !skills.as_ref().is_some_and(|skills| skills.is_changed())
            && panels.iter().all(|(_, panel)| !panel.is_added())
    {
        return;
    }
    for (entity, panel) in &panels {
        let rows = section_rows(
            &snapshot.0.universe,
            selection.0,
            panel.section,
            skills.as_ref().is_some_and(|skills| skills.0),
        );
        if rows == panel.rows {
            continue;
        }
        let section = panel.section;
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|parent| {
                for row in &rows {
                    let color = if row.muted { MUTED } else { TEXT };
                    parent
                        .spawn((
                            Node {
                                width: if row.table { px(720) } else { percent(100) },
                                min_height: px(if row.table { 26 } else { 0 }),
                                min_width: if row.table { percent(100) } else { px(0) },
                                flex_direction: if row.table {
                                    FlexDirection::Row
                                } else {
                                    FlexDirection::Column
                                },
                                flex_shrink: 0.0,
                                ..default()
                            },
                            Interaction::None,
                            crate::tooltip::Tooltip(row.tooltip.clone()),
                            bevy::ui::FocusPolicy::Pass,
                        ))
                        .with_children(|line| {
                            if row.table {
                                for (fragment, width) in
                                    row.fragments.iter().zip([27.0, 22.0, 17.0, 17.0, 17.0])
                                {
                                    line.spawn(Node {
                                        width: percent(width),
                                        min_width: px(0),
                                        padding: UiRect::right(px(5)),
                                        flex_shrink: 0.0,
                                        ..default()
                                    })
                                    .with_children(|cell| {
                                        crate::navigation::render(
                                            cell,
                                            std::slice::from_ref(fragment),
                                            14.0,
                                            color,
                                        )
                                    });
                                }
                            } else {
                                crate::navigation::render(line, &row.fragments, 14.0, color);
                            }
                        });
                }
            })
            .insert(LinkedReadout { section, rows });
    }
}
pub(crate) fn location_fragments(citizen: &Citizen) -> Vec<crate::navigation::Fragment> {
    use crate::navigation::{self, Fragment as F};
    if let Some(active) = citizen.active_action()
        && let learning_lord_simulation::CitizenAction::Travel(id) = active.action()
    {
        return vec![
            F::text("Location: Travelling to "),
            navigation::place(citizen, id),
        ];
    }
    let map = citizen.map();
    let place = map
        .places()
        .values()
        .filter(|place| place.position == citizen.position())
        .min_by_key(|place| (place.id != citizen.home(), place.name.clone()));
    vec![
        F::text("Location: "),
        place.map_or_else(
            || {
                F::text(format!(
                    "({:.0}, {:.0}) m",
                    citizen.position().x,
                    citizen.position().y
                ))
            },
            |place| navigation::place(citizen, place.id),
        ),
    ]
}

#[cfg(test)]
pub(crate) fn readout(universe: &Universe, selected: Option<AgentId>) -> String {
    let mut rows = [
        Section::Header,
        Section::Needs,
        Section::Reserves,
        Section::Belongings,
        Section::Planning,
        Section::Skills,
        Section::Decision,
    ]
    .into_iter()
    .flat_map(|section| section_rows(universe, selected, section, true))
    .map(|row| {
        if row.table {
            row.fragments
                .iter()
                .map(|f| f.label.clone())
                .collect::<Vec<_>>()
                .join(" | ")
        } else {
            crate::navigation::plain(&row.fragments)
        }
    })
    .collect::<Vec<_>>();
    if let Some(agent) = selected_agent(universe, selected) {
        let AgentKind::Citizen(citizen) = &agent.kind;
        for need in [Need::Hunger, Need::Thirst, Need::Sleep, Need::Clothing] {
            rows.push(format!("{}: {:.1}", need.name(), need.value(citizen)));
        }
        rows.push(format!(
            "Diet wellbeing: +{:.2}",
            citizen.dietary_wellbeing()
        ));
        if let Some(goal) = citizen
            .active_plan()
            .and_then(|active| active.plan().goals().first())
        {
            rows.push(format!("Replan after {} completes", goal_label(goal.goal)));
        }
    }
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{self, Snapshot};
    use learning_lord_simulation::{CitizenAction, locations::Map};

    #[test]
    fn overview_uses_shared_accounts_and_needs_wellbeing_without_wealth() {
        let universe = simulation::new_universe().unwrap();
        let (id, agent) = sorted_agents(&universe)[0];
        let AgentKind::Citizen(citizen) = &agent.kind;
        let household = citizen.household_id().unwrap();
        let values = overview_values(&universe, agent);
        assert_eq!(
            values[2],
            format!("{} | H", universe.households()[&household].coins)
        );
        assert_eq!(
            values[3],
            format!("{:.3} | H", universe.household_wealth(household).unwrap())
        );
        assert_eq!(values[4], format!("{:.1}", citizen.needs_wellbeing()));
        assert_ne!(citizen.needs_wellbeing(), citizen.decision_score().unwrap());
        let header = section_rows(&universe, Some(id), Section::Header, false);
        assert!(header[0].fragments.iter().any(
            |fragment| fragment.target == Some(crate::navigation::Target::Household(household))
        ));
    }

    #[test]
    fn zero_contribution_dimming_handles_satiation_and_overfull_penalties() {
        for (hunger, contribution) in [
            (-150.0, -50.0),
            (-100.0, 0.0),
            (-50.0, 0.0),
            (0.0, 0.0),
            (25.0, -25.0),
            (125.0, -225.0),
        ] {
            let citizen = Citizen::new(hunger).unwrap();
            assert_eq!(Need::Hunger.contribution(&citizen), contribution);
        }
        let citizen = Citizen::new(0.0).unwrap().with_thirst(-150.0).unwrap();
        assert_eq!(Need::Thirst.contribution(&citizen), 0.0);
        assert!(Need::Clothing.contribution(&citizen) < 0.0);
        let (universe, id) = Universe::with_map(Map::default())
            .with_citizen("Ada", citizen)
            .unwrap();
        let reserves = section_rows(&universe, Some(id), Section::Reserves, false);
        assert!(reserves.iter().all(|row| row.muted));
    }

    #[test]
    fn household_belongings_include_all_carriers_shared_storage_and_work_commitments_once() {
        use learning_lord_simulation::{
            CitizenAction, locations::Location, marketplace::Good, production::Recipe,
            storage::GoodsOwner,
        };
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen(
                "Ada",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(learning_lord_simulation::StartingRole::Baker)
                    .with_good(Good::Berries, 12)
                    .unwrap(),
            )
            .unwrap();
        let (world, second) = world
            .with_citizen(
                "Ben",
                Citizen::new(0.0)
                    .unwrap()
                    .with_good(Good::Water, 34)
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(worker) = &world.agents()[&first].kind;
        let (world, household) = world
            .with_household("Family", worker.home(), &[first, second])
            .unwrap();
        let (mut world, bakery) = world.with_property(first, Location::Bakery).unwrap();
        for &(good, units) in Recipe::BakeRolls.inputs() {
            world = world
                .with_stored_good(bakery, GoodsOwner::Household(household), good, units + 20)
                .unwrap();
        }
        world = world
            .with_stored_good(bakery, GoodsOwner::Town, Good::BreadRoll, 3)
            .unwrap();
        world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    recipe: Recipe::BakeRolls,
                    place: bakery,
                },
            )
            .unwrap();
        let AgentKind::Citizen(worker) = &world.agents()[&first].kind;
        let rows = inventory_rows(&world, worker);
        assert!(rows.iter().any(|row| row.holder.target
            == Some(crate::navigation::Target::Citizen(first))
            && row.good == Good::Berries
            && row.available == 12));
        assert!(rows.iter().any(|row| row.holder.target
            == Some(crate::navigation::Target::Citizen(second))
            && row.good == Good::Water
            && row.available == 34));
        for &(good, units) in Recipe::BakeRolls.inputs() {
            let row = rows
                .iter()
                .find(|row| {
                    row.holder.target == Some(crate::navigation::Target::Location(bakery))
                        && row.good == good
                })
                .unwrap();
            assert_eq!(
                (row.available, row.tax_reserved, row.work_reserved),
                (20, 0, units)
            );
        }
        assert_eq!(
            rows.iter()
                .filter(|row| row.good == Good::BreadRoll && row.town_owned == 3)
                .count(),
            1
        );
        assert!(rows.iter().all(|row| row.available
            + row.tax_reserved
            + row.work_reserved
            + row.town_owned
            > 0));
        let AgentKind::Citizen(member) = &world.agents()[&second].kind;
        assert_eq!(inventory_rows(&world, member), rows);
    }

    #[test]
    fn table_is_default_row_opens_details_and_return_preserves_selection_and_tabs() {
        let universe = simulation::new_universe().unwrap();
        let ids = sorted_agents(&universe)
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
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
        .init_resource::<Selection>()
        .init_resource::<RosterState>()
        .init_resource::<SkillsExpanded>()
        .init_resource::<crate::citizen_history::Selection>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<bevy::input_focus::InputFocus>()
        .add_systems(Startup, move |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                spawn_roster(parent, ids.len());
                spawn_details(parent);
            });
        })
        .add_systems(
            Update,
            (
                handle_selection,
                crate::citizen_history::handle_selection,
                refresh_cards,
                refresh_screens,
                crate::citizen_history::refresh,
            )
                .chain(),
        );
        app.update();
        assert!(!app.world().resource::<RosterState>().detail);
        let tabs = app
            .world_mut()
            .query::<&crate::citizen_history::ViewPanel>()
            .iter(app.world())
            .count();
        assert_eq!(tabs, 4);
        let row = app
            .world_mut()
            .query::<(Entity, &Card)>()
            .iter(app.world())
            .find(|(_, card)| card.0 == 1)
            .unwrap()
            .0;
        app.world_mut().entity_mut(row).insert(Interaction::Pressed);
        app.update();
        let selected = app.world().resource::<Selection>().0;
        assert!(app.world().resource::<RosterState>().detail);
        let tab = app
            .world_mut()
            .query::<(Entity, &crate::citizen_history::Choice)>()
            .iter(app.world())
            .find(|(_, choice)| {
                matches!(
                    choice,
                    crate::citizen_history::Choice::View(crate::citizen_history::View::Belongings)
                )
            })
            .unwrap()
            .0;
        app.world_mut().entity_mut(row).insert(Interaction::None);
        app.world_mut().entity_mut(tab).insert(Interaction::Pressed);
        app.update();
        assert!(
            app.world_mut()
                .query::<(&crate::citizen_history::ViewPanel, &Node)>()
                .iter(app.world())
                .all(|(panel, node)| (node.display == Display::Flex)
                    == (panel.0 == crate::citizen_history::View::Belongings))
        );
        let back = app
            .world_mut()
            .query_filtered::<Entity, With<Back>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(back)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app.world().resource::<RosterState>().detail);
        assert_eq!(app.world().resource::<Selection>().0, selected);
        let populated = app.world().resource::<DisplaySnapshot>().0.universe.clone();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe =
            Universe::with_map(Map::default());
        app.update();
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "No citizens")
        );
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = populated;
        app.update();
        let cells = app
            .world_mut()
            .query_filtered::<&Text, With<Summary>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 50);
        assert!(cells.iter().all(|text| !text.0.is_empty()));
        assert!(
            !app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "No citizens")
        );
    }

    #[test]
    fn selected_diet_bars_follow_consumption_and_selection() {
        use learning_lord_simulation::diet::DietCategory;
        let eater = Citizen::new(50.0)
            .unwrap()
            .with_good(learning_lord_simulation::marketplace::Good::Fish, 2)
            .unwrap();
        let (universe, eater_id) = Universe::with_map(Map::default())
            .with_citizen("Eater", eater)
            .unwrap();
        let (universe, empty_id) = universe
            .with_citizen("Empty", Citizen::new(0.0).unwrap())
            .unwrap();
        let AgentKind::Citizen(eater) = &universe.agents()[&eater_id].kind;
        let universe = universe
            .deposit_goods(
                eater_id,
                eater.home(),
                learning_lord_simulation::marketplace::Good::Fish,
                2,
            )
            .unwrap();
        let universe = universe
            .start_action(eater_id, CitizenAction::Eat)
            .unwrap()
            .advance(500_000)
            .unwrap();
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
        .insert_resource(Selection(Some(eater_id)))
        .add_systems(Update, refresh_diet);
        let label = app
            .world_mut()
            .spawn((DietLabel(DietCategory::Meat), Text::new("")))
            .id();
        let fill = app
            .world_mut()
            .spawn((DietFill(DietCategory::Meat), Node::default()))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(label).unwrap().0,
            "Meat: 49.7/100 | +4.97 wellbeing"
        );
        let width = app.world().get::<Node>(fill).unwrap().width;
        assert!(matches!(width, Val::Percent(value) if (value - 49.71065).abs() < 0.001));
        app.world_mut().resource_mut::<Selection>().0 = Some(empty_id);
        app.update();
        assert_eq!(app.world().get::<Node>(fill).unwrap().width, percent(0));
        assert_eq!(
            app.world().get::<Text>(label).unwrap().0,
            "Meat: 0.0/100 | +0.00 wellbeing"
        );
    }

    #[test]
    fn wheel_scrolls_only_the_frontmost_window_and_shift_scrolls_horizontally() {
        use crate::floating_ui::{FloatingWindow, InputCapture, WindowKind};
        let mut app = App::new();
        app.insert_resource(InputCapture {
            blocked: true,
            hovered: Some(WindowKind::Market),
        })
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<MouseWheel>()
        .add_systems(Update, scroll_panels);
        let mut panels = Vec::new();
        for kind in [WindowKind::Citizens, WindowKind::Market] {
            let root = app.world_mut().spawn(FloatingWindow(kind)).id();
            let panel = app
                .world_mut()
                .spawn((
                    Node {
                        overflow: Overflow::scroll(),
                        ..default()
                    },
                    ComputedNode {
                        size: Vec2::splat(100.0),
                        content_size: Vec2::splat(400.0),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition {
                        cursor_over: true,
                        ..default()
                    },
                    InheritedVisibility::VISIBLE,
                ))
                .id();
            app.world_mut().entity_mut(root).add_child(panel);
            panels.push(panel);
        }
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(panels[0]).unwrap().0,
            Vec2::ZERO
        );
        assert_eq!(
            app.world().get::<ScrollPosition>(panels[1]).unwrap().0,
            Vec2::new(0.0, 30.0)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(panels[0]).unwrap().0,
            Vec2::ZERO
        );
        assert_eq!(
            app.world().get::<ScrollPosition>(panels[1]).unwrap().0,
            Vec2::new(30.0, 30.0)
        );
        app.world_mut().resource_mut::<InputCapture>().hovered = None;
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(panels[1]).unwrap().0,
            Vec2::new(30.0, 30.0)
        );
    }

    #[test]
    fn clothing_bar_covers_its_full_unsigned_range() {
        for (need, width) in [
            (0.0, 0.0),
            (10.0, 50.0),
            (20.0, 100.0),
            (-1.0, 0.0),
            (30.0, 100.0),
            (f64::NAN, 0.0),
        ] {
            assert_eq!(clothing_geometry(need), (0.0, width));
        }
    }

    #[test]
    fn thirst_labels_show_hydration_and_urgency() {
        for (value, label) in [
            (-100.0, "Hydrated"),
            (-50.0, "Hydrated"),
            (0.0, "Balanced"),
            (25.0, "Thirsty"),
            (100.0, "Very thirsty"),
            (101.0, "URGENT >100"),
        ] {
            assert_eq!(thirst_label(value), label);
        }
        assert_eq!(goal_label(Effect::ReduceThirst), "Reduce Thirst");
    }

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
    fn every_citizen_is_reachable_and_restart_resets_selection() {
        let universe = simulation::new_universe().unwrap();
        let ids: Vec<_> = sorted_agents(&universe).iter().map(|(id, _)| *id).collect();
        assert_eq!(ids.len(), 10);
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
        .init_resource::<Selection>()
        .init_resource::<RosterState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, handle_selection);
        let card = app.world_mut().spawn((Card(0), Interaction::None)).id();
        for (slot, id) in ids.iter().enumerate() {
            app.world_mut()
                .entity_mut(card)
                .insert((Card(slot), Interaction::Pressed));
            app.update();
            assert_eq!(app.world().resource::<Selection>().0, Some(*id));
            app.world_mut().entity_mut(card).insert(Interaction::None);
            app.update();
        }
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert_eq!(app.world().resource::<Selection>().0, Some(ids[0]));
    }

    #[test]
    fn selection_survives_snapshots_and_repairs_after_restart() {
        let universe = simulation::new_universe().unwrap();
        let sorted = sorted_agents(&universe);
        assert_eq!(
            sorted
                .iter()
                .map(|(_, agent)| agent.name.as_str())
                .take(6)
                .collect::<Vec<_>>(),
            ["Ada", "Bram", "Cleo", "Dara", "Eira", "Finn"]
        );
        let dara = sorted[3].0;
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
        .init_resource::<Selection>()
        .init_resource::<RosterState>()
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
        assert!(app.world().resource::<RosterState>().detail);
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
    fn details_distinguish_current_goal_from_forecast() {
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
        let AgentKind::Citizen(eater) = &universe.agents()[&ada].kind;
        let universe = universe
            .deposit_goods(
                ada,
                eater.home(),
                learning_lord_simulation::marketplace::Good::Berries,
                310,
            )
            .unwrap();
        let started = universe.start_planning(ada).unwrap();
        let progressed = started.advance(50_000).unwrap();
        let AgentKind::Citizen(citizen) = &progressed.agents()[&ada].kind;
        assert_eq!(
            citizen.active_action().unwrap().action(),
            CitizenAction::Eat
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
            0
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
                .any(|row| row.label.contains("Forecast after replanning"))
        );
        assert!(
            rows.iter()
                .any(|row| row.label.contains("Reduce Sleep Need"))
        );
        let details = crate::citizen_readout(&progressed, Some(bram));
        assert!(details.starts_with("Bram"));
        assert!(details.contains("Hunger: -9.9"));
        assert!(details.contains("Location: Bram's home"));
        assert!(!details.contains("Berries: 0 g"));
    }
}
