use crate::{
    DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, citizens,
    floating_ui::WindowKind,
    guild_history, household_history, locations,
    navigation::{self, Target},
    text,
};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use learning_lord_simulation::{AgentId, Universe, locations::Location, storage::GoodsOwner};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Households,
    Guilds,
    Businesses,
}

#[derive(Resource, Default)]
pub(crate) struct State {
    tab: Tab,
    selected: Option<AgentId>,
    selected_guild: Option<AgentId>,
    generation: u64,
    revision: Option<u64>,
    dirty: bool,
    display_key: String,
    guild_history_view: household_history::View,
    guild_history_day: Option<u64>,
    guild_history_record: Option<learning_lord_simulation::guild_history::DailyGuildHistory>,
    history_view: household_history::View,
    history_day: Option<u64>,
    history_record: Option<learning_lord_simulation::household_history::DailyHouseholdHistory>,
}
impl State {
    pub(crate) fn title(&self) -> &'static str {
        match self.tab {
            Tab::Households => "Households",
            Tab::Guilds => "Guilds",
            Tab::Businesses => "Businesses",
        }
    }
}

#[cfg(test)]
impl State {
    fn select_guild(&mut self, id: AgentId) {
        self.tab = Tab::Guilds;
        self.selected_guild = Some(id);
        self.guild_history_day = None;
        self.dirty = true;
    }

    fn select(&mut self, id: AgentId) {
        self.tab = Tab::Households;
        self.selected = Some(id);
        self.history_day = None;
        self.dirty = true;
    }
}
#[derive(Component)]
pub(crate) struct Content;
#[derive(Component, Clone, Copy)]
pub(crate) struct ScrollPanel(pub(crate) bool);
#[derive(Component)]
pub(crate) struct Choose(Tab);

pub(crate) fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            column_gap: px(8),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (tab, label) in [
                (Tab::Households, "Households"),
                (Tab::Guilds, "Guilds"),
                (Tab::Businesses, "Businesses"),
            ] {
                row.spawn((
                    Button,
                    Choose(tab),
                    BackgroundColor(PANEL),
                    Node {
                        padding: UiRect::all(px(8)),
                        ..default()
                    },
                ))
                .with_children(|button| {
                    button.spawn(text(label, 14.0, TEXT));
                });
            }
        });
    spawn_content(parent, ());
}

pub(crate) fn spawn_entity(parent: &mut ChildSpawnerCommands, kind: WindowKind) {
    spawn_content(parent, (EntityDetails(kind), DetailState::default()));
}

#[derive(Component)]
pub(crate) struct EntityDetails(WindowKind);

#[derive(Component, Default)]
pub(crate) struct DetailState(State);

fn spawn_content(parent: &mut ChildSpawnerCommands, state: impl Bundle) {
    parent.spawn((
        Content,
        state,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            column_gap: px(14),
            margin: UiRect::top(px(10)),
            ..default()
        },
    ));
}

pub(crate) fn handle(
    mut overview: ResMut<State>,
    choices: Query<(&Interaction, &Choose), Changed<Interaction>>,
    history_choices: Query<
        (Entity, &Interaction, &household_history::Choice),
        Changed<Interaction>,
    >,
    guild_history_choices: Query<
        (Entity, &Interaction, &guild_history::Choice),
        Changed<Interaction>,
    >,
    mut scrolls: Query<(Entity, &ScrollPanel, &mut ScrollPosition)>,
    mut panes: Query<&mut DetailState>,
    parents: Query<&ChildOf>,
) {
    let changed = history_choices
        .iter()
        .filter_map(|(entity, interaction, choice)| {
            (*interaction == Interaction::Pressed).then_some((entity, false, *choice))
        })
        .chain(
            guild_history_choices
                .iter()
                .filter_map(|(entity, interaction, choice)| {
                    let choice = match *choice {
                        guild_history::Choice::View(view) => household_history::Choice::View(view),
                        guild_history::Choice::Day(day) => household_history::Choice::Day(day),
                    };
                    (*interaction == Interaction::Pressed).then_some((entity, true, choice))
                }),
        );
    for (entity, guild, choice) in changed {
        let pane = parents
            .iter_ancestors(entity)
            .find(|&ancestor| panes.contains(ancestor));
        for (entity, panel, mut position) in &mut scrolls {
            let owner = parents
                .iter_ancestors(entity)
                .find(|&ancestor| panes.contains(ancestor));
            if panel.0 && owner == pane {
                position.0 = Vec2::ZERO;
            }
        }
        let mut detail = pane.and_then(|pane| panes.get_mut(pane).ok());
        let state = detail
            .as_deref_mut()
            .map(|pane| &mut pane.0)
            .unwrap_or(&mut overview);
        match (guild, choice) {
            (false, household_history::Choice::View(view)) => state.history_view = view,
            (false, household_history::Choice::Day(day)) => state.history_day = day,
            (true, household_history::Choice::View(view)) => state.guild_history_view = view,
            (true, household_history::Choice::Day(day)) => state.guild_history_day = day,
        }
        state.dirty = true;
    }
    for (interaction, choice) in &choices {
        if *interaction == Interaction::Pressed && overview.tab != choice.0 {
            overview.tab = choice.0;
            overview.dirty = true;
        }
    }
}

