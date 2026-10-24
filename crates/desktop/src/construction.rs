use crate::{DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, market, text};
use bevy::prelude::*;
use learning_lord_simulation::{
    AgentId, Universe,
    construction::{
        ConstructionItem, ConstructionProject, ContractId, ContractStatus, HOUR_MS, ProjectId,
    },
    locations::{Building, BuildingId, Place, PlaceId, RoomId, RoomSize},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Destination {
    pub place: PlaceId,
    pub building: BuildingId,
    pub room: Option<RoomId>,
    pub owner: AgentId,
}

#[derive(Resource, Default)]
pub(crate) struct State {
    expanded: Option<Destination>,
    generation: u64,
    pending: Option<u64>,
    error: Option<String>,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    Toggle(Destination),
    Request(Destination, ConstructionItem),
    Continue(ProjectId),
    Cancel(ContractId),
}

fn button(
    parent: &mut ChildSpawnerCommands,
    label: impl Into<String>,
    choice: Choice,
    reason: Option<String>,
    tooltip: String,
    selected: bool,
) {
    let enabled = reason.is_none();
    let mut entity = parent.spawn((
        Node {
            padding: UiRect::axes(px(7), px(3)),
            border_radius: BorderRadius::all(px(5)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(if selected { SELECTED } else { PANEL }),
        Interaction::None,
        crate::tooltip::Tooltip(
            reason.map_or(tooltip.clone(), |reason| format!("{reason}\n{tooltip}")),
        ),
    ));
    if enabled {
        entity.insert((Button, choice));
    }
    entity.with_children(|row| {
        row.spawn(text(label, 13.0, if enabled { TEXT } else { MUTED }));
    });
}

fn budget(
    universe: &Universe,
    owner: AgentId,
) -> Result<learning_lord_simulation::construction_finance::HouseholdBudget, String> {
    if universe.guilds().contains_key(&owner) {
        return Err("Construction currently requires a household owner".into());
    }
    universe
        .construction_budget(owner)
        .map_err(|error| error.to_string())
}

fn budget_tip(universe: &Universe, owner: AgentId) -> String {
    match budget(universe, owner) {
        Ok(budget) => format!(
            "Maximum offer: {} coins\nCash: {} | Liabilities: {} | Commitments: {}\nSeven-day essential reserve: {} coins",
            budget.maximum_offer,
            budget.cash,
            budget.liabilities,
            budget.commitments,
            budget.essential_reserve
        ),
        Err(error) => error,
    }
}

fn unavailable(
    universe: &Universe,
    destination: Destination,
    item: ConstructionItem,
) -> Option<String> {
    let budget = match budget(universe, destination.owner) {
        Ok(budget) => budget,
        Err(error) => return Some(error),
    };
    if budget.maximum_offer <= 0 {
        return Some(
            "No cash available above commitments and the seven-day essential reserve".into(),
        );
    }
    if universe.construction().contracts.values().any(|contract| {
        let project = &universe.construction().projects[&contract.project];
        contract.status == ContractStatus::NoBids
            && project.client == budget.account
            && project.place == destination.place
            && project.building == destination.building
            && project.room == destination.room
            && project.item == item
            && budget.maximum_offer <= contract.maximum_offer
    }) {
        return Some("Reposting after no bids requires a higher available budget".into());
    }
    if let Err(error) = universe.validate_construction_destination(
        destination.owner,
        destination.place,
        destination.building,
        destination.room,
        item,
    ) {
        return Some(match error {
            learning_lord_simulation::SimulationError::PrivateProperty => {
                "Room expansion requires ownership of both building and plot".into()
            }
            learning_lord_simulation::SimulationError::MissingProperty => {
                "Insufficient unreserved area".into()
            }
            error => error.to_string(),
        });
    }
    None
}

pub(crate) fn render_choices(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    place: &Place,
    building: &Building,
    room: Option<RoomId>,
    state: &State,
) {
    let destination = Destination {
        place: place.id,
        building: building.id,
        room,
        owner: building.owner,
    };
    let selected = state.expanded == Some(destination);
    let label = if room.is_some() {
        "Add object"
    } else {
        "Add room"
    };
    button(
        parent,
        if selected {
            format!("{label} ^")
        } else {
            format!("{label} v")
        },
        Choice::Toggle(destination),
        state
            .pending
            .map(|_| "Waiting for construction request".into())
            .or_else(|| budget(universe, destination.owner).err()),
        budget_tip(universe, destination.owner),
        selected,
    );
}

pub(crate) fn render_menu(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    place: &Place,
    building: &Building,
    room: Option<RoomId>,
    state: &State,
) {
    let destination = Destination {
        place: place.id,
        building: building.id,
        room,
        owner: building.owner,
    };
    if state.expanded != Some(destination) {
        return;
    }
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            padding: UiRect::all(px(4)),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|menu| {
            let items = if room.is_some() {
                ConstructionItem::OBJECTS.to_vec()
            } else {
                [RoomSize::Small, RoomSize::Medium, RoomSize::Large]
                    .map(ConstructionItem::Room)
                    .to_vec()
            };
            for item in items {
                let tooltip = format!(
                    "{} m2 | {} base labour hours | {} g wood\n{}",
                    item.area_m2(),
                    item.base_hours(),
                    item.wood(),
                    budget_tip(universe, destination.owner)
                );
                button(
                    menu,
                    format!("{} | {} m2", item.name(), item.area_m2()),
                    Choice::Request(destination, item),
                    state
                        .pending
                        .map(|_| "Waiting for construction request".into())
                        .or_else(|| unavailable(universe, destination, item)),
                    tooltip,
                    false,
                );
            }
        });
}

fn latest_contract<'a>(
    universe: &'a Universe,
    project: &ConstructionProject,
) -> Option<&'a learning_lord_simulation::construction::ConstructionContract> {
    universe
        .construction()
        .contracts
        .values()
        .filter(|contract| contract.project == project.id)
        .max_by_key(|contract| contract.sequence)
}

