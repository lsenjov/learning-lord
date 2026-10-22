use super::*;
use learning_lord_simulation::{CitizenAction, SimulationError};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Grouping {
    None,
    #[default]
    Profession,
    Household,
    Workplace,
    WealthFifth,
    Activity,
}
impl Grouping {
    const ALL: [Self; 6] = [
        Self::None,
        Self::Profession,
        Self::Household,
        Self::Workplace,
        Self::WealthFifth,
        Self::Activity,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Profession => "Profession",
            Self::Household => "Household",
            Self::Workplace => "Workplace",
            Self::WealthFifth => "Wealth fifth",
            Self::Activity => "Activity",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Sort {
    #[default]
    Name,
    Wellbeing,
    Wealth,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Citizen(AgentId),
    Group(String),
}
#[derive(Component)]
pub(super) struct Control(pub(super) Choice);
#[derive(Clone, Copy)]
pub(super) enum Choice {
    Group,
    SelectGroup(Grouping),
    Sort(Sort),
    OpenAll,
    CollapseAll,
}
#[derive(Component)]
pub(crate) struct ControlLabel;
#[derive(Component)]
pub(super) struct GroupMenu;
#[derive(Component)]
pub(super) struct ExpandControl;
#[derive(Component)]
pub(crate) struct Heading(pub(super) usize);

#[derive(Default)]
pub(super) struct View {
    pub grouping: Grouping,
    pub sort: Sort,
    pub descending: bool,
    pub open: BTreeSet<String>,
    pub menu: bool,
}
impl View {
    pub fn sort_by(&mut self, sort: Sort) {
        self.descending = if self.sort == sort {
            !self.descending
        } else {
            sort == Sort::Wealth
        };
        self.sort = sort;
    }
}
#[derive(Clone, Debug)]
pub(super) struct Member {
    pub id: AgentId,
    pub name: String,
    pub profession: String,
    pub coins: f64,
    pub wealth: Result<f64, SimulationError>,
    pub wellbeing: f64,
    pub account: AgentId,
    pub household: Option<AgentId>,
    pub workplace: Option<learning_lord_simulation::locations::PlaceId>,
    pub activity: &'static str,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub target: Target,
    pub cells: [String; 5],
    pub child: bool,
}
#[derive(Debug)]
struct Group {
    key: String,
    name: String,
    members: Vec<Member>,
}

pub(super) fn activity(action: Option<CitizenAction>) -> &'static str {
    use CitizenAction::*;
    match action {
        Some(
            Produce(_)
            | ProduceAt { .. }
            | Plant { .. }
            | Tend { .. }
            | Harvest { .. }
            | Clear { .. },
        ) => "Producing",
        Some(Travel(_)) => "Travelling",
        Some(BuyFood(_) | Buy(_) | BuyAt { .. } | ListExcess | List(..) | ListAt { .. }) => {
            "Trading"
        }
        Some(
            Withdraw(..)
            | WithdrawAt { .. }
            | CollectStored { .. }
            | DepositStored { .. }
            | CollectBeer { .. },
        ) => "Moving supplies",
        Some(Eat | Drink | Sleep | EquipClothing) => "Personal needs",
        Some(Wait) | None => "Waiting / idle",
    }
}
pub(super) fn members(universe: &Universe) -> Vec<Member> {
    sorted_agents(universe)
        .into_iter()
        .map(|(id, agent)| {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let household = citizen.household_id();
            let (coins, wealth) = if let Some(household) = household {
                let account = &universe.households()[&household];
                let count = account.members.len() as f64;
                (
                    account.coins as f64 / count,
                    universe.household_wealth(household).and_then(|shared| {
                        citizen.wealth().map(|personal| shared / count + personal)
                    }),
                )
            } else {
                (citizen.coins() as f64, citizen.wealth())
            };
            Member {
                id,
                name: agent.name.clone(),
                profession: citizen
                    .starting_role()
                    .map_or("Unspecified", |role| role.name())
                    .into(),
                coins,
                wealth,
                wellbeing: citizen.needs_wellbeing(),
                account: citizen.economic_id(),
                household,
                workplace: citizen.designated_workplace(),
                activity: activity(citizen.active_action().map(|action| action.action())),
            }
        })
        .collect()
}
fn mean_wealth(members: &[Member]) -> Result<f64, SimulationError> {
    members
        .iter()
        .try_fold(0.0, |total, member| {
            member.wealth.map(|value| total + value)
        })
        .map(|total| total / members.len() as f64)
}
fn median(members: &[Member]) -> f64 {
    let mut values: Vec<_> = members.iter().map(|member| member.wellbeing).collect();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}
fn compare_wealth(a: &Result<f64, SimulationError>, b: &Result<f64, SimulationError>) -> Ordering {
    match (a, b) {
        (Ok(a), Ok(b)) => a.total_cmp(b),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}
fn compare(
    view: &View,
    name_a: &str,
    name_b: &str,
    wellbeing_a: f64,
    wellbeing_b: f64,
    wealth_a: &Result<f64, SimulationError>,
    wealth_b: &Result<f64, SimulationError>,
) -> Ordering {
    let order = match view.sort {
        Sort::Name => name_a.cmp(name_b),
        Sort::Wellbeing => wellbeing_a.total_cmp(&wellbeing_b),
        Sort::Wealth => compare_wealth(wealth_a, wealth_b),
    };
    (if view.descending {
        order.reverse()
    } else {
        order
    })
    .then_with(|| name_a.cmp(name_b))
}
fn sort_members(members: &mut [Member], view: &View) {
    members.sort_by(|a, b| {
        compare(
            view,
            &a.name,
            &b.name,
            a.wellbeing,
            b.wellbeing,
            &a.wealth,
            &b.wealth,
        )
        .then_with(|| a.id.0.cmp(&b.id.0))
    });
}
fn wealth_label(value: &Result<f64, SimulationError>) -> String {
    value
        .as_ref()
        .map_or_else(|error| error.to_string(), |value| format!("{value:.3}"))
}
fn member_row(member: &Member, child: bool) -> Row {
    Row {
        target: Target::Citizen(member.id),
        cells: [
            format!("{}{}", if child { "    " } else { "" }, member.name),
            member.profession.clone(),
            format!("{:.1}", member.coins),
            wealth_label(&member.wealth),
            format!("{:.1}", member.wellbeing),
        ],
        child,
    }
}

fn fifths(members: &[Member]) -> BTreeMap<String, usize> {
    let mut accounts: BTreeMap<String, Vec<Member>> = BTreeMap::new();
    for member in members {
        accounts
            .entry(member.account.0.to_string())
            .or_default()
            .push(member.clone());
    }
    let mut accounts: Vec<_> = accounts.into_iter().collect();
    accounts.sort_by(|(a_id, a), (b_id, b)| {
        compare_wealth(&mean_wealth(a), &mean_wealth(b))
            .then_with(|| a[0].name.cmp(&b[0].name))
            .then_with(|| a_id.cmp(b_id))
    });
    let mut cumulative = vec![0usize];
    for (_, account) in &accounts {
        cumulative.push(cumulative.last().unwrap() + account.len());
    }
    // Boundaries may coincide when a household spans more than one fifth.
    let boundaries: Vec<_> = (1..5)
        .map(|fifth| {
            (0..cumulative.len())
                .min_by_key(|index| (cumulative[*index] * 5).abs_diff(members.len() * fifth))
                .unwrap()
        })
        .collect();
    accounts
        .into_iter()
        .enumerate()
        .map(|(index, (id, _))| {
            (
                id,
                boundaries
                    .iter()
                    .filter(|boundary| index >= **boundary)
                    .count(),
            )
        })
        .collect()
}
fn groups(universe: &Universe, members: Vec<Member>, grouping: Grouping) -> Vec<Group> {
    let bands = if grouping == Grouping::WealthFifth {
        fifths(&members)
    } else {
        BTreeMap::new()
    };
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for member in members {
        let (key, name) = match grouping {
            Grouping::Profession => (member.profession.clone(), member.profession.clone()),
            Grouping::Household => (
                format!("account:{}", member.account.0),
                member.household.map_or_else(
                    || member.name.clone(),
                    |id| universe.households()[&id].name.clone(),
                ),
            ),
            Grouping::Workplace => member.workplace.map_or_else(
                || ("work:none".into(), "No designated workplace".into()),
                |id| {
                    (
                        format!("work:{}", id.0),
                        universe.map().place(id).map_or_else(
                            |_| "Unknown workplace".into(),
                            |place| place.name.clone(),
                        ),
                    )
                },
            ),
            Grouping::WealthFifth => {
                let band = bands[&member.account.0.to_string()];
                (
                    format!("fifth:{band}"),
                    [
                        "1: Lowest fifth",
                        "2: Lower fifth",
                        "3: Middle fifth",
                        "4: Upper fifth",
                        "5: Highest fifth",
                    ][band]
                        .into(),
                )
            }
            Grouping::Activity => (member.activity.into(), member.activity.into()),
            Grouping::None => unreachable!(),
        };
        groups
            .entry(key.clone())
            .or_insert_with(|| Group {
                key,
                name,
                members: vec![],
            })
            .members
            .push(member);
    }
    groups.into_values().collect()
}
pub(super) fn rows(universe: &Universe, view: &View) -> Vec<Row> {
    let mut members = members(universe);
    if view.grouping == Grouping::None {
        sort_members(&mut members, view);
        return members
            .iter()
            .map(|member| member_row(member, false))
            .collect();
    }
    let mut groups = groups(universe, members, view.grouping);
    groups.sort_by(|a, b| {
        compare(
            view,
            &a.name,
            &b.name,
            median(&a.members),
            median(&b.members),
            &mean_wealth(&a.members),
            &mean_wealth(&b.members),
        )
        .then_with(|| a.key.cmp(&b.key))
    });
    let mut rows = vec![];
    for mut group in groups {
        if view.grouping == Grouping::Household && group.members[0].household.is_none() {
            rows.push(member_row(&group.members[0], false));
            continue;
        }
        let open = view.open.contains(&group.key);
        rows.push(Row {
            target: Target::Group(group.key.clone()),
            cells: [
                format!("{} {}", if open { "[-]" } else { "[+]" }, group.name),
                format!("{} citizens", group.members.len()),
                format!(
                    "{:.1}",
                    group.members.iter().map(|member| member.coins).sum::<f64>()
                        / group.members.len() as f64
                ),
                wealth_label(&mean_wealth(&group.members)),
                format!("{:.1}", median(&group.members)),
            ],
            child: false,
        });
        if open {
            sort_members(&mut group.members, view);
            rows.extend(group.members.iter().map(|member| member_row(member, true)));
        }
    }
    rows
}
pub(super) fn open_all(universe: &Universe, view: &mut View) {
    view.open = groups(universe, members(universe), view.grouping)
        .into_iter()
        .map(|group| group.key)
        .collect();
}

fn button(parent: &mut ChildSpawnerCommands, choice: Choice, label: &str) {
    parent
        .spawn((
            Button,
            Control(choice),
            Node {
                padding: UiRect::axes(px(6), px(3)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|button| {
            button.spawn((text(label, 14.0, TEXT), ControlLabel));
        });
}
pub(super) fn controls(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(5),
            flex_wrap: FlexWrap::Wrap,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            row.spawn(text("Group by", 14.0, MUTED));
            button(row, Choice::Group, "Profession v");
            row.spawn((
                ExpandControl,
                Node {
                    column_gap: px(5),
                    ..default()
                },
            ))
            .with_children(|row| {
                button(row, Choice::CollapseAll, "Collapse all");
                button(row, Choice::OpenAll, "Open all");
            });
        });
    parent
        .spawn((
            GroupMenu,
            Node {
                display: Display::None,
                column_gap: px(4),
                flex_wrap: FlexWrap::Wrap,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|row| {
            for group in Grouping::ALL {
                button(row, Choice::SelectGroup(group), group.name());
            }
        });
}
pub(super) fn headings(parent: &mut ChildSpawnerCommands) {
    for (index, (label, width)) in COLUMNS.iter().enumerate() {
        let mut heading=parent.spawn((Node {width:percent(*width),padding:UiRect::horizontal(px(6)),flex_shrink:0.0,..default()},Interaction::None,crate::tooltip::Tooltip(match index {
            0=>"Sort groups and citizens by name.",
            1=>"Starting profession for citizens; citizen count for groups.",
            2=>"Coins per citizen. Shared household coins are divided evenly; groups show the mean.",
            3=>"Wealth per citizen. Shared household assets are divided evenly; personal assets remain with their owner. Groups show the mean.",
            _=>"Needs wellbeing, excluding wealth. Groups show the median.",
        }.into())));
        if let Some(sort) = match index {
            0 => Some(Sort::Name),
            3 => Some(Sort::Wealth),
            4 => Some(Sort::Wellbeing),
            _ => None,
        } {
            heading.insert((Button, Control(Choice::Sort(sort))));
        }
        heading.with_children(|cell| {
            cell.spawn((text(*label, 14.0, MUTED), Heading(index)));
        });
    }
}
pub(super) fn spawn_rows(parent: &mut ChildSpawnerCommands, rows: &[Row]) {
    if rows.is_empty() {
        parent.spawn(text("No citizens", 14.0, MUTED));
    }
    for (slot, value) in rows.iter().enumerate() {
        parent
            .spawn((
                Button,
                Card(value.target.clone()),
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
                            text(&value.cells[column], 14.0, TEXT),
                            Summary(slot, column),
                        ));
                    });
                }
            });
    }
}
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Controls<'w, 's> {
    pub(super) choices:
        Query<'w, 's, (&'static Interaction, &'static Control), Changed<Interaction>>,
}
pub(crate) type SummaryTexts<'w, 's> = Query<
    'w,
    's,
    (&'static Summary, &'static mut Text),
    (Without<NeedLabel>, Without<ControlLabel>, Without<Heading>),