pub(crate) fn row(
    parent: &mut ChildSpawnerCommands,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(6),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(children);
}
pub(crate) fn label(parent: &mut ChildSpawnerCommands, value: impl Into<String>) {
    parent.spawn(text(value, 14.0, TEXT));
}
pub(crate) fn place_link(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    id: learning_lord_simulation::locations::PlaceId,
) {
    if let Ok(place) = universe.map().place(id) {
        navigation::link(parent, &place.name, Target::Location(id));
    }
}
fn living_tier_tooltip(
    standard: &learning_lord_simulation::living_standards::LivingStandard,
    now: u64,
) -> String {
    use learning_lord_simulation::living_standards::LivingStandardResult;
    let assessment = standard.last_check.as_ref().map_or_else(
        || "Last assessment: none".into(),
        |check| {
            let result = match check.result {
                LivingStandardResult::Promoted => "promoted",
                LivingStandardResult::Demoted => "demoted",
                LivingStandardResult::Stayed => "unchanged",
                LivingStandardResult::PromotionCooldown => "promotion cooldown",
                LivingStandardResult::AwaitingFreshDays => "awaiting seven fresh days",
            };
            format!(
                "Last assessment: {} - {result}",
                citizens::duration(check.time_ms)
            )
        },
    );
    let mut tip = format!(
        "{assessment}\nNext assessment: {}",
        citizens::duration(standard.next_check_ms(now))
    );
    if standard.promotion_allowed_ms > now {
        tip.push_str(&format!(
            "\nPromotion allowed: {}",
            citizens::duration(standard.promotion_allowed_ms)
        ));
    }
    for (index, description) in [
        "Hunger, thirst, sleep, discomfort",
        "+ clothing",
        "+ meat, carbohydrates, fibre",
        "+ alcohol (ale)",
        "+ sweets",
    ]
    .iter()
    .enumerate()
    {
        let tier = index as u8 + 1;
        tip.push_str(&format!(
            "\n{}Tier {tier}: {description}",
            if standard.tier == tier { "> " } else { "  " }
        ));
    }
    tip
}