fn continuation_reason(universe: &Universe, project: &ConstructionProject) -> Option<String> {
    if let Err(error) = universe.validate_construction_continuation(project.client, project.id) {
        return Some(error.to_string());
    }
    let available = match budget(universe, project.client) {
        Ok(budget) => budget.maximum_offer,
        Err(error) => return Some(error),
    };
    if available <= 0 {
        return Some(
            "No cash available above commitments and the seven-day essential reserve".into(),
        );
    }
    if universe
        .construction()
        .contracts
        .values()
        .filter(|contract| {
            contract.project == project.id && contract.status == ContractStatus::NoBids
        })
        .any(|contract| available <= contract.maximum_offer)
    {
        return Some("Reposting after no bids requires a higher available budget".into());
    }
    None
}

pub(crate) fn render_projects(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    place: PlaceId,
    building: BuildingId,
    room: Option<RoomId>,
    state: &State,
) {
    let mut projects: Vec<_> = universe
        .construction()
        .projects
        .values()
        .filter(|project| {
            project.place == place
                && project.building == building
                && project.room == room
                && !project.completed
        })
        .collect();
    projects.sort_by_key(|project| project.id.0);
    for project in projects {
        let contract = latest_contract(universe, project);
        let status = match contract.map(|contract| contract.status) {
            Some(ContractStatus::Bidding) => {
                format!("Bidding | {} bids", contract.unwrap().quotes.len())
            }
            Some(ContractStatus::Active) => "Building".into(),
            Some(ContractStatus::NoBids) => "No bids".into(),
            Some(ContractStatus::Cancelled) => "Cancelled".into(),
            _ => "Unfinished".into(),
        };
        let mut tooltip = format!(
            "Progress: {:.1}/{} base labour hours\nInstalled wood: {}/{} g\nMissing wood: {} g\n{}",
            project.completed_base_ms as f64 / HOUR_MS as f64,
            project.item.base_hours(),
            project.installed_wood,
            project.item.wood(),
            project.missing_wood(),
            budget_tip(universe, project.client)
        );
        if let Some(contract) = contract {
            tooltip.push_str(&format!(
                "\nOffer: {} | Price: {} | Paid: {} | Escrow: {} coins",
                contract.maximum_offer, contract.price, contract.paid, contract.escrow
            ));
            if contract.status == ContractStatus::Bidding {
                tooltip.push_str(&format!(
                    "\nBidding ends: {}",
                    crate::format_clock(contract.bidding_ends_ms)
                ));
            }
            if let Some(deadline) = contract.deadline_ms {
                tooltip.push_str(&format!("\nDeadline: {}", crate::format_clock(deadline)));
            }
            if let Some(carpenter) = contract.carpenter {
                tooltip.push_str(&format!(
                    "\nCarpenter: {}",
                    universe
                        .agents()
                        .get(&carpenter)
                        .map(|agent| agent.name.as_str())
                        .unwrap_or("Unknown")
                ));
            }
            for quote in &contract.quotes {
                tooltip.push_str(&format!(
                    "\n{}: {} coins",
                    universe
                        .agents()
                        .get(&quote.carpenter)
                        .map(|agent| agent.name.as_str())
                        .unwrap_or("Unknown"),
                    quote.price
                ));
            }
        }
        parent
            .spawn((
                Node {
                    column_gap: px(6),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    flex_shrink: 0.0,
                    ..default()
                },
                Interaction::None,
                crate::tooltip::Tooltip(tooltip.clone()),
            ))
            .with_children(|row| {
                let percent = project.completed_base_ms.saturating_mul(100)
                    / (project.item.base_hours() * HOUR_MS);
                row.spawn(text(
                    format!("{} | {status} | {percent}%", project.item.name()),
                    13.0,
                    MUTED,
                ));
                if let Some(contract) =
                    contract.filter(|contract| contract.status == ContractStatus::Bidding)
                {
                    button(
                        row,
                        "Cancel",
                        Choice::Cancel(contract.id),
                        state
                            .pending
                            .map(|_| "Waiting for construction request".into()),
                        "Cancel bidding and release reserved cash and unused space".into(),
                        false,
                    );
                } else if !contract.is_some_and(|contract| contract.status.open()) {
                    button(
                        row,
                        "Continue",
                        Choice::Continue(project.id),
                        state
                            .pending
                            .map(|_| "Waiting for construction request".into())
                            .or_else(|| continuation_reason(universe, project)),
                        tooltip,
                        false,
                    );
                }
            });
    }
}

