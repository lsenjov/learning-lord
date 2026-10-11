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
    Guild(AgentId),
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
            Target::Household(id) => {
                collectives.tab() != collectives::Tab::Households
                    || collectives.selected() != Some(id)
            }
            Target::Guild(id) => {
                collectives.tab() != collectives::Tab::Guilds
                    || collectives.selected_guild() != Some(id)
            }
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
            Target::Guild(id) if universe.guilds().contains_key(&id) => {
                collectives.select_guild(id);
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fragment {
    pub(crate) label: String,
    pub(crate) target: Option<Target>,
}
impl Fragment {
    pub(crate) fn text(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            target: None,
        }
    }
    pub(crate) fn entity(label: impl Into<String>, target: Target) -> Self {
        Self {
            label: label.into(),
            target: Some(target),
        }
    }
    pub(crate) fn good(good: Good) -> Self {
        Self::entity(good.name(), Target::Good(good))
    }
}
pub(crate) fn plain(fragments: &[Fragment]) -> String {
    fragments.iter().map(|part| part.label.as_str()).collect()
}
pub(crate) fn render(
    parent: &mut ChildSpawnerCommands,
    fragments: &[Fragment],
    size: f32,
    color: Color,
) {
    parent
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(3),
            flex_shrink: 0.0,
            align_items: AlignItems::Baseline,
            ..default()
        })
        .with_children(|row| {
            for fragment in fragments {
                if let Some(target) = fragment.target {
                    row.spawn((
                        Button,
                        target,
                        Node {
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ))
                    .with_children(|button| {
                        button.spawn((
                            text(&fragment.label, size, Color::srgb(0.40, 0.80, 0.91)),
                            Underline,
                        ));
                    });
                } else {
                    row.spawn(text(&fragment.label, size, color));
                }
            }
        });
}
pub(crate) fn account(universe: &learning_lord_simulation::Universe, id: AgentId) -> Fragment {
    Fragment::entity(
        crate::locations::name(universe, id),
        if universe.guilds().contains_key(&id) {
            Target::Guild(id)
        } else if universe.households().contains_key(&id) {
            Target::Household(id)
        } else {
            Target::Citizen(id)
        },
    )
}
pub(crate) fn place(citizen: &learning_lord_simulation::Citizen, id: PlaceId) -> Fragment {
    Fragment::entity(
        citizen
            .map()
            .place(id)
            .map_or_else(|_| "Unknown location".into(), |place| place.name.clone()),
        Target::Location(id),
    )
}
pub(crate) fn shopping(list: learning_lord_simulation::marketplace::ShoppingList) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    for good in Good::ALL {
        let units = list.units(good);
        if units == 0 {
            continue;
        }
        if !fragments.is_empty() {
            fragments.push(Fragment::text(", "));
        }
        fragments.push(Fragment::text(format!(
            "{} ",
            crate::quantity_label(good, units)
        )));
        fragments.push(Fragment::good(good));
    }
    if fragments.is_empty() {
        fragments.push(Fragment::text("nothing"));
    }
    fragments
}
pub(crate) fn action(
    action: learning_lord_simulation::CitizenAction,
    citizen: &learning_lord_simulation::Citizen,
) -> Vec<Fragment> {
    use learning_lord_simulation::CitizenAction as A;
    match action {
        A::BuyFood(good) => vec![Fragment::text("Buy "), Fragment::good(good)],
        A::List(good, units) | A::Withdraw(good, units) => vec![
            Fragment::text(format!(
                "{} {} ",
                if matches!(action, A::List(..)) {
                    "List"
                } else {
                    "Withdraw"
                },
                crate::quantity_label(good, units)
            )),
            Fragment::good(good),
        ],
        A::Buy(list) => {
            let mut row = vec![Fragment::text("Buy ")];
            row.extend(shopping(list));
            row
        }
        A::BuyAt { place: id, list }
        | A::CollectStored { place: id, list }
        | A::DepositStored { place: id, list } => {
            let mut row = vec![Fragment::text(match action {
                A::BuyAt { .. } => "Buy ",
                A::CollectStored { .. } => "Collect ",
                _ => "Deposit ",
            })];
            row.extend(shopping(list));
            row.push(Fragment::text(
                if matches!(action, A::CollectStored { .. }) {
                    " from "
                } else {
                    " at "
                },
            ));
            row.push(place(citizen, id));
            row
        }
        A::Travel(id) => vec![Fragment::text("Travel to "), place(citizen, id)],
        A::ProduceAt { recipe, place: id } => vec![
            Fragment::text(format!("{} at ", recipe.name())),
            place(citizen, id),
        ],
        A::Plant { field, plot, crop } => vec![
            Fragment::text("Plant "),
            Fragment::entity(crop.name(), Target::Good(crop.good())),
            Fragment::text(" at "),
            place(citizen, field),
            Fragment::text(format!(", plot {}", plot + 1)),
        ],
        A::Tend { field, plot, .. } | A::Harvest { field, plot } | A::Clear { field, plot } => {
            let verb = match action {
                A::Tend { .. } => "Tend",
                A::Harvest { .. } => "Harvest",
                _ => "Clear",
            };
            let name = crate::crop_label(citizen, field, plot);
            let crop = citizen
                .map()
                .place(field)
                .ok()
                .and_then(|p| p.plots.get(plot).copied().flatten())
                .map(|crop| crop.kind)
                .or_else(|| {
                    citizen.active_plan().and_then(|active| {
                        active
                            .plan()
                            .actions()
                            .iter()
                            .find_map(|action| match action {
                                A::Plant {
                                    field: target,
                                    plot: target_plot,
                                    crop,
                                } if *target == field && *target_plot == plot => Some(*crop),
                                _ => None,
                            })
                    })
                });
            let crop_fragment = crop.map_or_else(
                || Fragment::text(name),
                |crop| Fragment::entity(name, Target::Good(crop.good())),
            );
            let mut row = vec![
                Fragment::text(format!("{verb} ")),
                crop_fragment,
                Fragment::text(" at "),
                place(citizen, field),
                Fragment::text(format!(", plot {}", plot + 1)),
            ];
            if let A::Tend { stage, .. } = action {
                row.push(Fragment::text(format!(" (stage {})", stage + 1)));
            }
            row
        }
        A::CollectBeer { brewery, slot } => vec![
            Fragment::text("Collect "),
            Fragment::good(Good::Beer),
            Fragment::text(" at "),
            place(citizen, brewery),
            Fragment::text(format!(" (slot {})", slot + 1)),
        ],
        A::Eat => vec![Fragment::text("Eat")],
        A::Drink => vec![Fragment::text("Drink")],
        A::Wait => vec![Fragment::text("Wait")],
        A::Sleep => vec![Fragment::text("Sleep")],
        A::EquipClothing => vec![Fragment::text("Equip clothing")],
        A::ListExcess => vec![Fragment::text("List excess goods")],
        A::Produce(recipe) => vec![Fragment::text(
            if recipe == learning_lord_simulation::production::Recipe::Forage {
                "Forage"
            } else {
                recipe.name()
            },
        )],
    }
}
pub(crate) fn active_action(
    active: &learning_lord_simulation::ActiveAction,
    citizen: &learning_lord_simulation::Citizen,
) -> Vec<Fragment> {
    use learning_lord_simulation::{CitizenAction as A, production::Recipe};
    let label = match active.action() {
        A::Eat if active.drinking_units() > 0 => Some("Eating & drinking"),
        A::Eat => Some("Eating"),
        A::Drink => Some("Drinking"),
        A::Wait => Some("Waiting"),
        A::Sleep => Some("Sleeping"),
        A::EquipClothing => Some("Equipping clothing"),
        A::Produce(Recipe::Forage)
        | A::ProduceAt {
            recipe: Recipe::Forage,
            ..
        } => Some("Foraging"),
        _ => None,
    };
    label.map_or_else(
        || action(active.action(), citizen),
        |label| {
            let mut row = vec![Fragment::text(label)];
            if let A::ProduceAt { place: id, .. } = active.action() {
                row.push(Fragment::text(" at "));
                row.push(place(citizen, id));
            }
            row
        },
    )
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
        let guild = *universe.guilds().keys().next().unwrap();
        let household = *universe.households().keys().next().unwrap();
        let member = universe.households()[&household].members[0];
        let home = universe.households()[&household].home;
        click(&mut app, Target::Citizen(member));
        click(&mut app, Target::Household(household));
        assert_eq!(
            app.world().resource::<collectives::State>().selected(),
            Some(household)
        );
        click(&mut app, Target::Guild(guild));
        assert_eq!(
            app.world()
                .resource::<collectives::State>()
                .selected_guild(),
            Some(guild)
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
    fn collective_tab_links_reset_detail_scroll_for_already_selected_entities() {
        let mut app = app();
        let universe = &app.world().resource::<DisplaySnapshot>().0.universe;
        let household = *universe.households().keys().next().unwrap();
        let guild = *universe.guilds().keys().next().unwrap();
        click(&mut app, Target::Household(household));
        for target in [Target::Guild(guild), Target::Household(household)] {
            let mut scrolls = app
                .world_mut()
                .query::<(&collectives::ScrollPanel, &mut ScrollPosition)>();
            for (panel, mut scroll) in scrolls.iter_mut(app.world_mut()) {
                if panel.0 {
                    scroll.0 = Vec2::new(0.0, 100.0);
                }
            }
            click(&mut app, target);
            let mut scrolls = app
                .world_mut()
                .query::<(&collectives::ScrollPanel, &ScrollPosition)>();
            assert!(
                scrolls
                    .iter(app.world())
                    .filter(|(panel, _)| panel.0)
                    .all(|(_, scroll)| scroll.0 == Vec2::ZERO)
            );
        }
    }

    #[test]
    fn action_fragments_link_each_good_and_the_explicit_site() {
        use learning_lord_simulation::{CitizenAction, marketplace::ShoppingList};
        let universe = crate::simulation::new_universe().unwrap();
        let learning_lord_simulation::AgentKind::Citizen(citizen) =
            &universe.agents().values().next().unwrap().kind;
        let site = citizen.home();
        let row = action(
            CitizenAction::BuyAt {
                place: site,
                list: ShoppingList::new([(Good::Wheat, 25), (Good::Water, 50)]).unwrap(),
            },
            citizen,
        );
        let targets = row
            .iter()
            .filter_map(|fragment| fragment.target)
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 3);
        assert!(targets.contains(&Target::Good(Good::Wheat)));
        assert!(targets.contains(&Target::Good(Good::Water)));
        assert!(targets.contains(&Target::Location(site)));
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