fn details(parent: &mut ChildSpawnerCommands, universe: &Universe, id: AgentId) {
    let household = &universe.households()[&id];
    parent.spawn(text(&household.name, 18.0, TEXT));
    parent.spawn((
        text(
            &format!("Living tier: {}", household.living_standard.tier),
            14.0,
            TEXT,
        ),
        Interaction::None,
        crate::tooltip::Tooltip(living_tier_tooltip(
            &universe.households()[&id].living_standard,
            universe.current_time_ms(),
        )),
    ));
    row(parent, |row| {
        label(row, "Members:");
        for member in &household.members {
            navigation::link(
                row,
                locations::name(universe, *member),
                Target::Citizen(*member),
            );
        }
    });
    row(parent, |row| {
        label(row, "Shared home:");
        if let Some(home) = household.home {
            place_link(row, universe, home);
        } else {
            label(row, "None");
        }
    });
    let wealth = universe
        .household_wealth(id)
        .map_or_else(|error| error.to_string(), |wealth| format!("{wealth:.3}"));
    label(
        parent,
        format!(
            "Shared coins: {} | Shared wealth: {wealth} coins",
            household.coins
        ),
    );
    let fees_owed: f64 = universe
        .workshop_fee_accounts()
        .iter()
        .filter(|((payer, _), _)| *payer == id)
        .map(|(_, fees)| fees.liability())
        .sum();
    if fees_owed > 0.0 {
        label(
            parent,
            format!("Workshop fees owed: {fees_owed:.3} coins (including fractional carry)"),
        );
    }
    parent.spawn(text("SHARED STOCKS BY LOCATION", 14.0, MUTED));
    let mut places = universe
        .storage()
        .stock()
        .iter()
        .filter(|(key, units)| key.owner == GoodsOwner::Household(id) && **units > 0)
        .map(|(key, _)| key.place)
        .chain(
            universe
                .market()
                .orders()
                .filter(|order| {
                    order.seller
                        == learning_lord_simulation::marketplace::MarketParty::Household(id)
                })
                .map(|order| order.place),
        )
        .collect::<Vec<_>>();
    places.sort_by_key(|place| place.0);
    places.dedup();
    for place in &places {
        row(parent, |row| {
            place_link(row, universe, *place);
        });
        for good in learning_lord_simulation::marketplace::Good::ALL {
            let stored = universe
                .storage()
                .stock()
                .iter()
                .find(|(key, _)| {
                    key.place == *place
                        && key.owner == GoodsOwner::Household(id)
                        && key.good == good
                })
                .map_or(0, |(_, units)| *units);
            let reserved =
                universe
                    .storage()
                    .reserved_units(*place, GoodsOwner::Household(id), good);
            let listed = universe
                .market()
                .orders()
                .filter(|order| {
                    order.place == *place
                        && order.good == good
                        && order.seller
                            == learning_lord_simulation::marketplace::MarketParty::Household(id)
                })
                .map(|order| order.units)
                .sum();
            if stored == 0 && listed == 0 {
                continue;
            }
            row(parent, |row| {
                navigation::link(row, good.name(), Target::Good(good));
                label(
                    row,
                    format!(
                        "{} stored | {} tax reserved | {} listed",
                        crate::quantity_label(good, stored),
                        crate::quantity_label(good, reserved),
                        crate::quantity_label(good, listed)
                    ),
                );
            });
        }
    }
    if places.is_empty() {
        label(parent, "None");
    }
    parent.spawn(text("CARRIED SHARED GOODS", 14.0, MUTED));
    let mut carried = false;
    for member in &household.members {
        let Some(agent) = universe.agents().get(member) else {
            continue;
        };
        let learning_lord_simulation::AgentKind::Citizen(citizen) = &agent.kind;
        let goods = learning_lord_simulation::marketplace::Good::ALL
            .into_iter()
            .filter(|good| citizen.units(*good) > 0)
            .collect::<Vec<_>>();
        if goods.is_empty() {
            continue;
        }
        carried = true;
        row(parent, |row| {
            navigation::link(row, &agent.name, Target::Citizen(*member));
        });
        for good in goods {
            row(parent, |row| {
                navigation::link(row, good.name(), Target::Good(good));
                label(
                    row,
                    format!(
                        "{} carried | {} tax reserved",
                        crate::quantity_label(good, citizen.units(good)),
                        crate::quantity_label(good, citizen.tax_reserved_units(good))
                    ),
                );
            });
        }
    }
    if !carried {
        label(parent, "None");
    }
    parent.spawn(text("ACCESSIBLE WORKPLACES", 14.0, MUTED));
    let map = universe.map();
    let mut workplaces = map
        .places()
        .values()
        .filter(|place| {
            place.kind != Location::Home
                && place.owner.is_some_and(|owner| {
                    owner == household.id
                        || universe.guilds().get(&owner).is_some_and(|guild| {
                            guild.workshops.contains(&place.id)
                                && guild
                                    .members
                                    .iter()
                                    .any(|member| household.members.contains(member))
                        })
                })
        })
        .collect::<Vec<_>>();
    workplaces.sort_by_key(|place| place.id.0);
    for place in &workplaces {
        row(parent, |row| {
            place_link(row, universe, place.id);
            label(row, "| Owner:");
            if let Some(owner) = place.owner {
                navigation::render(row, &[navigation::account(universe, owner)], 14.0, TEXT);
            }
        });
    }
    if workplaces.is_empty() {
        label(parent, "None");
    }
    parent.spawn(text("MAINTENANCE", 14.0, MUTED));
    for (name, goal, task) in [
        (
            "Replenishment",
            learning_lord_simulation::planning::goals::Effect::ReplenishReserves,
            &household.maintenance.replenishment,
        ),
        (
            "Listing excess",
            learning_lord_simulation::planning::goals::Effect::ListExcess,
            &household.maintenance.listing,
        ),
    ] {
        row(parent, |row| {
            label(
                row,
                format!(
                    "{name}: {} cooldown remaining | Committed worker:",
                    citizens::duration(task.remaining_ms(goal, universe.current_time_ms()))
                ),
            );
            if let Some(claimant) = task.claimant {
                navigation::link(
                    row,
                    locations::name(universe, claimant),
                    Target::Citizen(claimant),
                );
            } else {
                label(row, "None");
            }
        });
    }
}