impl State {
    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }
}

pub(crate) fn handle(
    mut state: ResMut<State>,
    mut locations: ResMut<crate::locations::State>,
    (snapshot, worker): (
        Res<DisplaySnapshot>,
        Res<crate::simulation::SimulationWorker>,
    ),
    selection: Res<market::Selection>,
    buttons: Query<(&Interaction, &Choice), Changed<Interaction>>,
    controls: Option<Res<crate::Controls>>,
    caravan: Option<Res<crate::caravan_controls::State>>,
) {
    if state.generation != snapshot.0.generation {
        *state = State {
            generation: snapshot.0.generation,
            ..default()
        };
        locations.invalidate();
        return;
    }
    if state
        .pending
        .is_some_and(|revision| snapshot.0.mutation_revision > revision)
    {
        state.pending = None;
        state.error.clone_from(&snapshot.0.mutation_error);
        locations.invalidate();
    }
    if selection.view != market::View::Locations
        || state.pending()
        || locations.pending()
        || caravan.is_some_and(|state| state.pending())
        || controls.is_some_and(|controls| controls.restart_pending)
    {
        return;
    }
    let universe = &snapshot.0.universe;
    for (interaction, choice) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.error = None;
        locations.invalidate();
        let mutation = match *choice {
            Choice::Toggle(destination) => {
                state.expanded = (state.expanded != Some(destination)).then_some(destination);
                continue;
            }
            Choice::Request(destination, item) => {
                if let Some(reason) = unavailable(universe, destination, item) {
                    state.error = Some(reason);
                    continue;
                }
                let maximum_offer = budget(universe, destination.owner).unwrap().maximum_offer;
                crate::simulation::Mutation::RequestConstruction {
                    client: destination.owner,
                    place: destination.place,
                    building: destination.building,
                    room: destination.room,
                    item,
                    maximum_offer,
                }
            }
            Choice::Continue(project_id) => {
                let Some(project) = universe.construction().projects.get(&project_id) else {
                    continue;
                };
                if let Some(reason) = continuation_reason(universe, project) {
                    state.error = Some(reason);
                    continue;
                }
                let maximum_offer = budget(universe, project.client).unwrap().maximum_offer;
                crate::simulation::Mutation::ContinueConstruction {
                    client: project.client,
                    project: project_id,
                    maximum_offer,
                }
            }
            Choice::Cancel(contract_id) => {
                let Some(contract) = universe.construction().contracts.get(&contract_id) else {
                    continue;
                };
                let Some(project) = universe.construction().projects.get(&contract.project) else {
                    continue;
                };
                crate::simulation::Mutation::CancelConstruction {
                    client: project.client,
                    contract: contract_id,
                }
            }
        };
        match worker.send(crate::simulation::Command::Mutate {
            generation: snapshot.0.generation,
            mutation,
        }) {
            Ok(()) => state.pending = Some(snapshot.0.mutation_revision),
            Err(error) => state.error = Some(error),
        }
        break;
    }
}