>;
pub(crate) type NeedTexts<'w, 's> = Query<
    'w,
    's,
    (
        &'static NeedLabel,
        &'static mut Text,
        &'static mut TextColor,
    ),
    (Without<Summary>, Without<ControlLabel>, Without<Heading>),
>;
type ControlTexts<'w, 's> = Query<
    'w,
    's,
    (&'static ChildOf, &'static mut Text),
    (
        With<ControlLabel>,
        Without<Summary>,
        Without<NeedLabel>,
        Without<Heading>,
    ),
>;
type HeadingTexts<'w, 's> = Query<
    'w,
    's,
    (&'static Heading, &'static mut Text),
    (Without<ControlLabel>, Without<Summary>, Without<NeedLabel>),
>;
type MenuNodes<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<GroupMenu>, Without<ExpandControl>, Without<NeedFill>)>;
type ExpandNodes<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<ExpandControl>, Without<GroupMenu>, Without<NeedFill>)>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Queries<'w, 's> {
    contents: Query<'w, 's, (Entity, &'static mut RosterContent)>,
    tips: Query<'w, 's, (&'static Children, &'static mut crate::tooltip::Tooltip)>,
    labels: ControlTexts<'w, 's>,
    controls: Query<'w, 's, &'static Control>,
    headers: HeadingTexts<'w, 's>,
    menus: MenuNodes<'w, 's>,
    expand: ExpandNodes<'w, 's>,
    added: Query<'w, 's, (), Added<Summary>>,
}
impl Queries<'_, '_> {
    pub(super) fn refresh(
        &mut self,
        commands: &mut Commands,
        universe: &Universe,
        view: &View,
        summaries: &mut SummaryTexts,
        changed: bool,
    ) {
        if !changed
            && self.added.is_empty()
            && !self
                .contents
                .iter_mut()
                .any(|(_, content)| content.is_added())
        {
            return;
        }
        let rows = rows(universe, view);
        let targets: Vec<_> = rows.iter().map(|row| row.target.clone()).collect();
        for (entity, mut content) in &mut self.contents {
            if content.0 != targets {
                commands
                    .entity(entity)
                    .despawn_children()
                    .with_children(|parent| spawn_rows(parent, &rows));
                content.0 = targets.clone();
            }
        }
        for (summary, mut text) in summaries.iter_mut() {
            let next = rows
                .get(summary.0)
                .map_or_else(String::new, |row| row.cells[summary.1].clone());
            if text.0 != next {
                text.0 = next;
            }
        }
        for (children, mut tip) in &mut self.tips {
            let Some((summary, _)) = children.iter().find_map(|child| summaries.get(child).ok())
            else {
                continue;
            };
            tip.0=match summary.1 {
                2=>"Shared household coins divided evenly; group rows show mean coins per citizen.".into(),
                3=>"Shared household wealth divided evenly, plus each citizen's personal assets; group rows show mean wealth per citizen.".into(),
                4=>"Needs wellbeing, excluding wealth; group rows show the median.".into(),
                1 if view.grouping==Grouping::Profession=>"Citizens are grouped by their starting profession.".into(),
                0 if view.grouping==Grouping::WealthFifth=>"Accounts ranked by wealth per person. Boundaries are nearest to population fifths without splitting households.".into(),
                0 if view.grouping==Grouping::Activity=>"The action happening now, including travel between activities.".into(),
                _=>String::new(),
            };
        }
        for (parent, mut text) in &mut self.labels {
            let Ok(control) = self.controls.get(parent.parent()) else {
                continue;
            };
            if matches!(control.0, Choice::Group) {
                let next = format!(
                    "{} {}",
                    view.grouping.name(),
                    if view.menu { "^" } else { "v" }
                );
                if text.0 != next {
                    text.0 = next;
                }
            }
        }
        for (heading, mut text) in &mut self.headers {
            let label = if heading.0 == 1 && view.grouping != Grouping::None {
                "Profession / count"
            } else {
                COLUMNS[heading.0].0
            };
            let active = matches!(
                (heading.0, view.sort),
                (0, Sort::Name) | (3, Sort::Wealth) | (4, Sort::Wellbeing)
            );
            let next = format!(
                "{label}{}",
                if active {
                    if view.descending { " v" } else { " ^" }
                } else {
                    ""
                }
            );
            if text.0 != next {
                text.0 = next;
            }
        }
        for mut node in &mut self.menus {
            node.display = if view.menu {
                Display::Flex
            } else {
                Display::None
            };
        }
        for mut node in &mut self.expand {
            node.display = if view.grouping != Grouping::None {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{self, Snapshot};
    use learning_lord_simulation::{
        StartingRole,
        farming::CropKind,
        locations::Map,
        marketplace::{Good, ShoppingList},
        production::Recipe,
    };

    fn household_world() -> Universe {
        let (world, a) = Universe::with_map(Map::default())
            .with_citizen(
                "Ada",
                Citizen::new(10.0)
                    .unwrap()
                    .with_coins(100)
                    .unwrap()
                    .with_garment_condition(Some(1.0))
                    .unwrap()
                    .with_starting_role(StartingRole::Farmer),
            )
            .unwrap();
        let (world, b) = world
            .with_citizen(
                "Bram",
                Citizen::new(30.0)
                    .unwrap()
                    .with_coins(300)
                    .unwrap()
                    .with_garment_condition(Some(0.25))
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (world, _) = world
            .with_citizen(
                "Cleo",
                Citizen::new(80.0)
                    .unwrap()
                    .with_coins(50)
                    .unwrap()
                    .with_starting_role(StartingRole::Farmer),
            )
            .unwrap();
        let AgentKind::Citizen(a_citizen) = &world.agents()[&a].kind;
        world
            .with_household("Ada and Bram", a_citizen.home(), &[a, b])
            .unwrap()
            .0
    }

    #[test]
    fn household_shares_preserve_personal_assets_and_town_totals() {
        let world = household_world();
        let members = members(&world);
        let household = world.households().values().next().unwrap();
        let shared = world.household_wealth(household.id).unwrap();
        for member in &members {
            let AgentKind::Citizen(citizen) = &world.agents()[&member.id].kind;
            let expected = if member.household.is_some() {
                shared / 2.0 + citizen.wealth().unwrap()
            } else {
                citizen.wealth().unwrap()
            };
            assert_eq!(member.wealth.as_ref().unwrap(), &expected);
        }
        assert_eq!(
            members.iter().map(|member| member.coins).sum::<f64>(),
            450.0
        );
        assert_eq!(members[0].coins, 200.0);
        assert_eq!(members[1].coins, 200.0);
        assert!(members[0].wealth.as_ref().unwrap() > members[1].wealth.as_ref().unwrap());
        let personal: f64 = world
            .agents()
            .values()
            .map(|agent| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                citizen.wealth().unwrap()
            })
            .sum();
        assert!(
            (members
                .iter()
                .map(|member| member.wealth.as_ref().unwrap())
                .sum::<f64>()
                - shared
                - personal)
                .abs()
                < 1e-9
        );
        let view = View {
            grouping: Grouping::Household,
            ..default()
        };
        let rows = rows(&world, &view);
        let group = rows
            .iter()
            .find(|row| matches!(row.target, Target::Group(_)))
            .unwrap();
        assert_eq!(group.cells[1], "2 citizens");
        assert_eq!(group.cells[2], "200.0");
        assert_eq!(group.cells[4], format!("{:.1}", median(&members[..2])));
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row.target, Target::Citizen(_)))
                .count(),
            1
        );
    }

    #[test]
    fn every_grouping_partitions_citizens_and_open_all_exposes_every_member_once() {
        let world = simulation::new_universe().unwrap();
        let expected: BTreeSet<_> = world.agents().keys().map(|id| id.0).collect();
        for grouping in Grouping::ALL {
            let mut view = View {
                grouping,
                ..default()
            };
            if grouping != Grouping::None {
                open_all(&world, &mut view);
            }
            let rows = rows(&world, &view);
            let ids: Vec<_> = rows
                .iter()
                .filter_map(|row| match row.target {
                    Target::Citizen(id) => Some(id.0),
                    _ => None,
                })
                .collect();
            assert_eq!(ids.len(), 30, "{grouping:?}");
            assert_eq!(
                ids.into_iter().collect::<BTreeSet<_>>(),
                expected,
                "{grouping:?}"
            );
        }
        let view = View::default();
        let collapsed = rows(&world, &view);
        assert_eq!(collapsed.len(), 8);
        assert!(
            collapsed
                .iter()
                .all(|row| matches!(row.target, Target::Group(_)))
        );
    }

    #[test]
    fn wealth_fifths_use_population_boundaries_and_keep_accounts_together() {
        let world = simulation::new_universe().unwrap();
        let mut members = members(&world);
        members.truncate(10);
        let shared = members[0].id;
        for (index, member) in members.iter_mut().enumerate() {
            member.account = if index < 3 { shared } else { member.id };
            member.wealth = Ok(if index < 3 { 0.0 } else { index as f64 * 10.0 });
        }
        let bands = fifths(&members);
        let assigned: Vec<_> = members
            .iter()
            .map(|member| bands[&member.account.0.to_string()])
            .collect();
        assert_eq!(assigned, [0, 0, 0, 1, 2, 2, 3, 3, 4, 4]);
        // A household larger than a fifth can leave intermediate bands empty.
        for member in members.iter_mut().take(8) {
            member.account = shared;
            member.wealth = Ok(0.0);
        }
        let bands = fifths(&members);
        assert!(
            members[..8]
                .iter()
                .all(|member| bands[&member.account.0.to_string()] == bands[&shared.0.to_string()])
        );
        assert_eq!(bands[&members[8].account.0.to_string()], 4);
        // Different personal wealth does not split an account.
        members[0].wealth = Ok(10_000.0);
        let bands = fifths(&members);
        assert!(
            members[..8]
                .iter()
                .all(|member| bands[&member.account.0.to_string()] == bands[&shared.0.to_string()])
        );
    }

    #[test]
    fn sorting_uses_defaults_reversals_group_metrics_and_name_ties() {
        let world = household_world();
        let mut view = View {
            grouping: Grouping::None,
            ..default()
        };
        assert_eq!(
            rows(&world, &view)
                .iter()
                .map(|row| row.cells[0].as_str())
                .collect::<Vec<_>>(),
            ["Ada", "Bram", "Cleo"]
        );
        view.sort_by(Sort::Wellbeing);
        assert!(!view.descending);
        assert_eq!(rows(&world, &view)[0].cells[0], "Cleo");
        view.sort_by(Sort::Wellbeing);
        assert!(view.descending);
        assert_eq!(rows(&world, &view)[0].cells[0], "Ada");
        view.sort_by(Sort::Wealth);
        assert!(view.descending);
        assert_eq!(rows(&world, &view)[0].cells[0], "Ada");
        view.sort_by(Sort::Wealth);
        assert!(!view.descending);
        assert_eq!(rows(&world, &view)[0].cells[0], "Cleo");
        view.grouping = Grouping::Profession;
        view.sort_by(Sort::Wellbeing);
        let groups = rows(&world, &view);
        assert!(groups[0].cells[0].contains("Farmer"));
        open_all(&world, &mut view);
        assert_eq!(
            rows(&world, &view)
                .iter()
                .filter(|row| row.child)
                .map(|row| row.cells[0].trim())
                .collect::<Vec<_>>(),
            ["Cleo", "Ada", "Bram"]
        );
        let mut members = members(&world);
        for member in &mut members {
            member.wealth = Ok(100.0);
            member.wellbeing = 0.0;
        }
        members.reverse();
        view.sort_by(Sort::Wealth);
        sort_members(&mut members, &view);
        assert_eq!(
            members
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
            ["Ada", "Bram", "Cleo"]
        );
        assert_eq!(median(&members), 0.0);
        members[0].wellbeing = -90.0;
        members[1].wellbeing = -10.0;
        members[2].wellbeing = 40.0;
        assert_eq!(median(&members), -10.0);
        assert_eq!(median(&members[..2]), -50.0);
    }

    #[test]
    fn current_action_categories_cover_all_actions() {
        let world = household_world();
        let AgentKind::Citizen(citizen) = &sorted_agents(&world)[0].1.kind;
        let place = citizen.home();
        let list = ShoppingList::default();
        let cases = [
            (CitizenAction::Eat, "Personal needs"),
            (CitizenAction::Drink, "Personal needs"),
            (CitizenAction::Sleep, "Personal needs"),
            (CitizenAction::EquipClothing, "Personal needs"),
            (CitizenAction::Wait, "Waiting / idle"),
            (CitizenAction::BuyFood(Good::Berries), "Trading"),
            (CitizenAction::Buy(list), "Trading"),
            (CitizenAction::BuyAt { place, list }, "Trading"),
            (CitizenAction::ListExcess, "Trading"),
            (CitizenAction::List(Good::Berries, 1), "Trading"),
            (CitizenAction::ListAt { place, list }, "Trading"),
            (CitizenAction::Produce(Recipe::Forage), "Producing"),
            (
                CitizenAction::ProduceAt {
                    place,
                    recipe: Recipe::Forage,
                },
                "Producing",
            ),
            (
                CitizenAction::Plant {
                    field: place,
                    plot: 0,
                    crop: CropKind::Wheat,
                },
                "Producing",
            ),
            (
                CitizenAction::Tend {
                    field: place,
                    plot: 0,
                    stage: 0,
                },
                "Producing",
            ),
            (
                CitizenAction::Harvest {
                    field: place,
                    plot: 0,
                },
                "Producing",
            ),
            (
                CitizenAction::Clear {
                    field: place,
                    plot: 0,
                },
                "Producing",
            ),
            (
                CitizenAction::CollectBeer {
                    brewery: place,
                    slot: 0,
                },
                "Moving supplies",
            ),
            (CitizenAction::Withdraw(Good::Berries, 1), "Moving supplies"),
            (
                CitizenAction::WithdrawAt {
                    place,
                    good: Good::Berries,
                    units: 1,
                },
                "Moving supplies",
            ),
            (
                CitizenAction::CollectStored { place, list },
                "Moving supplies",
            ),
            (
                CitizenAction::DepositStored { place, list },
                "Moving supplies",
            ),
            (CitizenAction::Travel(place), "Travelling"),
        ];
        for (action, expected) in cases {
            assert_eq!(activity(Some(action)), expected, "{action:?}");
        }
        assert_eq!(activity(None), "Waiting / idle");
    }

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe: household_world(),
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: default(),
            generation: 0,
            revision: 0,
        }))
        .init_resource::<Selection>()
        .init_resource::<RosterState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<bevy::input_focus::InputFocus>()
        .add_systems(Startup, |mut commands: Commands| {
            commands
                .spawn(Node::default())
                .with_children(|parent| spawn_roster(parent, 3));
        })
        .add_systems(
            Update,
            (handle_selection, refresh_cards, refresh_screens).chain(),
        );
        app.update();
        app
    }
    fn control(app: &mut App, predicate: impl Fn(Choice) -> bool) -> Entity {
        app.world_mut()
            .query::<(Entity, &Control)>()
            .iter(app.world())
            .find(|(_, control)| predicate(control.0))
            .unwrap()
            .0
    }
    fn press(app: &mut App, entity: Entity) {
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
        if let Ok(mut entity) = app.world_mut().get_entity_mut(entity) {
            entity.insert(Interaction::None);
        }
        app.update();
    }
    fn cards(app: &mut App) -> Vec<(Entity, Target)> {
        app.world_mut()
            .query::<(Entity, &Card)>()
            .iter(app.world())
            .map(|(entity, card)| (entity, card.0.clone()))
            .collect()
    }

    #[test]
    fn controls_expansion_selection_snapshot_refresh_and_restart() {
        let mut app = app();
        assert_eq!(cards(&mut app).len(), 2);
        let hidden = control(&mut app, |choice| {
            matches!(choice, Choice::SelectGroup(Grouping::Household))
        });
        press(&mut app, hidden);
        assert_eq!(
            app.world().resource::<RosterState>().view.grouping,
            Grouping::Profession
        );
        let menu = control(&mut app, |choice| matches!(choice, Choice::Group));
        app.world_mut()
            .entity_mut(menu)
            .insert(Interaction::Pressed);
        app.world_mut()
            .entity_mut(hidden)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<RosterState>().view.grouping,
            Grouping::Profession
        );
        assert!(app.world().resource::<RosterState>().view.menu);
        app.world_mut().entity_mut(menu).insert(Interaction::None);
        app.world_mut().entity_mut(hidden).insert(Interaction::None);
        app.update();
        press(&mut app, menu);
        assert!(
            cards(&mut app)
                .iter()
                .all(|(_, target)| matches!(target, Target::Group(_)))
        );
        let open = control(&mut app, |choice| matches!(choice, Choice::OpenAll));
        press(&mut app, open);
        assert_eq!(cards(&mut app).len(), 5);
        let stable = cards(&mut app);
        let selected = app.world().resource::<Selection>().0;
        let group = control(&mut app, |choice| matches!(choice, Choice::Group));
        press(&mut app, group);
        assert!(app.world().resource::<RosterState>().view.menu);
        let wellbeing = control(&mut app, |choice| {
            matches!(choice, Choice::Sort(Sort::Wellbeing))
        });
        press(&mut app, wellbeing);
        assert!(!app.world().resource::<RosterState>().view.descending);
        let revision = app.world().resource::<DisplaySnapshot>().0.revision;
        let world = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .advance(1)
            .unwrap();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = world;
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision = revision + 1;
        app.update();
        assert_eq!(
            app.world().resource::<RosterState>().view.sort,
            Sort::Wellbeing
        );
        assert_eq!(app.world().resource::<RosterState>().view.open.len(), 2);
        assert_eq!(app.world().resource::<Selection>().0, selected);
        let current = cards(&mut app);
        app.update();
        assert_eq!(cards(&mut app), current);
        assert!(
            stable
                .iter()
                .any(|(_, target)| matches!(target, Target::Citizen(_)))
        );
        let (entity, id) = current
            .iter()
            .find_map(|(entity, target)| match target {
                Target::Citizen(id) => Some((*entity, *id)),
                _ => None,
            })
            .unwrap();
        press(&mut app, entity);
        assert!(app.world().resource::<RosterState>().detail);
        assert_eq!(app.world().resource::<Selection>().0, Some(id));
        // Hidden roster controls cannot change the view while inspecting a citizen.
        let collapse = control(&mut app, |choice| matches!(choice, Choice::CollapseAll));
        press(&mut app, collapse);
        assert_eq!(app.world().resource::<RosterState>().view.open.len(), 2);
        app.world_mut().resource_mut::<RosterState>().detail = false;
        press(&mut app, collapse);
        assert!(app.world().resource::<RosterState>().view.open.is_empty());
        assert_eq!(cards(&mut app).len(), 2);
        let household = control(&mut app, |choice| {
            matches!(choice, Choice::SelectGroup(Grouping::Household))
        });
        press(&mut app, household);
        assert!(!app.world().resource::<RosterState>().view.menu);
        let independent = cards(&mut app)
            .into_iter()
            .find(|(_, target)| matches!(target, Target::Citizen(_)))
            .unwrap();
        press(&mut app, independent.0);
        assert!(app.world().resource::<RosterState>().detail);
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        let state = app.world().resource::<RosterState>();
        assert!(!state.detail);
        assert_eq!(state.view.grouping, Grouping::Profession);
        assert_eq!(state.view.sort, Sort::Name);
        assert!(!state.view.descending);
        assert!(state.view.open.is_empty());
        assert_eq!(cards(&mut app).len(), 2);
    }

    #[test]
    fn rendered_citizen_identity_survives_a_snapshot_that_reorders_wealth() {
        let mut app = app();
        let menu = control(&mut app, |choice| matches!(choice, Choice::Group));
        press(&mut app, menu);
        let none = control(&mut app, |choice| {
            matches!(choice, Choice::SelectGroup(Grouping::None))
        });
        press(&mut app, none);
        assert_eq!(
            app.world_mut()
                .query_filtered::<&Node, With<ExpandControl>>()
                .single(app.world())
                .unwrap()
                .display,
            Display::None
        );
        let wealth = control(&mut app, |choice| {
            matches!(choice, Choice::Sort(Sort::Wealth))
        });
        press(&mut app, wealth);
        let before = cards(&mut app);
        let (entity, target) = before[0].clone();
        let Target::Citizen(clicked) = target else {
            panic!("individual row")
        };
        let world = &app.world().resource::<DisplaySnapshot>().0.universe;
        let (other, agent) = sorted_agents(world)
            .into_iter()
            .find(|(_, agent)| agent.name == "Cleo")
            .unwrap();
        let AgentKind::Citizen(citizen) = &agent.kind;
        let next = world
            .with_stored_good(
                citizen.home(),
                learning_lord_simulation::storage::GoodsOwner::Agent(other),
                Good::Berries,
                1_000_000,
            )
            .unwrap();
        let expected = rows(&next, &app.world().resource::<RosterState>().view);
        assert_eq!(expected[0].target, Target::Citizen(other));
        assert_ne!(clicked, other);
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = next;
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(app.world().resource::<Selection>().0, Some(clicked));
        assert!(app.world().resource::<RosterState>().detail);
        // Replacing the universe cannot open an inspector for a stale rendered ID.
        app.world_mut().resource_mut::<RosterState>().detail = false;
        let stale = cards(&mut app)[0].0;
        let replacement = household_world();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = replacement.clone();
        app.world_mut()
            .entity_mut(stale)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app.world().resource::<RosterState>().detail);
        assert!(
            app.world()
                .resource::<Selection>()
                .0
                .is_some_and(|id| replacement.agents().contains_key(&id))
        );
    }
}