fn guild_content(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    state: &State,
    offsets: [Vec2; 2],
    show_list: bool,
    show_details: bool,
) {
    let selected = state.selected_guild;
    if show_list {
        parent
            .spawn((
                Node {
                    width: if show_details { px(230) } else { percent(100) },
                    flex_shrink: 0.0,
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition(offsets[0]),
                RelativeCursorPosition::default(),
                ScrollPanel(false),
            ))
            .with_children(|list| {
                let mut guilds = universe.guilds().values().collect::<Vec<_>>();
                guilds.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.0.cmp(&b.id.0)));
                for guild in guilds {
                    list.spawn((
                        Node::default(),
                        BackgroundColor(if selected == Some(guild.id) {
                            SELECTED
                        } else {
                            PANEL
                        }),
                    ))
                    .with_children(|entry| {
                        navigation::link(entry, &guild.name, Target::Guild(guild.id))
                    });
                }
                if universe.guilds().is_empty() {
                    label(list, "No guilds");
                }
            });
    }
    if !show_details {
        return;
    }
    parent
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(offsets[1]),
            RelativeCursorPosition::default(),
            ScrollPanel(true),
        ))
        .with_children(|panel| {
            let Some(guild) = selected.and_then(|id| universe.guilds().get(&id)) else {
                label(panel, "Select a guild");
                return;
            };
            guild_history::controls(
                panel,
                universe,
                guild.id,
                state.guild_history_view,
                state.guild_history_day,
            );
            panel.spawn(text(&guild.name, 18.0, TEXT));
            if state.guild_history_view == household_history::View::History {
                if let Some(day) = &state.guild_history_record {
                    guild_history::render(panel, universe, day, state.guild_history_day.is_none());
                }
                return;
            }
            label(
                panel,
                format!(
                    "Guild coins: {} | Workshop fee: {} coin per production hour",
                    guild.coins, guild.hourly_fee
                ),
            );
            let mut accounts = universe
                .workshop_fee_accounts()
                .iter()
                .filter(|((_, payee), _)| *payee == guild.id)
                .collect::<Vec<_>>();
            accounts.sort_by_key(|((payer, _), _)| payer.0);
            let owed: f64 = accounts.iter().map(|(_, fees)| fees.liability()).sum();
            label(
                panel,
                format!("Workshop fees owed: {owed:.3} coins (including fractional carry)"),
            );
            for ((payer, _), fees) in accounts {
                row(panel, |row| {
                    navigation::render(row, &[navigation::account(universe, *payer)], 14.0, TEXT);
                    label(
                        row,
                        format!(
                            "owes {} coins | Fractional carry {:.3} coins",
                            fees.unpaid,
                            fees.liability() - fees.unpaid as f64
                        ),
                    );
                });
            }
            label(panel, "Members keep separate accounts and own their goods.");
            row(panel, |row| {
                label(row, "Members:");
                for member in &guild.members {
                    navigation::link(
                        row,
                        locations::name(universe, *member),
                        Target::Citizen(*member),
                    );
                }
            });
            panel.spawn(text(
                "SHARED WORKSHOPS · INDIVIDUALLY OWNED STOCKS",
                14.0,
                MUTED,
            ));
            for place in &guild.workshops {
                place_link(panel, universe, *place);
                let mut rows = crate::locations::stock_rows(universe, *place);
                if rows.is_empty() {
                    label(panel, "No workshop stocks");
                }
                for fragments in rows.drain(..) {
                    navigation::render(panel, &fragments, 14.0, TEXT);
                }
            }
        });
}

pub(crate) fn refresh(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    mut overview: ResMut<State>,
    mut content: Query<
        (
            Entity,
            Option<&Children>,
            Option<(&EntityDetails, &mut DetailState)>,
        ),
        With<Content>,
    >,
    mut tabs: Query<(&Choose, &mut BackgroundColor)>,
    scrolls: Query<(Entity, &ScrollPanel, &ScrollPosition)>,
    parents: Query<&ChildOf>,
    windows: Option<Res<crate::floating_ui::Windows>>,
) {
    for (root, children, detail) in &mut content {
        let (kind, state) = match detail {
            Some((details, state)) => (details.0, Some(state)),
            None => (WindowKind::Collectives, None),
        };
        if windows
            .as_ref()
            .is_some_and(|windows| !windows.is_open(kind))
        {
            continue;
        }
        let mut detail = state;
        let state = detail
            .as_deref_mut()
            .map(|pane| &mut pane.0)
            .unwrap_or(&mut overview);
        let mut offsets = [Vec2::ZERO; 2];
        for (entity, panel, position) in &scrolls {
            if parents
                .iter_ancestors(entity)
                .any(|ancestor| ancestor == root)
            {
                offsets[usize::from(panel.0)] = position.0;
            }
        }
        refresh_pane(
            &mut commands,
            &snapshot,
            root,
            children.is_some(),
            state,
            kind,
            offsets,
            windows.is_none(),
        );
    }
    for (tab, mut color) in &mut tabs {
        color.set_if_neq(BackgroundColor(if tab.0 == overview.tab {
            SELECTED
        } else {
            PANEL
        }));
    }
}