pub(crate) fn render_error(parent: &mut ChildSpawnerCommands, state: &State) {
    if let Some(error) = &state.error {
        parent.spawn(text(error, 13.0, MUTED));
    }
}

pub(crate) fn cashflow(
    cash: learning_lord_simulation::construction::ConstructionCash,
) -> Option<String> {
    let mut items = Vec::new();
    if cash.escrow_deposited > 0 {
        items.push(format!("Escrow deposited -{}", cash.escrow_deposited));
    }
    if cash.escrow_refunded > 0 {
        items.push(format!("Escrow refunded +{}", cash.escrow_refunded));
    }
    if cash.payments_received > 0 {
        items.push(format!("Payments received +{}", cash.payments_received));
    }
    (!items.is_empty()).then(|| format!("Construction | {} coins", items.join(" | ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use learning_lord_simulation::{AgentKind, Citizen, Coins};

    fn fixture(coins: Coins) -> (Universe, Destination) {
        let (world, owner) = Universe::default()
            .with_citizen(
                "Owner",
                Citizen::new(0.0).unwrap().with_coins(coins).unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents()[&owner].kind;
        let map = world.map();
        let place = map.place(citizen.home()).unwrap();
        let building = &place.buildings[0];
        let destination = Destination {
            place: place.id,
            building: building.id,
            room: Some(building.rooms[0].id),
            owner,
        };
        (world, destination)
    }

    fn app(world: Universe) -> App {
        let worker = crate::simulation::SimulationWorker::spawn(world, 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<State>()
            .init_resource::<crate::locations::State>()
            .init_resource::<market::Selection>()
            .add_systems(Update, handle);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        app
    }

    fn press(app: &mut App, choice: Choice) {
        let entity = app.world_mut().spawn((choice, Interaction::Pressed)).id();
        app.update();
        app.world_mut().despawn(entity);
    }

    fn await_request(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.world().resource::<State>().pending() {
            assert!(std::time::Instant::now() < deadline);
            let snapshot = app
                .world()
                .resource::<crate::simulation::SimulationWorker>()
                .snapshot();
            app.world_mut().resource_mut::<DisplaySnapshot>().0 = snapshot;
            app.update();
            std::thread::yield_now();
        }
    }

    #[test]
    fn request_and_cancel_use_the_snapshot_mutation_channel() {
        let (world, destination) = fixture(1_000_000);
        let maximum = world
            .construction_budget(destination.owner)
            .unwrap()
            .maximum_offer;
        let mut app = app(world);
        press(
            &mut app,
            Choice::Request(destination, ConstructionItem::FishCrate),
        );
        assert!(app.world().resource::<State>().pending());
        await_request(&mut app);
        let snapshot = &app.world().resource::<DisplaySnapshot>().0;
        assert!(snapshot.mutation_error.is_none());
        let contract = snapshot
            .universe
            .construction()
            .contracts
            .values()
            .next()
            .unwrap();
        assert_eq!(contract.maximum_offer, maximum);
        let id = contract.id;
        press(&mut app, Choice::Cancel(id));
        await_request(&mut app);
        assert_eq!(
            app.world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .construction()
                .contracts[&id]
                .status,
            ContractStatus::Cancelled
        );
    }

    #[test]
    fn continuation_uses_remaining_project_and_latest_contract_sequence() {
        let (world, destination) = fixture(1_000_000);
        let (world, original) = world
            .request_construction(
                destination.owner,
                destination.place,
                destination.building,
                destination.room,
                ConstructionItem::FishCrate,
                1,
            )
            .unwrap();
        let world = world
            .advance(learning_lord_simulation::construction::BIDDING_MS)
            .unwrap();
        let project = world.construction().contracts[&original].project;
        let mut app = app(world);
        press(&mut app, Choice::Continue(project));
        await_request(&mut app);
        let snapshot = &app.world().resource::<DisplaySnapshot>().0;
        assert!(snapshot.mutation_error.is_none());
        assert_eq!(snapshot.universe.construction().projects.len(), 1);
        let contract = latest_contract(
            &snapshot.universe,
            &snapshot.universe.construction().projects[&project],
        )
        .unwrap();
        assert_eq!(contract.status, ContractStatus::Bidding);
        let latest = contract.id;
        press(&mut app, Choice::Cancel(latest));
        await_request(&mut app);
        press(&mut app, Choice::Continue(project));
        await_request(&mut app);
        let snapshot = &app.world().resource::<DisplaySnapshot>().0;
        let contract = latest_contract(
            &snapshot.universe,
            &snapshot.universe.construction().projects[&project],
        )
        .unwrap();
        assert_eq!(contract.status, ContractStatus::Bidding);
        assert!(contract.sequence > snapshot.universe.construction().contracts[&latest].sequence);
        assert_eq!(
            contract.posted_ms,
            snapshot.universe.construction().contracts[&latest].posted_ms
        );
    }

    #[test]
    fn generation_change_discards_old_pressed_controls() {
        let (world, destination) = fixture(1_000_000);
        let mut app = app(world);
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation = 1;
        press(
            &mut app,
            Choice::Request(destination, ConstructionItem::FishCrate),
        );
        assert!(!app.world().resource::<State>().pending());
        assert!(
            app.world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .construction()
                .contracts
                .is_empty()
        );
    }

    #[test]
    fn cashflow_counts_cash_transfers_without_counting_escrow_payments_twice() {
        use learning_lord_simulation::construction::ConstructionCash;
        assert!(
            cashflow(ConstructionCash {
                payments_paid: 50,
                ..default()
            })
            .is_none()
        );
        let line = cashflow(ConstructionCash {
            escrow_deposited: 200,
            escrow_refunded: 70,
            payments_received: 20,
            payments_paid: 50,
        })
        .unwrap();
        assert_eq!(
            line,
            "Construction | Escrow deposited -200 | Escrow refunded +70 | Payments received +20 coins"
        );
    }

    #[test]
    fn choices_respect_cash_and_occupied_room_area() {
        let (world, destination) = fixture(0);
        assert!(
            unavailable(&world, destination, ConstructionItem::FishCrate)
                .unwrap()
                .contains("reserve")
        );
        let (world, destination) = fixture(1_000_000);
        assert!(unavailable(&world, destination, ConstructionItem::FishCrate).is_none());
        assert_eq!(
            unavailable(&world, destination, ConstructionItem::Millstone).as_deref(),
            Some("Insufficient unreserved area")
        );
    }

    #[test]
    fn pending_requests_reserve_room_area_in_choice_validation() {
        let (world, destination) = fixture(1_000_000);
        let (world, _) = world
            .request_construction(
                destination.owner,
                destination.place,
                destination.building,
                destination.room,
                ConstructionItem::Bed,
                1,
            )
            .unwrap();
        assert_eq!(
            unavailable(&world, destination, ConstructionItem::FishCrate).as_deref(),
            Some("Insufficient unreserved area")
        );
    }

    #[test]
    fn disabled_menu_choices_keep_tooltips_without_click_actions() {
        let (world, destination) = fixture(1_000_000);
        let map = world.map();
        let place = map.place(destination.place).unwrap().clone();
        let mut app = App::new();
        app.add_systems(Startup, move |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                render_menu(
                    parent,
                    &world,
                    &place,
                    &place.buildings[0],
                    destination.room,
                    &State {
                        expanded: Some(destination),
                        ..default()
                    },
                )
            });
        });
        app.update();
        let tooltips: Vec<_> = app
            .world_mut()
            .query::<(&crate::tooltip::Tooltip, Option<&Choice>)>()
            .iter(app.world())
            .map(|(tip, choice)| (tip.0.clone(), choice.is_some()))
            .collect();
        assert_eq!(tooltips.len(), ConstructionItem::OBJECTS.len());
        assert!(tooltips.iter().any(|(tip, enabled)| !enabled
            && tip.contains("Insufficient unreserved area")
            && tip.contains("Seven-day essential reserve")));
        assert!(tooltips.iter().any(|(_, enabled)| *enabled));
    }

    #[test]
    fn failed_auction_requires_higher_budget_to_continue() {
        let (world, destination) = fixture(1_000_000);
        let offer = world
            .construction_budget(destination.owner)
            .unwrap()
            .maximum_offer;
        let (world, contract_id) = world
            .request_construction(
                destination.owner,
                destination.place,
                destination.building,
                destination.room,
                ConstructionItem::FishCrate,
                offer,
            )
            .unwrap();
        let world = world
            .advance(learning_lord_simulation::construction::BIDDING_MS)
            .unwrap();
        let contract = &world.construction().contracts[&contract_id];
        assert_eq!(contract.status, ContractStatus::NoBids);
        let project = &world.construction().projects[&contract.project];
        assert_eq!(
            continuation_reason(&world, project).as_deref(),
            Some("Reposting after no bids requires a higher available budget")
        );
    }
}
