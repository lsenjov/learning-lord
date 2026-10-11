use crate::{
    DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, citizens, locations,
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
    generation: u64,
    revision: Option<u64>,
    dirty: bool,
    display_key: String,
}
impl State {
    pub(crate) fn selected(&self) -> Option<AgentId> {
        self.selected
    }
    pub(crate) fn select(&mut self, id: AgentId) {
        if self.tab != Tab::Households || self.selected != Some(id) {
            self.tab = Tab::Households;
            self.selected = Some(id);
            self.dirty = true;
        }
    }
}
#[derive(Component)]
pub(crate) struct Content;
#[derive(Component)]
pub(crate) struct Membership;
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
    parent.spawn((
        Content,
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
    mut state: ResMut<State>,
    choices: Query<(&Interaction, &Choose), Changed<Interaction>>,
) {
    for (interaction, choice) in &choices {
        if *interaction == Interaction::Pressed && state.tab != choice.0 {
            state.tab = choice.0;
            state.dirty = true;
        }
    }
}

fn row(parent: &mut ChildSpawnerCommands, children: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(6),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(children);
}
fn label(parent: &mut ChildSpawnerCommands, value: impl Into<String>) {
    parent.spawn(text(value, 14.0, TEXT));
}
fn place_link(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    id: learning_lord_simulation::locations::PlaceId,
) {
    if let Ok(place) = universe.map().place(id) {
        navigation::link(parent, &place.name, Target::Location(id));
    }
}
fn details(parent: &mut ChildSpawnerCommands, universe: &Universe, id: AgentId) {
    let household = &universe.households()[&id];
    parent.spawn(text(&household.name, 18.0, TEXT));
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
        place_link(row, universe, household.home);
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
    parent.spawn(text("ACCESSIBLE WORKPLACES", 14.0, MUTED));
    let map = universe.map();
    let mut workplaces = map
        .places()
        .values()
        .filter(|place| {
            place.kind != Location::Home
                && place
                    .owner
                    .is_some_and(|owner| household.members.contains(&owner))
        })
        .collect::<Vec<_>>();
    workplaces.sort_by_key(|place| place.id.0);
    for place in &workplaces {
        row(parent, |row| {
            place_link(row, universe, place.id);
            label(row, "| Owner:");
            if let Some(owner) = place.owner {
                navigation::link(
                    row,
                    locations::name(universe, owner),
                    Target::Citizen(owner),
                );
            }
        });
    }
    if workplaces.is_empty() {
        label(parent, "None");
    }
    parent.spawn(text("MAINTENANCE (6-hour cooldown)", 14.0, MUTED));
    for (name, task) in [
        ("Replenishment", &household.maintenance.replenishment),
        ("Listing excess", &household.maintenance.listing),
    ] {
        row(parent, |row| {
            label(
                row,
                format!(
                    "{name}: {} cooldown remaining | Committed worker:",
                    citizens::duration(task.remaining_ms(universe.current_time_ms()))
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

pub(crate) fn refresh(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    mut state: ResMut<State>,
    content: Query<(Entity, Option<&Children>), With<Content>>,
    mut tabs: Query<(&Choose, &mut BackgroundColor)>,
    scrolls: Query<(&ScrollPanel, &ScrollPosition)>,
    windows: Option<Res<crate::floating_ui::Windows>>,
) {
    if state.generation != snapshot.0.generation {
        *state = State {
            generation: snapshot.0.generation,
            dirty: true,
            ..default()
        };
    }
    if content.is_empty()
        || windows
            .as_ref()
            .is_some_and(|windows| !windows.is_open(crate::floating_ui::WindowKind::Collectives))
    {
        return;
    }
    if !state.dirty
        && state.revision == Some(snapshot.0.revision)
        && content.iter().all(|(_, children)| children.is_some())
    {
        return;
    }
    let universe = &snapshot.0.universe;
    if state
        .selected
        .is_none_or(|id| !universe.households().contains_key(&id))
    {
        state.selected = universe.households().keys().copied().min_by_key(|id| id.0);
    }
    let display_key = format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        state.tab,
        state.selected,
        universe.households(),
        universe.storage().stock(),
        universe.market().orders().collect::<Vec<_>>()
    );
    let selected_key = state
        .selected
        .map(|id| {
            let h = &universe.households()[&id];
            format!(
                "{:?}|{:?}|{}|{}|{:?}|{:?}",
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
                citizens::duration(
                    h.maintenance
                        .replenishment
                        .remaining_ms(universe.current_time_ms())
                ),
                citizens::duration(
                    h.maintenance
                        .listing
                        .remaining_ms(universe.current_time_ms())
                ),
                universe
                    .map()
                    .places()
                    .values()
                    .filter(|place| place.owner.is_some_and(|owner| h.members.contains(&owner)))
                    .map(|place| (place.id, &place.name, place.owner, place.kind))
                    .collect::<Vec<_>>(),
                h.maintenance
            )
        })
        .unwrap_or_default();
    let display_key = display_key + &selected_key;
    state.revision = Some(snapshot.0.revision);
    let changed = state.dirty || state.display_key != display_key;
    let mut offsets = [Vec2::ZERO; 2];
    for (panel, position) in &scrolls {
        offsets[usize::from(panel.0)] = position.0;
    }
    state.display_key = display_key;
    for (tab, mut color) in &mut tabs {
        color.set_if_neq(BackgroundColor(if tab.0 == state.tab {
            SELECTED
        } else {
            PANEL
        }));
    }
    if !changed && content.iter().all(|(_, children)| children.is_some()) {
        return;
    }
    state.dirty = false;
    state.revision = Some(snapshot.0.revision);
    for (entity, _) in &content {
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|parent| {
                if state.tab != Tab::Households {
                    label(
                        parent,
                        match state.tab {
                            Tab::Guilds => "Guilds are not implemented yet.",
                            _ => "Businesses are not implemented yet.",
                        },
                    );
                    return;
                }
                parent
                    .spawn((
                        Node {
                            width: px(230),
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
                            navigation::link(
                                list,
                                &household.name,
                                Target::Household(household.id),
                            );
                        }
                        if universe.households().is_empty() {
                            label(list, "No households");
                        }
                    });
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
                            details(panel, universe, id);
                        } else {
                            label(panel, "Select a household");
                        }
                    });
            });
    }
}

pub(crate) fn refresh_membership(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<citizens::Selection>,
    panels: Query<(Entity, Option<&Children>), With<Membership>>,
) {
    if !snapshot.is_changed()
        && !selection.is_changed()
        && panels.iter().all(|(_, children)| children.is_some())
    {
        return;
    }
    for (entity, _) in &panels {
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|parent| {
                if let Some(agent) = citizens::selected_agent(&snapshot.0.universe, selection.0) {
                    let learning_lord_simulation::AgentKind::Citizen(citizen) = &agent.kind;
                    if let Some(id) = citizen.household_id() {
                        label(parent, "Household:");
                        navigation::link(
                            parent,
                            &snapshot.0.universe.households()[&id].name,
                            Target::Household(id),
                        );
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            "Ada's home",
            "Shared coins: 6000",
            "Bread rolls",
            "140 rolls stored",
            "Ada's field",
            "Owner:",
            "MAINTENANCE (6-hour cooldown)",
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