fn refresh_pane(
    commands: &mut Commands,
    snapshot: &DisplaySnapshot,
    entity: Entity,
    has_children: bool,
    state: &mut State,
    kind: WindowKind,
    offsets: [Vec2; 2],
    combined: bool,
) {
    if state.generation != snapshot.0.generation {
        *state = State {
            generation: snapshot.0.generation,
            dirty: true,
            ..default()
        };
    }
    if !state.dirty && state.revision == Some(snapshot.0.revision) && has_children {
        return;
    }
    let universe = &snapshot.0.universe;
    match kind {
        WindowKind::Household(id) => {
            state.tab = Tab::Households;
            state.selected = universe.households().contains_key(&id).then_some(id);
        }
        WindowKind::Guild(id) => {
            state.tab = Tab::Guilds;
            state.selected_guild = universe.guilds().contains_key(&id).then_some(id);
        }
        WindowKind::Collectives if combined => {
            if state
                .selected
                .is_none_or(|id| !universe.households().contains_key(&id))
            {
                state.selected = universe.households().keys().copied().min_by_key(|id| id.0);
            }
            if state
                .selected_guild
                .is_none_or(|id| !universe.guilds().contains_key(&id))
            {
                state.selected_guild = universe.guilds().keys().copied().min_by_key(|id| id.0);
            }
        }
        _ => {}
    }
    let show_list = kind == WindowKind::Collectives;
    let show_details = combined || !show_list;
    let display_key = if !show_details {
        match state.tab {
            Tab::Households => format!(
                "{:?}",
                universe
                    .households()
                    .values()
                    .map(|household| (household.id, &household.name))
                    .collect::<Vec<_>>()
            ),
            Tab::Guilds => format!(
                "{:?}",
                universe
                    .guilds()
                    .values()
                    .map(|guild| (guild.id, &guild.name))
                    .collect::<Vec<_>>()
            ),
            Tab::Businesses => String::new(),
        }
    } else if state.tab == Tab::Guilds
        && state.guild_history_view == household_history::View::History
    {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            state.selected_guild,
            universe
                .guilds()
                .values()
                .map(|g| (g.id, &g.name))
                .collect::<Vec<_>>(),
            universe
                .agents()
                .iter()
                .map(|(id, a)| (id, &a.name))
                .collect::<Vec<_>>(),
            universe
                .map()
                .places()
                .values()
                .map(|p| (p.id, &p.name))
                .collect::<Vec<_>>(),
            state
                .selected_guild
                .and_then(|id| universe.guild_history(id))
                .map(|h| (
                    h.completed.front().map(|d| d.start_ms),
                    h.completed.back().map(|d| d.start_ms)
                ))
        )
    } else if state.tab == Tab::Guilds {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            universe.workshop_fee_accounts(),
            state.selected_guild,
            universe.guilds(),
            universe
                .guilds()
                .values()
                .flat_map(|guild| guild.workshops.iter())
                .map(|place| (*place, crate::locations::stock_rows(universe, *place)))
                .collect::<Vec<_>>(),
            universe.market().orders().collect::<Vec<_>>(),
            universe
                .map()
                .places()
                .values()
                .map(|p| (p.id, &p.name, p.owner))
                .collect::<Vec<_>>(),
            universe
                .agents()
                .iter()
                .map(|(id, a)| (id, &a.name))
                .collect::<Vec<_>>()
        )
    } else if state.history_view == household_history::View::History {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            state.tab,
            state.selected,
            universe
                .households()
                .values()
                .map(|h| (h.id, &h.name))
                .collect::<Vec<_>>(),
            universe
                .agents()
                .iter()
                .map(|(id, agent)| (*id, &agent.name))
                .collect::<Vec<_>>(),
            universe
                .map()
                .places()
                .values()
                .map(|place| (place.id, &place.name))
                .collect::<Vec<_>>(),
            state
                .selected
                .and_then(|id| universe.household_history(id))
                .map(|history| (
                    history.completed.front().map(|day| day.start_ms),
                    history.completed.back().map(|day| day.start_ms)
                ))
        )
    } else {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            state.tab,
            state.selected,
            universe.households(),
            universe.storage().stock(),
            universe.market().orders().collect::<Vec<_>>()
        )
    };
    let selected_key = state
        .selected
        .filter(|_| {
            state.tab == Tab::Households && state.history_view == household_history::View::Current
        })
        .map(|id| {
            let h = &universe.households()[&id];
            format!(
                "{:?}|{:?}|{}|{}|{:?}|{:?}|{:?}|{:?}",
                universe.household_wealth(id),
                h.members
                    .iter()
                    .filter_map(|id| universe.agents().get(id))
                    .map(|a| {
                        let learning_lord_simulation::AgentKind::Citizen(c) = &a.kind;
                        learning_lord_simulation::marketplace::Good::ALL
                            .map(|g| (c.units(g), c.tax_reserved_units(g)))
                    })
                    .collect::<Vec<_>>(),
                citizens::duration(h.maintenance.replenishment.remaining_ms(
                    learning_lord_simulation::planning::goals::Effect::ReplenishReserves,
                    universe.current_time_ms()
                )),
                citizens::duration(h.maintenance.listing.remaining_ms(
                    learning_lord_simulation::planning::goals::Effect::ListExcess,
                    universe.current_time_ms()
                )),
                universe
                    .map()
                    .places()
                    .values()
                    .filter(|place| place.owner.is_some_and(|owner| owner == h.id
                        || universe.guilds().get(&owner).is_some_and(|guild| guild
                            .workshops
                            .contains(&place.id)
                            && guild
                                .members
                                .iter()
                                .any(|member| h.members.contains(member)))))
                    .map(|place| (place.id, &place.name, place.owner, place.kind))
                    .collect::<Vec<_>>(),
                h.maintenance,
                universe.workshop_fee_accounts(),
                living_tier_tooltip(
                    &universe.households()[&id].living_standard,
                    universe.current_time_ms()
                )
            )
        })
        .unwrap_or_default();
    let record = state
        .selected
        .filter(|_| {
            state.tab == Tab::Households && state.history_view == household_history::View::History
        })
        .and_then(|id| {
            household_history::selected_record(universe, id, state.history_view, state.history_day)
        });
    let history_changed = state.history_record.as_ref() != record;
    if history_changed {
        state.history_record = record.cloned();
    }
    if let Some(id) = state.selected
        && state.history_day.is_some_and(|start| {
            universe
                .household_history(id)
                .is_none_or(|history| !history.completed.iter().any(|day| day.start_ms == start))
        })
    {
        state.history_day = None;
    }
    let guild_record = state
        .selected_guild
        .filter(|_| {
            state.tab == Tab::Guilds && state.guild_history_view == household_history::View::History
        })
        .and_then(|id| guild_history::selected_record(universe, id, state.guild_history_day));
    let guild_history_changed = state.guild_history_record.as_ref() != guild_record;
    if guild_history_changed {
        state.guild_history_record = guild_record.cloned();
    }
    if let Some(id) = state.selected_guild
        && state.guild_history_day.is_some_and(|start| {
            universe
                .guild_history(id)
                .is_none_or(|h| !h.completed.iter().any(|d| d.start_ms == start))
        })
    {
        state.guild_history_day = None;
    }
    let display_key = display_key + &selected_key;
    state.revision = Some(snapshot.0.revision);
    let changed =
        state.dirty || history_changed || guild_history_changed || state.display_key != display_key;
    state.display_key = display_key;
    if !changed && has_children {
        return;
    }
    state.dirty = false;
    commands
        .entity(entity)
        .despawn_children()
        .with_children(|parent| {
            if state.tab == Tab::Guilds {
                guild_content(parent, universe, state, offsets, show_list, show_details);
                return;
            }
            if state.tab == Tab::Businesses {
                label(parent, "Businesses are not implemented yet.");
                return;
            }
            if show_list {
                parent
                    .spawn((
                        Node {
                            width: if show_details { px(230) } else { percent(100) },
                            flex_shrink: 0.0,
                            min_height: px(0),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(8),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollPosition(offsets[0]),
                        RelativeCursorPosition::default(),
                        ScrollPanel(false),
                    ))
                    .with_children(|list| {
                        let mut households = universe.households().values().collect::<Vec<_>>();
                        households.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.0.cmp(&b.id.0)));
                        for household in households {
                            list.spawn((
                                Node::default(),
                                BackgroundColor(if state.selected == Some(household.id) {
                                    SELECTED
                                } else {
                                    PANEL
                                }),
                            ))
                            .with_children(|entry| {
                                navigation::link(
                                    entry,
                                    &household.name,
                                    Target::Household(household.id),
                                );
                            });
                        }
                        if universe.households().is_empty() {
                            label(list, "No households");
                        }
                    });
            }
            if !show_details {
                return;
            }
            parent
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        min_height: px(0),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition(offsets[1]),
                    RelativeCursorPosition::default(),
                    ScrollPanel(true),
                ))
                .with_children(|panel| {
                    if let Some(id) = state.selected {
                        household_history::controls(
                            panel,
                            universe,
                            id,
                            state.history_view,
                            state.history_day,
                        );
                        if state.history_view == household_history::View::Current {
                            details(panel, universe, id);
                        }
                        if state.history_view == household_history::View::History
                            && let Some(day) = &state.history_record
                        {
                            panel.spawn(text(&universe.households()[&id].name, 18.0, TEXT));
                            household_history::render(
                                panel,
                                universe,
                                day,
                                state.history_day.is_none(),
                            );
                        }
                    } else {
                        label(panel, "Select a household");
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_household_panes_keep_history_and_scroll_independent() {
        use crate::{floating_ui::Windows, simulation::Snapshot};
        let universe = crate::simulation::new_universe().unwrap();
        let [first, second]: [AgentId; 2] = universe
            .households()
            .keys()
            .copied()
            .take(2)
            .collect::<Vec<_>>()
            .try_into()
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
        .init_resource::<State>()
        .init_resource::<Windows>()
        .add_systems(
            Update,
            (handle, crate::inspectors::open_windows, refresh).chain(),
        );
        let mut windows = app.world_mut().resource_mut::<Windows>();
        windows.open(WindowKind::Household(first));
        windows.link(WindowKind::Household(first), WindowKind::Household(second));
        app.update();

        fn in_pane(world: &World, mut entity: Entity, pane: WindowKind) -> bool {
            while let Some(parent) = world.get::<ChildOf>(entity) {
                entity = parent.parent();
                if let Some(details) = world.get::<EntityDetails>(entity) {
                    return details.0 == pane;
                }
            }
            false
        }
        let world = app.world_mut();
        let first_history = world
            .query::<(Entity, &household_history::Choice)>()
            .iter(world)
            .find(|(entity, choice)| {
                matches!(
                    choice,
                    household_history::Choice::View(household_history::View::History)
                ) && in_pane(world, *entity, WindowKind::Household(first))
            })
            .unwrap()
            .0;
        let second_scroll = world
            .query::<(Entity, &ScrollPanel)>()
            .iter(world)
            .find(|(entity, panel)| {
                panel.0 && in_pane(world, *entity, WindowKind::Household(second))
            })
            .unwrap()
            .0;
        world
            .entity_mut(second_scroll)
            .insert(ScrollPosition(Vec2::new(0.0, 180.0)));
        world.entity_mut(first_history).insert(Interaction::Pressed);
        app.update();

        let world = app.world_mut();
        for (pane, state) in world.query::<(&EntityDetails, &DetailState)>().iter(world) {
            let (id, view) = if pane.0 == WindowKind::Household(first) {
                (first, household_history::View::History)
            } else {
                (second, household_history::View::Current)
            };
            assert_eq!(state.0.selected, Some(id));
            assert_eq!(state.0.history_view, view);
        }
        assert_eq!(
            world.get::<ScrollPosition>(second_scroll).unwrap().0.y,
            180.0
        );
    }

    #[test]
    fn living_tier_tooltip_marks_current_tier_and_shows_assessment_schedule() {
        let universe = crate::simulation::new_universe().unwrap();
        let household = *universe.households().keys().next().unwrap();
        let tip = living_tier_tooltip(
            &universe.households()[&household].living_standard,
            universe.current_time_ms(),
        );
        assert!(tip.contains("Last assessment: none"));
        assert!(tip.contains("Next assessment:"));
        assert!(tip.contains("> Tier 1: Hunger, thirst, sleep, discomfort"));
        assert!(!tip.contains("Promotion allowed:"));
        let universe = universe
            .set_household_living_tier(household, 4)
            .unwrap()
            .set_household_living_tier(household, 3)
            .unwrap();
        let tip = living_tier_tooltip(
            &universe.households()[&household].living_standard,
            universe.current_time_ms(),
        );
        assert!(tip.contains("> Tier 3: + meat, carbohydrates, fibre"));
        assert!(tip.contains("Promotion allowed:"));
        assert_eq!(tip.matches("Tier ").count(), 5);
        let mut standard = universe.households()[&household].living_standard.clone();
        standard.last_check = Some(
            learning_lord_simulation::living_standards::LivingStandardCheck {
                time_ms: 60_000,
                income: 0,
                expenditure: 1,
                result: learning_lord_simulation::living_standards::LivingStandardResult::Demoted,
            },
        );
        assert!(
            living_tier_tooltip(&standard, 60_000).contains("Last assessment: 00:01:00 - demoted")
        );
    }

    #[test]
    fn guild_details_keep_owners_separate_and_reset_selection() {
        let worker = crate::simulation::SimulationWorker::spawn(
            crate::simulation::new_universe().unwrap(),
            60.try_into().unwrap(),
        );
        let snapshot = worker.snapshot();
        let guild = snapshot
            .universe
            .guilds()
            .values()
            .find(|guild| guild.name == "Weavers’ Guild")
            .unwrap();
        let id = guild.id;
        let members = guild.members.clone();
        let mut state = State::default();
        state.select_guild(id);
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(snapshot))
            .insert_resource(state)
            .add_systems(Update, refresh);
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        let labels = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Weavers’ Guild",
            "Guild coins: 0",
            "1 coin per production hour",
            "Eira",
            "Jori",
            "INDIVIDUALLY OWNED STOCKS",
        ] {
            assert!(labels.contains(expected), "missing {expected}: {labels}");
        }
        let links = app
            .world_mut()
            .query::<&Target>()
            .iter(app.world())
            .copied()
            .collect::<Vec<_>>();
        assert!(links.contains(&Target::Guild(id)));
        for member in members {
            assert!(links.contains(&Target::Citizen(member)));
        }
        let old = app
            .world_mut()
            .query::<Entity>()
            .iter(app.world())
            .collect::<Vec<_>>();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(
            old,
            app.world_mut()
                .query::<Entity>()
                .iter(app.world())
                .collect::<Vec<_>>()
        );
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert_eq!(app.world().resource::<State>().tab, Tab::Households);
    }

    #[test]
    fn guild_history_navigation_preserves_idle_entities_and_resets_on_load() {
        let worker = crate::simulation::SimulationWorker::spawn(
            crate::simulation::new_universe().unwrap(),
            60.try_into().unwrap(),
        );
        let snapshot = worker.snapshot();
        let id = *snapshot.universe.guilds().keys().next().unwrap();
        let mut state = State::default();
        state.select_guild(id);
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(snapshot))
            .insert_resource(state)
            .add_systems(Update, (handle, refresh).chain());
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        let choice = app
            .world_mut()
            .query::<(Entity, &guild_history::Choice)>()
            .iter(app.world())
            .find(|(_, choice)| {
                matches!(
                    choice,
                    guild_history::Choice::View(household_history::View::History)
                )
            })
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(choice)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world().resource::<State>().guild_history_view,
            household_history::View::History
        );
        let labels = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "CURRENT DAY · partial totals",
            "Fees accrued 0.000",
            "Payments received 0",
            "Expenses paid 0",
            "No completed days yet.",
        ] {
            assert!(labels.contains(expected), "missing {expected}: {labels}");
        }
        let old = app
            .world_mut()
            .query::<Entity>()
            .iter(app.world())
            .collect::<Vec<_>>();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(
            old,
            app.world_mut()
                .query::<Entity>()
                .iter(app.world())
                .collect::<Vec<_>>()
        );
        app.world_mut().resource_mut::<State>().guild_history_day = Some(999);
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(app.world().resource::<State>().guild_history_day, None);
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.tab, Tab::Households);
        assert_eq!(state.guild_history_view, household_history::View::Current);
        assert_eq!(state.guild_history_day, None);
    }

    #[test]
    fn daily_history_shows_partial_totals_and_preserves_idle_entities() {
        let worker = crate::simulation::SimulationWorker::spawn(
            crate::simulation::new_universe().unwrap(),
            60.try_into().unwrap(),
        );
        let snapshot = worker.snapshot();
        let id = *snapshot
            .universe
            .households()
            .iter()
            .filter(|(_, household)| household.members.len() == 2)
            .map(|(id, _)| id)
            .min_by_key(|id| id.0)
            .unwrap();
        let mut state = State::default();
        state.select(id);
        state.history_view = household_history::View::History;
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(snapshot))
            .insert_resource(state)
            .add_systems(Update, refresh);
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        let labels = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "CURRENT DAY · partial totals",
            "Opening 3000",
            "New member funds: 3000 coins",
            "Sales receipts 0",
            "Taxes paid 0",
            "CURRENT SUPPLIES",
            "available",
            "No completed days yet.",
        ] {
            assert!(labels.contains(expected), "missing {expected}: {labels}");
        }
        let old = app
            .world_mut()
            .query::<Entity>()
            .iter(app.world())
            .collect::<Vec<_>>();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(
            old,
            app.world_mut()
                .query::<Entity>()
                .iter(app.world())
                .collect::<Vec<_>>()
        );
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.history_view, household_history::View::Current);
        assert_eq!(state.history_day, None);
    }

    #[test]
    fn household_details_show_shared_assets_and_actual_workplace_owners() {
        let worker = crate::simulation::SimulationWorker::spawn(
            crate::simulation::new_universe().unwrap(),
            60.try_into().unwrap(),
        );
        let snapshot = worker.snapshot();
        let id = snapshot
            .universe
            .households()
            .values()
            .find(|h| {
                h.members
                    .iter()
                    .any(|id| snapshot.universe.agents()[id].name == "Ada")
            })
            .unwrap()
            .id;
        let household = &snapshot.universe.households()[&id];
        let stored = snapshot.universe.storage().units(
            household.home.unwrap(),
            learning_lord_simulation::storage::GoodsOwner::Household(id),
            learning_lord_simulation::marketplace::Good::BreadRoll,
        );
        let consumed: u64 = household
            .members
            .iter()
            .map(|member| {
                snapshot
                    .universe
                    .citizen_history(*member)
                    .unwrap()
                    .current
                    .consumed[learning_lord_simulation::marketplace::Good::BreadRoll as usize]
            })
            .sum();
        assert_eq!(stored + consumed, 140);
        let stock_label = format!("{stored} rolls stored");
        let mut app = App::new();
        let mut state = State::default();
        state.select(id);
        app.insert_resource(DisplaySnapshot(snapshot))
            .insert_resource(state)
            .add_systems(Update, refresh);
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        let labels = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Ada",
            "Iris",
            "Ada and Iris's home",
            "Shared coins: 6000",
            "Bread rolls",
            stock_label.as_str(),
            "Ada and Iris's field",
            "Owner:",
            "MAINTENANCE",
            "Committed worker:",
        ] {
            assert!(labels.contains(expected), "missing {expected}: {labels}");
        }
        let links = app
            .world_mut()
            .query::<&Target>()
            .iter(app.world())
            .copied()
            .collect::<Vec<_>>();
        assert!(
            links
                .iter()
                .any(|target| matches!(target, Target::Citizen(_)))
        );
        assert!(
            links
                .iter()
                .any(|target| matches!(target, Target::Location(_)))
        );
        assert!(links.iter().any(|target| matches!(target, Target::Good(_))));
        let old = app
            .world_mut()
            .query::<Entity>()
            .iter(app.world())
            .collect::<Vec<_>>();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(
            old,
            app.world_mut()
                .query::<Entity>()
                .iter(app.world())
                .collect::<Vec<_>>()
        );
    }
}
