use crate::{
    DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, market,
    simulation::{Command, Mutation, SimulationWorker},
    text,
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    ui::RelativeCursorPosition,
};
use learning_lord_simulation::{
    AgentId, AgentKind, Universe,
    brewing::Batch,
    farming::{Crop, CropStatus},
    locations::{Location, Object, ObjectKind, Place, PlaceId, Room},
    marketplace::Good,
    production::Recipe,
    storage::GoodsOwner,
    taxation::{TaxAmount, TaxKind, TaxPayer, TaxRate, TaxRuleId, TaxScope},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    FlatFee,
    Socage,
    Asset,
    Income,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::FlatFee => "Flat fee",
            Self::Socage => "Socage",
            Self::Asset => "Asset",
            Self::Income => "Income",
        }
    }
}

#[derive(Clone)]
struct Draft {
    id: Option<TaxRuleId>,
    name: String,
    kind: Kind,
    payer: Option<AgentId>,
    owner_payer: bool,
    automatic_name: bool,
    coins: String,
    rates: Vec<(Good, String)>,
    all_rate: String,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            kind: Kind::FlatFee,
            payer: None,
            owner_payer: false,
            automatic_name: true,
            coins: String::new(),
            rates: Vec::new(),
            all_rate: String::new(),
        }
    }
}

impl Draft {
    fn remember_common_rate(&mut self) {
        if let Some((_, rate)) = self.rates.first()
            && self.rates.iter().all(|(_, value)| value == rate)
        {
            self.all_rate = rate.clone();
        }
    }

    fn select_all(&mut self) {
        self.remember_common_rate();
        self.rates.clear();
    }

    fn toggle_good(&mut self, good: Good) {
        if let Some(index) = self.rates.iter().position(|(item, _)| *item == good) {
            if self.rates.len() == 1 {
                self.all_rate = self.rates[index].1.clone();
            }
            self.rates.remove(index);
        } else {
            self.rates.push((good, self.all_rate.clone()));
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Field {
    Name,
    Coins,
    Rate(Good),
    AllRate,
    Quantity,
}

#[derive(Resource, Default)]
pub struct State {
    place: Option<PlaceId>,
    location_type: Option<Location>,
    draft: Option<Draft>,
    drafts: Vec<(TaxScope, Draft)>,
    pending_scope: Option<TaxScope>,
    field: Option<Field>,
    citizen: Option<AgentId>,
    good: Option<Good>,
    quantity: String,
    awaiting_mutation: Option<(u64, bool)>,
    generation: u64,
    error: Option<String>,
    dirty: bool,
    revision: Option<u64>,
}

impl State {
    pub(crate) fn selected_place(&self) -> Option<PlaceId> {
        if self.location_type.is_none() {
            self.place
        } else {
            None
        }
    }
    #[cfg(test)]
    pub fn pending_fixture() -> Self {
        Self {
            awaiting_mutation: Some((0, false)),
            ..default()
        }
    }

    #[cfg(test)]
    pub fn focused_fixture() -> Self {
        Self {
            draft: Some(Draft::default()),
            field: Some(Field::Name),
            ..default()
        }
    }

    pub fn pending(&self) -> bool {
        self.awaiting_mutation.is_some()
    }

    pub fn editing(&self) -> bool {
        self.field.is_some()
    }
    fn value_mut(&mut self, field: Field) -> Option<&mut String> {
        match field {
            Field::Quantity => Some(&mut self.quantity),
            Field::Name => self.draft.as_mut().map(|draft| &mut draft.name),
            Field::AllRate => self.draft.as_mut().map(|draft| &mut draft.all_rate),
            Field::Coins => self.draft.as_mut().map(|draft| &mut draft.coins),
            Field::Rate(good) => self
                .draft
                .as_mut()?
                .rates
                .iter_mut()
                .find(|(item, _)| *item == good)
                .map(|(_, value)| value),
        }
    }
}

const PRIVATE_TYPES: [Location; 7] = [
    Location::Home,
    Location::Field,
    Location::Mill,
    Location::Bakery,
    Location::Weavery,
    Location::Tailory,
    Location::Brewery,
];
const WEEKLY_TAX_TIME: &str = "Weekly taxes: Sunday at 04:00 | first collection Day 6";

fn type_name(location: Location) -> String {
    let plural = match location {
        Location::Bakery => "bakeries".into(),
        Location::Weavery => "weaveries".into(),
        Location::Tailory => "tailories".into(),
        Location::Brewery => "breweries".into(),
        _ => format!("{}s", location.name()),
    };
    format!("All {plural}")
}

impl State {
    fn scope(&self) -> Option<TaxScope> {
        self.location_type
            .map(TaxScope::LocationType)
            .or_else(|| self.place.map(TaxScope::Location))
    }

    fn select_scope(&mut self, scope: Option<TaxScope>) {
        if self.scope() != scope {
            if let Some(previous) = self.scope() {
                self.drafts.retain(|(saved, _)| *saved != previous);
                if let Some(draft) = self.draft.take() {
                    self.drafts.push((previous, draft));
                }
            }
            self.draft = scope.and_then(|scope| {
                self.drafts
                    .iter()
                    .position(|(saved, _)| *saved == scope)
                    .map(|index| self.drafts.remove(index).1)
            });
        }
        match scope {
            Some(TaxScope::Location(place)) => {
                self.place = Some(place);
                self.location_type = None;
            }
            Some(TaxScope::LocationType(kind)) => self.location_type = Some(kind),
            None => self.location_type = None,
        }
        self.field = None;
        self.error = None;
        self.dirty = true;
    }

    pub(crate) fn select_place(&mut self, place: PlaceId) {
        self.select_scope(Some(TaxScope::Location(place)));
    }

    fn location_kind(&self, universe: &Universe) -> Option<Location> {
        self.location_type.or_else(|| {
            self.place
                .and_then(|id| universe.map().place(id).ok().map(|place| place.kind))
        })
    }
}

fn default_tax_name(universe: &Universe, scope: TaxScope, kind: Kind) -> String {
    let scope = match scope {
        TaxScope::Location(id) => universe
            .map()
            .place(id)
            .map_or_else(|_| "Location".into(), |place| place.name.clone()),
        TaxScope::LocationType(location) => type_name(location),
    };
    format!("{scope} {}", kind.label())
}

#[derive(Component)]
pub(crate) struct Content;

#[derive(Component)]
pub(crate) enum ScrollPanel {
    List,
    Details,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    Place(PlaceId),
    IndividualPlaces,
    LocationTypes,
    LocationType(Location),
    New,
    Kind(Kind),
    Field(Field),
    Good(Good),
    AllGoods,
    Payer,
    Citizen,
    TransferGood,
    Deposit,
    Withdraw,
    Save,
    Cancel,
    Edit(TaxRuleId),
    Remove(TaxRuleId),
}

fn button(
    parent: &mut ChildSpawnerCommands,
    label: impl Into<String>,
    choice: Choice,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            choice,
            BackgroundColor(if selected { SELECTED } else { PANEL }),
            Node {
                padding: UiRect::axes(px(10), px(7)),
                min_height: px(32),
                border_radius: BorderRadius::all(px(7)),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(text(label, 14.0, TEXT));
        });
}

fn row(parent: &mut ChildSpawnerCommands, children: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn(Node {
            column_gap: px(8),
            row_gap: px(7),
            flex_wrap: FlexWrap::Wrap,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(children);
}

fn field(parent: &mut ChildSpawnerCommands, label: &str, value: &str, id: Field, state: &State) {
    button(
        parent,
        format!(
            "{label}: {}{}",
            if value.is_empty() { "—" } else { value },
            if state.field == Some(id) { " |" } else { "" }
        ),
        Choice::Field(id),
        state.field == Some(id),
    );
}

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            market::ViewPanel(market::View::Locations),
            Visibility::Hidden,
            Node {
                display: Display::None,
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Content,
                Node {
                    width: percent(100),
                    min_height: px(0),
                    column_gap: px(14),
                    ..default()
                },
            ));
        });
}

pub(crate) fn name(universe: &Universe, id: AgentId) -> String {
    if let Some(guild) = universe.guilds().get(&id) {
        return guild.name.clone();
    }
    if let Some(household) = universe.households().get(&id) {
        return format!("{} household", household.name);
    }
    universe
        .agents()
        .get(&id)
        .map_or_else(|| "Unknown citizen".into(), |agent| agent.name.clone())
}

pub(crate) fn place_owner(
    universe: &Universe,
    place: &learning_lord_simulation::locations::Place,
) -> String {
    if place.kind.is_town_owned() {
        "Town".into()
    } else {
        place
            .owner
            .map_or_else(|| "Public".into(), |id| name(universe, id))
    }
}

pub(crate) fn plot_readout(plot: usize, crop: Option<Crop>, now: u64) -> String {
    let Some(crop) = crop else {
        return format!("Plot {} | Empty", plot + 1);
    };
    let status = match crop.status(now) {
        CropStatus::Growing => "Growing",
        CropStatus::Ready => "Ready",
        CropStatus::Dead => "Dead",
    };
    let tending = (0..2)
        .map(|stage| {
            let (start, end) = crop.tend_window(stage).unwrap();
            let state = if crop.tended[stage] {
                "Done"
            } else if now > end {
                "Missed"
            } else if now >= start {
                "Open"
            } else {
                "Upcoming"
            };
            format!(
                "Tend {}: {} | {} to {}",
                stage + 1,
                state,
                crate::format_clock(start),
                crate::format_clock(end)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Plot {} | {} | {} | Expected {} {}\n{}\nReady: {}",
        plot + 1,
        crop.kind.name(),
        status,
        crop.expected_yield(now),
        crop.kind.good().unit_name(),
        tending,
        crate::format_clock(crop.ready_at_ms())
    )
}

pub(crate) fn batch_readout(slot: usize, batch: Option<Batch>, now: u64) -> String {
    let stage = match batch {
        None => "Empty".into(),
        Some(Batch::Preparing) => "Preparing".into(),
        Some(batch) if batch.ready(now) => "Ready".into(),
        Some(Batch::Fermenting { ready_at_ms }) => {
            let minutes = (ready_at_ms - now).div_ceil(60_000);
            format!(
                "Fermenting | {}h {:02}m remaining",
                minutes / 60,
                minutes % 60
            )
        }
    };
    format!("Slot {} | {stage}", slot + 1)
}

#[cfg(test)]
fn stocks(universe: &Universe, place: PlaceId) -> String {
    stock_rows(universe, place)
        .iter()
        .map(|row| crate::navigation::plain(row))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn kind_label(kind: &TaxKind) -> &'static str {
    match kind {
        TaxKind::FlatFee { .. } => "Flat fee",
        TaxKind::Socage { .. } => "Socage",
        TaxKind::Asset { .. } => "Asset",
        TaxKind::Income { .. } => "Income",
    }
}

#[cfg(test)]
pub(crate) fn treasury(universe: &Universe) -> String {
    treasury_rows(universe)
        .iter()
        .map(|row| crate::navigation::plain(row))
        .collect::<Vec<_>>()
        .join("\n")
}

fn room_readout(room: &Room) -> String {
    let workers: std::collections::HashSet<_> = room
        .objects
        .iter()
        .flat_map(|object| &object.workers)
        .collect();
    format!(
        "{} | Objects {}/{} | Workers {}",
        room.name,
        room.objects.len(),
        room.object_capacity,
        workers.len()
    )
}

fn object_row(
    universe: &Universe,
    place: &Place,
    object: &Object,
) -> (Vec<crate::navigation::Fragment>, String) {
    use crate::navigation::{Fragment as F, Target};
    let same_kind = || {
        place.objects().filter(|other| {
            std::mem::discriminant(&other.kind) == std::mem::discriminant(&object.kind)
        })
    };
    let ordinal = same_kind()
        .position(|other| other.id == object.id)
        .unwrap_or(0)
        + 1;
    let now = universe.current_time_ms();
    let mut tooltip = String::new();
    let mut fragments = match object.kind {
        ObjectKind::Bed { resident } => vec![
            F::text("Bed | Resident: "),
            F::entity(name(universe, resident), Target::Citizen(resident)),
        ],
        ObjectKind::CropPlot { crop } => {
            tooltip = plot_readout(ordinal - 1, crop, now);
            match crop {
                None => vec![F::text(format!("Plot {ordinal} | Empty"))],
                Some(crop) => {
                    let status = match crop.status(now) {
                        CropStatus::Growing => "Growing",
                        CropStatus::Ready => "Ready",
                        CropStatus::Dead => "Dead",
                    };
                    vec![
                        F::text(format!("Plot {ordinal} | ")),
                        F::entity(crop.kind.name(), Target::Good(crop.kind.good())),
                        F::text(format!(
                            " | {status} | {} {}",
                            crop.expected_yield(now),
                            crop.kind.good().unit_name()
                        )),
                    ]
                }
            }
        }
        ObjectKind::FermentationVessel { batch } => {
            if let Some(Batch::Fermenting { ready_at_ms }) = batch {
                tooltip = format!("Ready: {}", crate::format_clock(ready_at_ms));
            }
            vec![F::text(
                batch_readout(ordinal - 1, batch, now).replacen("Slot", "Vessel", 1),
            )]
        }
        kind => vec![F::text(if same_kind().count() > 1 {
            format!("{} {ordinal}", kind.name())
        } else {
            kind.name().into()
        })],
    };
    let capacity = if object.kind.unlimited_workers() {
        "unlimited"
    } else {
        "1"
    };
    fragments.push(F::text(format!(
        " | Workers {}/{capacity}",
        object.workers.len()
    )));
    for (index, worker) in object.workers.iter().enumerate() {
        fragments.push(F::text(if index == 0 { ": " } else { ", " }));
        fragments.push(F::entity(name(universe, *worker), Target::Citizen(*worker)));
    }
    (fragments, tooltip)
}

fn render_object(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    place: &Place,
    object: &Object,
) {
    let (fragments, tooltip) = object_row(universe, place, object);
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                ..default()
            },
            Interaction::None,
            crate::tooltip::Tooltip(tooltip),
        ))
        .with_children(|row| crate::navigation::render(row, &fragments, 14.0, TEXT));
}

fn render_hierarchy(parent: &mut ChildSpawnerCommands, universe: &Universe, place: &Place) {
    use crate::navigation::Fragment as F;
    if place.buildings.is_empty() && place.outdoor_objects.is_empty() {
        return;
    }
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|content| {
            content.spawn(text("BUILDINGS & OBJECTS", 15.0, MUTED));
            for building in &place.buildings {
                crate::navigation::render(
                    content,
                    &[
                        F::text(format!("{} | Owner: ", building.name)),
                        crate::navigation::account(universe, building.owner),
                    ],
                    15.0,
                    TEXT,
                );
                content
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(3),
                        margin: UiRect::left(px(12)),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|rooms| {
                        for room in &building.rooms {
                            rooms.spawn(text(room_readout(room), 14.0, MUTED));
                            rooms
                                .spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(2),
                                    margin: UiRect::left(px(12)),
                                    flex_shrink: 0.0,
                                    ..default()
                                })
                                .with_children(|objects| {
                                    for object in &room.objects {
                                        render_object(objects, universe, place, object);
                                    }
                                });
                        }
                    });
            }
            if !place.outdoor_objects.is_empty() {
                content.spawn(text("Outdoors", 14.0, MUTED));
                content
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2),
                        margin: UiRect::left(px(12)),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|objects| {
                        for object in &place.outdoor_objects {
                            render_object(objects, universe, place, object);
                        }
                    });
            }
        });
}

pub fn refresh(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    mut state: ResMut<State>,
    roots: Query<Entity, With<Content>>,
    panels: Query<(&ScrollPanel, &ScrollPosition)>,
) {
    if state.generation != snapshot.0.generation {
        *state = State {
            generation: snapshot.0.generation,
            dirty: true,
            ..default()
        };
    }
    if state
        .awaiting_mutation
        .is_some_and(|(revision, _)| snapshot.0.mutation_revision > revision)
    {
        let (_, saved) = state.awaiting_mutation.take().unwrap();
        let scope = state.pending_scope.take().or_else(|| state.scope());
        if saved && snapshot.0.mutation_error.is_none() {
            if scope == state.scope() {
                state.draft = None;
                state.field = None;
            } else if let Some(scope) = scope {
                state.drafts.retain(|(saved, _)| *saved != scope);
            }
        }
        state.dirty = true;
    }
    if !state.dirty && state.revision == Some(snapshot.0.revision) {
        return;
    }
    let universe = &snapshot.0.universe;
    let map = universe.map();
    let mut places: Vec<_> = map.places().values().collect();
    places.sort_by(|a, b| a.name.cmp(&b.name));
    if state.place.is_none() {
        state.place = places.first().map(|place| place.id);
    }
    let place = state.place.and_then(|id| map.place(id).ok());
    if state.location_type.is_none() && place.is_none() {
        return;
    }
    let citizens = citizen_ids(universe);
    if state.citizen.is_none() {
        state.citizen = citizens.first().copied();
    }
    let Some(root) = roots.iter().next() else {
        return;
    };
    let mut list_scroll = Vec2::ZERO;
    let mut details_scroll = Vec2::ZERO;
    for (panel, scroll) in &panels {
        match panel {
            ScrollPanel::List => list_scroll = scroll.0,
            ScrollPanel::Details => details_scroll = scroll.0,
        }
    }
    commands.entity(root).despawn_children();
    commands.entity(root).with_children(|parent| {
        parent.spawn((Node { width: px(250), flex_shrink: 0.0, min_height: px(0), flex_direction: FlexDirection::Column, row_gap: px(7), overflow: Overflow::scroll_y(), ..default() }, ScrollPanel::List, ScrollPosition(list_scroll), RelativeCursorPosition::default()))
            .with_children(|list| {
                row(list, |row| {
                    button(row, "Individual places", Choice::IndividualPlaces, state.location_type.is_none());
                    button(row, "Location types", Choice::LocationTypes, state.location_type.is_some());
                });
                list.spawn(text("LOCATIONS", 16.0, MUTED));
                if state.location_type.is_some() {
                    for kind in PRIVATE_TYPES { button(list, type_name(kind), Choice::LocationType(kind), state.location_type == Some(kind)); }
                } else {
                for place in &places { button(list, format!("{}\n{}", place.name, place_owner(universe, place)), Choice::Place(place.id), state.place == Some(place.id)); }
                }
            });
        parent.spawn((Node { flex_basis: px(0), flex_grow: 1.0, min_width: px(0), min_height: px(0), padding: UiRect::all(px(16)), flex_direction: FlexDirection::Column, row_gap: px(12), overflow: Overflow::scroll_y(), border_radius: BorderRadius::all(px(12)), ..default() }, BackgroundColor(PANEL), ScrollPanel::Details, ScrollPosition(details_scroll), RelativeCursorPosition::default()))
            .with_children(|details| {
                if let Some(location) = state.location_type {
                    details.spawn(text(type_name(location), 23.0, TEXT));
                    details.spawn(text("One shared rule applies to every private location of this type, including future locations.", 14.0, MUTED));
                }
                if state.location_type.is_none() && let Some(place) = place {
                let place_id = place.id;
                details.spawn(text(&place.name, 23.0, TEXT));
                let owner = if place.kind.is_town_owned() { crate::navigation::Fragment::text("Town") } else { place.owner.map_or_else(|| crate::navigation::Fragment::text("Public"), |id| crate::navigation::account(universe, id)) };
                crate::navigation::render(details, &[crate::navigation::Fragment::text("Owner: "), owner], 15.0, MUTED);
                render_hierarchy(details, universe, place);
                details.spawn(text("STOCK BY OWNER", 15.0, MUTED));
                for row in stock_rows(universe, place_id) { crate::navigation::render(details, &row, 15.0, TEXT); }
                details.spawn(text("LOCAL TRANSFERS", 15.0, MUTED));
                details.spawn(text("Citizen must be at this location and stationary. Working ingredients remain reserved.", 13.0, MUTED));
                row(details, |row| {
                    button(row, format!("Citizen: {}", state.citizen.map_or_else(|| "None".into(), |id| {
                        let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
                        format!("{} ({})", name(universe, id), if citizen.position() == place.position && !citizen.active_action().is_some_and(|action| matches!(action.action(), learning_lord_simulation::CitizenAction::Travel(_))) { "onsite" } else { "away" })
                    })), Choice::Citizen, false);
                    button(row, format!("Good: {}", state.good.unwrap_or(Good::Berries).name()), Choice::TransferGood, false);
                    field(row, "Quantity (whole units)", &state.quantity, Field::Quantity, &state);
                    button(row, "Deposit", Choice::Deposit, false); button(row, "Withdraw", Choice::Withdraw, false);
                });
                }
                details.spawn(text("LOCATION TAXES", 15.0, MUTED));
                details.spawn(text(WEEKLY_TAX_TIME, 13.0, MUTED));
                let scope = state.scope().unwrap();
                let taxable = state.location_type.is_some() || place.is_some_and(|place| place.owner.is_some());
                if taxable {
                    let mut rules: Vec<_> = universe.tax_rules().values().filter(|rule| rule.active && match scope {
                        TaxScope::Location(id) => universe.tax_scope_matches(rule.scope, id),
                        TaxScope::LocationType(_) => rule.scope == scope,
                    }).collect();
                    rules.sort_by(|a, b| a.name.cmp(&b.name));
                    if rules.is_empty() { details.spawn(text("Frankalmoigne - no active tax rules.", 15.0, TEXT)); }
                    for rule in rules {
                        let inherited = rule.scope != scope;
                        details.spawn(text(format!("{} | {} | {}", rule.name, kind_label(&rule.kind), if inherited { "Inherited from location type" } else if state.location_type.is_some() { "Shared type rule" } else { "Individual rule" }), 16.0, TEXT));
                        use crate::navigation::{Fragment as F};
                        match &rule.kind {
                            TaxKind::FlatFee {payer, coins} => { let payer = match payer { TaxPayer::Agent(id) => crate::navigation::account(universe, *id), TaxPayer::LocationOwner => place.and_then(|place| place.owner).map_or_else(|| F::text("Property owner"), |id| crate::navigation::account(universe, id)) }; crate::navigation::render(details, &[F::text(format!("{coins} coins weekly · ")), payer], 14.0, MUTED); }
                            TaxKind::Socage {rates} | TaxKind::Asset {rates} | TaxKind::Income {rates} => { let mut rates = rates.iter().collect::<Vec<_>>(); rates.sort_by_key(|(good, _)| good.name()); let mut fragments = Vec::new(); for (good, rate) in rates { if !fragments.is_empty() { fragments.push(F::text(" · ")); } fragments.push(F::good(*good)); fragments.push(F::text(format!(": {:.2}%", f64::from(rate.basis_points()) / 100.0))); } crate::navigation::render(details, &fragments, 14.0, MUTED); }
                        }
                        row(details, |row| {
                            if inherited {
                                if let TaxScope::LocationType(kind) = rule.scope { button(row, "View type to edit", Choice::LocationType(kind), false); }
                            } else { button(row, "Edit", Choice::Edit(rule.id), false); button(row, "Remove", Choice::Remove(rule.id), false); }
                        });
                    }
                    if let Some(draft) = &state.draft { editor(details, draft, &state, universe); }
                    else { button(details, "Add tax rule", Choice::New, false); }
                } else {
                    details.spawn(text("Public and town locations have no location taxes.", 15.0, TEXT));
                }
                if let Some(error) = state.error.as_ref().or(snapshot.0.mutation_error.as_ref()) { details.spawn(text(error, 14.0, Color::srgb(1.0, 0.55, 0.48))); }
                details.spawn(text("TREASURY & COLLECTIONS", 15.0, MUTED));
                for row in treasury_rows(universe) { crate::navigation::render(details, &row, 14.0, TEXT); }
            });
    });
    state.dirty = false;
    state.revision = Some(snapshot.0.revision);
}

fn editor(parent: &mut ChildSpawnerCommands, draft: &Draft, state: &State, universe: &Universe) {
    parent.spawn(text(
        if draft.id.is_some() {
            "EDIT RULE"
        } else {
            "NEW RULE"
        },
        16.0,
        MUTED,
    ));
    row(parent, |row| {
        for kind in [Kind::FlatFee, Kind::Socage, Kind::Asset, Kind::Income] {
            button(row, kind.label(), Choice::Kind(kind), kind == draft.kind);
        }
    });
    field(parent, "Name", &draft.name, Field::Name, state);
    if draft.kind == Kind::FlatFee {
        if draft.owner_payer {
            parent.spawn(text(
                "Payer: property owner at each matching location",
                14.0,
                MUTED,
            ));
            field(parent, "Coins per week", &draft.coins, Field::Coins, state);
        } else {
            row(parent, |row| {
                button(
                    row,
                    format!(
                        "Payer: {}",
                        draft
                            .payer
                            .map_or_else(|| "Select citizen".into(), |id| name(universe, id))
                    ),
                    Choice::Payer,
                    false,
                );
                field(row, "Coins per week", &draft.coins, Field::Coins, state);
            });
        }
    } else {
        let goods = tax_goods(draft.kind, state.location_kind(universe).unwrap());
        parent.spawn(text(match draft.kind { Kind::Socage => "Percentage of each selected production output. Stacked total cannot exceed 100%.", Kind::Asset => "Weekly percentage of selected goods' stored and sale escrow value.", _ => "Percentage of gross sale revenue at this location." }, 13.0, MUTED));
        row(parent, |row| {
            if !goods.is_empty() {
                button(row, "All goods", Choice::AllGoods, draft.rates.is_empty());
            }
            for good in &goods {
                let good = *good;
                button(
                    row,
                    good.name(),
                    Choice::Good(good),
                    draft.rates.iter().any(|(item, _)| *item == good),
                );
            }
        });
        if goods.is_empty() {
            parent.spawn(text("This location produces no goods.", 13.0, MUTED));
        } else if draft.rates.is_empty() {
            field(
                parent,
                "All goods (%)",
                &draft.all_rate,
                Field::AllRate,
                state,
            );
        }
        for (good, rate) in draft.rates.iter().filter(|(good, _)| goods.contains(good)) {
            field(
                parent,
                &format!("{} (%)", good.name()),
                rate,
                Field::Rate(*good),
                state,
            );
        }
    }
    row(parent, |row| {
        button(row, "Save", Choice::Save, true);
        button(row, "Cancel", Choice::Cancel, false);
    });
}

fn citizen_ids(universe: &Universe) -> Vec<AgentId> {
    let mut citizens: Vec<_> = universe.agents().iter().collect();
    citizens.sort_by(|(_, a), (_, b)| a.name.cmp(&b.name));
    citizens.into_iter().map(|(id, _)| *id).collect()
}

fn next_citizen(universe: &Universe, current: Option<AgentId>) -> Option<AgentId> {
    let ids = citizen_ids(universe);
    if ids.is_empty() {
        return None;
    }
    let index = current
        .and_then(|id| ids.iter().position(|candidate| *candidate == id))
        .map_or(0, |index| (index + 1) % ids.len());
    Some(ids[index])
}

pub(crate) fn percentage(value: &str) -> Result<TaxRate, String> {
    let value = value.trim();
    let (whole, fractional) = value.split_once('.').unwrap_or((value, ""));
    if fractional.len() > 2 || !fractional.chars().all(|c| c.is_ascii_digit()) {
        return Err("Rates need at most two decimal places.".into());
    }
    let whole = whole
        .parse::<u16>()
        .map_err(|_| "Enter a percentage from 0 to 100.".to_string())?;
    let fraction = if fractional.is_empty() {
        0
    } else {
        fractional
            .parse::<u16>()
            .map_err(|_| "Invalid percentage.".to_string())?
            * if fractional.len() == 1 { 10 } else { 1 }
    };
    let points = whole
        .checked_mul(100)
        .and_then(|whole| whole.checked_add(fraction))
        .ok_or("Percentage is too large.")?;
    TaxRate::new(points).map_err(|error| error.to_string())
}

fn tax_goods(kind: Kind, location: Location) -> Vec<Good> {
    Good::ALL
        .into_iter()
        .filter(|good| {
            kind != Kind::Socage
                || Recipe::ALL.into_iter().any(|recipe| {
                    recipe.location() == location
                        && recipe.outputs().iter().any(|(output, _)| output == good)
                })
        })
        .collect()
}

fn tax_kind(draft: &Draft, goods: &[Good]) -> Result<TaxKind, String> {
    if draft.name.trim().is_empty() {
        return Err("Give the tax rule a name.".into());
    }
    if draft.kind == Kind::FlatFee {
        let coins = draft
            .coins
            .parse::<i64>()
            .map_err(|_| "Enter a whole coin amount.".to_string())?;
        if coins <= 0 {
            return Err("Flat fee must be positive.".into());
        }
        return Ok(TaxKind::FlatFee {
            payer: if draft.owner_payer {
                TaxPayer::LocationOwner
            } else {
                TaxPayer::Agent(draft.payer.ok_or("Select a payer.")?)
            },
            coins,
        });
    }
    if goods.is_empty() {
        return Err("This location produces no goods.".into());
    }
    if draft.rates.iter().any(|(good, _)| !goods.contains(good)) {
        return Err("This rule contains goods not produced here. Choose All goods to replace them with this location’s production goods.".into());
    }
    let rates = if draft.rates.is_empty() {
        let rate = percentage(&draft.all_rate)?;
        goods.iter().map(|good| (*good, rate)).collect()
    } else {
        draft
            .rates
            .iter()
            .map(|(good, value)| percentage(value).map(|rate| (*good, rate)))
            .collect::<Result<_, _>>()?
    };
    Ok(match draft.kind {
        Kind::Socage => TaxKind::Socage { rates },
        Kind::Asset => TaxKind::Asset { rates },
        _ => TaxKind::Income { rates },
    })
}

pub fn handle(
    mut state: ResMut<State>,
    (snapshot, worker): (Res<DisplaySnapshot>, Res<SimulationWorker>),
    buttons: Query<(&Interaction, &Choice), Changed<Interaction>>,
    mut keys: MessageReader<KeyboardInput>,
    caravan: Option<Res<crate::caravan_controls::State>>,
    controls: Option<Res<crate::Controls>>,
    mut windows: Option<ResMut<crate::floating_ui::Windows>>,
) {
    if state.generation != snapshot.0.generation {
        *state = State {
            generation: snapshot.0.generation,
            dirty: true,
            ..default()
        };
        keys.clear();
        return;
    }
    if controls.is_some_and(|controls| controls.restart_pending) {
        state.field = None;
        keys.clear();
        return;
    }
    if state.awaiting_mutation.is_some() {
        keys.clear();
        return;
    }
    let universe = &snapshot.0.universe;
    for (interaction, choice) in &buttons {
        if state.awaiting_mutation.is_some() {
            break;
        }
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.dirty = true;
        state.error = None;
        if !matches!(choice, Choice::Field(_)) {
            state.field = None;
        }
        let mutation = match *choice {
            Choice::Place(place) => {
                if let Some(windows) = windows.as_mut() {
                    windows.open(crate::floating_ui::WindowKind::Locations);
                }
                state.select_place(place);
                None
            }
            Choice::IndividualPlaces => {
                let scope = state.place.map(TaxScope::Location);
                state.select_scope(scope);
                None
            }
            Choice::LocationTypes | Choice::LocationType(_) => {
                let kind = match *choice {
                    Choice::LocationType(kind) => kind,
                    _ => Location::Home,
                };
                state.select_scope(Some(TaxScope::LocationType(kind)));
                None
            }
            Choice::New => {
                let name = state
                    .scope()
                    .map(|scope| default_tax_name(universe, scope, Kind::FlatFee))
                    .unwrap_or_default();
                state.draft = Some(Draft {
                    name,
                    payer: state.citizen,
                    owner_payer: state.location_type.is_some()
                        || state
                            .place
                            .and_then(|id| {
                                universe.map().place(id).ok().and_then(|place| place.owner)
                            })
                            .is_some_and(|id| universe.guilds().contains_key(&id)),
                    ..default()
                });
                None
            }
            Choice::Kind(kind) => {
                let location = state.location_kind(universe);
                let name = state
                    .scope()
                    .map(|scope| default_tax_name(universe, scope, kind));
                if let Some(draft) = &mut state.draft {
                    draft.kind = kind;
                    if draft.id.is_none()
                        && draft.automatic_name
                        && let Some(name) = name
                    {
                        draft.name = name;
                    }
                    if let Some(location) = location {
                        let goods = tax_goods(kind, location);
                        draft.rates.retain(|(good, _)| goods.contains(good));
                    }
                }
                None
            }
            Choice::Field(field) => {
                state.field = Some(field);
                None
            }
            Choice::Good(good) => {
                if let Some(draft) = &mut state.draft {
                    draft.toggle_good(good);
                }
                None
            }
            Choice::AllGoods => {
                if let Some(draft) = &mut state.draft {
                    draft.select_all();
                }
                None
            }
            Choice::Payer => {
                if let Some(draft) = &mut state.draft {
                    draft.payer = next_citizen(universe, draft.payer);
                }
                None
            }
            Choice::Citizen => {
                state.citizen = next_citizen(universe, state.citizen);
                None
            }
            Choice::TransferGood => {
                let good = state.good.unwrap_or(Good::Berries);
                state.good = Some(Good::ALL[(good as usize + 1) % Good::COUNT]);
                None
            }
            Choice::Cancel => {
                state.draft = None;
                None
            }
            Choice::Save => match state
                .draft
                .as_ref()
                .and_then(|draft| state.scope().map(|scope| (draft, scope)))
            {
                Some((draft, scope)) => match tax_kind(
                    draft,
                    &tax_goods(draft.kind, state.location_kind(universe).unwrap()),
                ) {
                    Ok(kind) => Some(Mutation::SaveTax {
                        scope,
                        id: draft.id,
                        name: draft.name.trim().into(),
                        kind,
                    }),
                    Err(error) => {
                        state.error = Some(error);
                        None
                    }
                },
                None => None,
            },
            Choice::Remove(id) => Some(Mutation::RemoveTax(id)),
            Choice::Edit(id) => {
                if let Some(rule) = universe.tax_rules().get(&id) {
                    let mut draft = Draft {
                        id: Some(id),
                        name: rule.name.clone(),
                        automatic_name: false,
                        owner_payer: matches!(rule.scope, TaxScope::LocationType(_)),
                        ..default()
                    };
                    match &rule.kind {
                        TaxKind::FlatFee { payer, coins } => {
                            draft.owner_payer = *payer == TaxPayer::LocationOwner;
                            draft.payer = match payer {
                                TaxPayer::Agent(id) => Some(*id),
                                TaxPayer::LocationOwner => None,
                            };
                            draft.coins = coins.to_string();
                        }
                        TaxKind::Socage { rates }
                        | TaxKind::Asset { rates }
                        | TaxKind::Income { rates } => {
                            draft.kind = match rule.kind {
                                TaxKind::Socage { .. } => Kind::Socage,
                                TaxKind::Asset { .. } => Kind::Asset,
                                _ => Kind::Income,
                            };
                            draft.rates = rates
                                .iter()
                                .map(|(good, rate)| {
                                    (
                                        *good,
                                        format!("{:.2}", f64::from(rate.basis_points()) / 100.0),
                                    )
                                })
                                .collect();
                            draft.rates.sort_by_key(|(good, _)| *good as usize);
                            draft.remember_common_rate();
                        }
                    }
                    state.draft = Some(draft);
                }
                None
            }
            Choice::Deposit | Choice::Withdraw => match state.quantity.parse::<u64>() {
                Ok(units) if units > 0 => {
                    state
                        .citizen
                        .zip(state.place)
                        .map(|(citizen, place)| Mutation::Transfer {
                            citizen,
                            place,
                            good: state.good.unwrap_or(Good::Berries),
                            units,
                            deposit: matches!(choice, Choice::Deposit),
                        })
                }
                _ => {
                    state.error = Some("Enter a positive whole quantity.".into());
                    None
                }
            },
        };
        if let Some(mutation) = mutation {
            if caravan.as_ref().is_some_and(|editor| editor.pending()) {
                state.error = Some("Another change is saving. Try again when it finishes.".into());
                continue;
            }
            state.pending_scope = state.scope();
            state.awaiting_mutation =
                Some((snapshot.0.mutation_revision, matches!(choice, Choice::Save)));
            if let Err(error) = worker.send(Command::Mutate {
                generation: snapshot.0.generation,
                mutation,
            }) {
                state.error = Some(error);
                state.awaiting_mutation = None;
                state.pending_scope = None;
            }
        }
    }
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let Some(field) = state.field else {
            continue;
        };
        if field == Field::Name
            && matches!(
                &key.logical_key,
                Key::Space | Key::Backspace | Key::Character(_)
            )
            && let Some(draft) = &mut state.draft
        {
            draft.automatic_name = false;
        }
        match &key.logical_key {
            Key::Escape | Key::Enter | Key::Tab => state.field = None,
            Key::Space if field == Field::Name => {
                if let Some(value) = state.value_mut(field)
                    && value.len() < 100
                {
                    value.push(' ');
                }
            }
            Key::Backspace => {
                if let Some(value) = state.value_mut(field) {
                    value.pop();
                }
            }
            Key::Character(value) => {
                if let Some(target) = state.value_mut(field)
                    && target.len() < 100
                    && (field == Field::Name
                        || value.chars().all(|character| {
                            character.is_ascii_digit()
                                || (matches!(field, Field::Rate(_) | Field::AllRate)
                                    && character == '.')
                        }))
                {
                    target.push_str(value);
                }
            }
            _ => {}
        }
        state.dirty = true;
    }
}

pub(crate) fn select_place(state: &mut State, place: PlaceId) {
    state.place = Some(place);
    state.location_type = None;
    state.draft = None;
    state.field = None;
    state.dirty = true;
}

pub(crate) fn stop_editing(state: &mut State) {
    if state.field.take().is_some() {
        state.dirty = true;
    }
}

pub(crate) fn stock_rows(
    universe: &Universe,
    place: PlaceId,
) -> Vec<Vec<crate::navigation::Fragment>> {
    use crate::navigation::{self as nav, Fragment as F};
    let mut rows = Vec::new();
    for (key, units) in universe
        .storage()
        .stock()
        .iter()
        .filter(|(key, _)| key.place == place)
    {
        let owner = match key.owner {
            GoodsOwner::Town => F::text("Town"),
            GoodsOwner::Agent(id) | GoodsOwner::Household(id) => nav::account(universe, id),
        };
        let reserved = universe
            .storage()
            .reserved_units(place, key.owner, key.good);
        rows.push(vec![
            owner,
            F::text(" | "),
            F::good(key.good),
            F::text(format!(
                " | {} available | {} tax reserved",
                crate::quantity_label(key.good, units.saturating_sub(reserved)),
                crate::quantity_label(key.good, reserved)
            )),
        ]);
    }
    for order in universe
        .market()
        .orders()
        .filter(|order| order.place == place)
    {
        let owner = match order.seller {
            learning_lord_simulation::marketplace::MarketParty::Caravan => F::text("Caravan"),
            learning_lord_simulation::marketplace::MarketParty::Citizen(id)
            | learning_lord_simulation::marketplace::MarketParty::Household(id) => {
                nav::account(universe, id)
            }
        };
        rows.push(vec![
            owner,
            F::text(" | "),
            F::good(order.good),
            F::text(format!(
                " | {} sale escrow",
                crate::quantity_label(order.good, order.units)
            )),
        ]);
    }
    rows.sort_by_key(|row| nav::plain(row));
    if rows.is_empty() {
        rows.push(vec![F::text("No stored goods or sale escrow.")]);
    }
    rows
}
fn treasury_rows(universe: &Universe) -> Vec<Vec<crate::navigation::Fragment>> {
    use crate::navigation::{self as nav, Fragment as F, Target};
    let mut rows = vec![vec![F::text(format!(
        "Town treasury: {} coins",
        universe.town_treasury()
    ))]];
    for rule in universe.tax_rules().values() {
        let independent = universe.agents().iter().filter_map(|(id, agent)| {
            let AgentKind::Citizen(citizen) = &agent.kind;
            citizen.household_id().is_none().then_some(id)
        });
        for id in independent
            .chain(universe.households().keys())
            .chain(universe.guilds().keys())
        {
            let owed = universe.tax_arrears(rule.id, *id);
            if owed > 0 {
                rows.push(vec![
                    nav::account(universe, *id),
                    F::text(format!(
                        " owes {owed} coins | {}{}",
                        rule.name,
                        if rule.active { "" } else { " (retired)" }
                    )),
                ]);
            }
        }
    }
    if rows.len() == 1 {
        rows.push(vec![F::text("No coin arrears.")]);
    }
    for receipt in universe.tax_history().iter().rev().take(12) {
        let mut amount = Vec::new();
        if let Some(good) = receipt.good {
            amount.push(F::good(good));
            amount.push(F::text(": "));
        }
        amount.push(F::text(match receipt.amount {
            TaxAmount::Coins {
                assessed,
                paid,
                arrears,
            } => format!("{assessed} coins assessed, {paid} paid, {arrears} owed"),
            TaxAmount::Reservation {
                assessed,
                reserved,
                outstanding,
            } => {
                format!("{assessed} units assessed, {reserved} reserved, {outstanding} outstanding")
            }
            TaxAmount::Goods {
                assessed,
                collected,
                outstanding,
            } => format!("{assessed} units assessed, {collected} collected, {outstanding} owed"),
        }));
        let mut row = vec![
            F::text(format!(
                "{} | {} | ",
                crate::format_clock(receipt.time_ms),
                universe
                    .tax_rules()
                    .get(&receipt.rule)
                    .map_or("Retired rule", |rule| rule.name.as_str())
            )),
            nav::account(universe, receipt.payer),
            F::text(" | "),
            F::entity(
                universe
                    .map()
                    .place(receipt.place)
                    .map_or("Unknown location".into(), |place| place.name.clone()),
                Target::Location(receipt.place),
            ),
            F::text(" | "),
        ];
        row.extend(amount);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_scopes_preserves_drafts_and_clears_focus() {
        let universe = Universe::default();
        let place = *universe.map().places().keys().next().unwrap();
        let mut state = State::default();
        state.select_place(place);
        state.draft = Some(Draft {
            name: "Place draft".into(),
            ..default()
        });
        state.field = Some(Field::Name);
        state.select_scope(Some(TaxScope::LocationType(Location::Field)));
        assert!(!state.editing());
        assert!(state.draft.is_none());
        state.draft = Some(Draft {
            name: "Type draft".into(),
            ..default()
        });
        state.select_place(place);
        assert_eq!(state.draft.as_ref().unwrap().name, "Place draft");
        state.select_scope(Some(TaxScope::LocationType(Location::Field)));
        assert_eq!(state.draft.as_ref().unwrap().name, "Type draft");
        state.select_scope(Some(TaxScope::LocationType(Location::Field)));
        assert_eq!(state.draft.as_ref().unwrap().name, "Type draft");
    }

    #[test]
    fn save_acknowledgment_clears_only_originating_scope() {
        let universe = Universe::default();
        let place = *universe.map().places().keys().next().unwrap();
        let worker = SimulationWorker::spawn(universe, 60.try_into().unwrap());
        let mut snapshot = worker.snapshot();
        snapshot.mutation_revision = 1;
        let mut state = State::default();
        state.select_place(place);
        state.draft = Some(Draft {
            name: "Saved".into(),
            ..default()
        });
        state.pending_scope = state.scope();
        state.awaiting_mutation = Some((0, true));
        state.select_scope(Some(TaxScope::LocationType(Location::Field)));
        state.draft = Some(Draft {
            name: "Unsaved".into(),
            ..default()
        });
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(snapshot))
            .insert_resource(state)
            .add_systems(Update, refresh);
        app.update();
        let mut state = app.world_mut().resource_mut::<State>();
        assert_eq!(state.draft.as_ref().unwrap().name, "Unsaved");
        state.select_place(place);
        assert!(state.draft.is_none());
    }

    #[test]
    fn treasury_reports_household_arrears_once() {
        use learning_lord_simulation::{Citizen, taxation::TaxPayer};
        let (universe, first) = Universe::default()
            .with_citizen("Ada", Citizen::new(0.0).unwrap())
            .unwrap();
        let (universe, second) = universe
            .with_citizen("Iris", Citizen::new(0.0).unwrap())
            .unwrap();
        let AgentKind::Citizen(citizen) = &universe.agents()[&first].kind;
        let home = citizen.home();
        let (universe, household) = universe
            .with_household("Ada and Iris", home, &[first, second])
            .unwrap();
        let universe = universe.with_property(first, Location::Field).unwrap().0;
        let (universe, _) = universe
            .with_scoped_tax_rule(
                TaxScope::LocationType(Location::Field),
                "Field fee",
                TaxKind::FlatFee {
                    payer: TaxPayer::LocationOwner,
                    coins: 10,
                },
            )
            .unwrap();
        let universe = universe
            .advance(learning_lord_simulation::calendar::FIRST_WEEKLY_SETTLEMENT_MS)
            .unwrap();
        let readout = treasury(&universe);
        assert_eq!(
            readout
                .lines()
                .filter(|line| line.contains(" owes "))
                .count(),
            1,
            "{readout}"
        );
        assert!(
            readout.contains(&format!("{} owes 10 coins", name(&universe, household))),
            "{readout}"
        );
    }

    #[test]
    fn brewery_slots_show_each_stage_and_round_remaining_minutes_up() {
        assert_eq!(batch_readout(0, None, 0), "Slot 1 | Empty");
        assert_eq!(
            batch_readout(1, Some(Batch::Preparing), 0),
            "Slot 2 | Preparing"
        );
        let batch = Some(Batch::Fermenting {
            ready_at_ms: 48 * 3_600_000,
        });
        assert_eq!(
            batch_readout(2, batch, 0),
            "Slot 3 | Fermenting | 48h 00m remaining"
        );
        assert_eq!(
            batch_readout(2, batch, 1),
            "Slot 3 | Fermenting | 48h 00m remaining"
        );
        assert_eq!(batch_readout(3, batch, 48 * 3_600_000), "Slot 4 | Ready");
        assert_eq!(batch_readout(3, batch, u64::MAX), "Slot 4 | Ready");
    }

    #[test]
    fn brewing_and_hops_have_type_scopes_and_production_tax_goods() {
        assert!(PRIVATE_TYPES.contains(&Location::Brewery));
        assert_eq!(type_name(Location::Brewery), "All breweries");
        assert_eq!(tax_goods(Kind::Socage, Location::Brewery), vec![Good::Beer]);
        assert!(tax_goods(Kind::Socage, Location::Field).contains(&Good::Hops));
    }

    #[test]
    fn cabbage_plot_yields_are_labelled_as_whole_heads() {
        use learning_lord_simulation::farming::{CropKind, HOUR_MS};
        for (tended, expected) in [([true, true], 8), ([true, false], 5)] {
            let crop = Crop {
                kind: CropKind::Cabbage,
                planted_at_ms: 0,
                tended,
            };
            let readout = plot_readout(0, Some(crop), 96 * HOUR_MS);
            assert!(readout.contains(&format!("Cabbage | Ready | Expected {expected} cabbages")));
        }
    }

    #[test]
    fn plots_show_empty_growing_ready_and_dead_crops_with_tending_windows() {
        use learning_lord_simulation::farming::{CropKind, HOUR_MS};
        assert_eq!(plot_readout(0, None, 0), "Plot 1 | Empty");
        let mut crop = Crop {
            kind: CropKind::Wheat,
            planted_at_ms: 0,
            tended: [false; 2],
        };
        let growing = plot_readout(1, Some(crop), 24 * HOUR_MS);
        assert!(growing.contains("Plot 2 | Wheat | Growing | Expected 150 g"));
        assert!(growing.contains("Tend 1: Open"));
        assert!(growing.contains("Tend 2: Upcoming"));
        assert!(growing.contains("Ready: Day 4"));
        crop.tended[0] = true;
        let ready = plot_readout(1, Some(crop), 96 * HOUR_MS);
        assert!(ready.contains("Ready | Expected 375 g"));
        assert!(ready.contains("Tend 1: Done"));
        assert!(ready.contains("Tend 2: Missed"));
        crop.tended[0] = false;
        assert!(plot_readout(1, Some(crop), 96 * HOUR_MS).contains("Dead | Expected 0 g"));
    }

    fn ui_app(universe: Universe, state: State) -> App {
        let worker = SimulationWorker::spawn(universe, 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .insert_resource(state)
            .init_resource::<market::Selection>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, handle);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        app
    }

    fn press(app: &mut App, choice: Choice) {
        let entity = app.world_mut().spawn((choice, Interaction::Pressed)).id();
        app.update();
        app.world_mut().despawn(entity);
    }

    #[test]
    fn new_scope_names_follow_kind_until_customized() {
        let mut app = ui_app(
            Universe::default(),
            State {
                location_type: Some(Location::Field),
                ..default()
            },
        );
        press(&mut app, Choice::New);
        assert_eq!(
            app.world().resource::<State>().draft.as_ref().unwrap().name,
            "All fields Flat fee"
        );
        press(&mut app, Choice::Kind(Kind::Income));
        assert_eq!(
            app.world().resource::<State>().draft.as_ref().unwrap().name,
            "All fields Income"
        );
        {
            let mut state = app.world_mut().resource_mut::<State>();
            let draft = state.draft.as_mut().unwrap();
            draft.name = "Custom charge".into();
            draft.automatic_name = false;
        }
        press(&mut app, Choice::Kind(Kind::Asset));
        assert_eq!(
            app.world().resource::<State>().draft.as_ref().unwrap().name,
            "Custom charge"
        );
    }

    #[test]
    fn type_flat_fees_use_property_owner_and_existing_names_stay() {
        let (universe, id) = Universe::default()
            .with_scoped_tax_rule(
                TaxScope::LocationType(Location::Field),
                "Original",
                TaxKind::FlatFee {
                    payer: TaxPayer::LocationOwner,
                    coins: 5,
                },
            )
            .unwrap();
        let mut app = ui_app(
            universe,
            State {
                location_type: Some(Location::Field),
                ..default()
            },
        );
        press(&mut app, Choice::Edit(id));
        let draft = app.world().resource::<State>().draft.as_ref().unwrap();
        assert_eq!(
            tax_kind(draft, &Good::ALL).unwrap(),
            TaxKind::FlatFee {
                payer: TaxPayer::LocationOwner,
                coins: 5
            }
        );
        press(&mut app, Choice::Kind(Kind::Income));
        assert_eq!(
            app.world().resource::<State>().draft.as_ref().unwrap().name,
            "Original"
        );
        assert!(WEEKLY_TAX_TIME.contains("Sunday at 04:00"));
        assert!(WEEKLY_TAX_TIME.contains("Day 6"));
    }

    #[test]
    fn existing_type_rate_rules_switch_to_owner_flat_fees() {
        for kind in [
            TaxKind::Socage {
                rates: Default::default(),
            },
            TaxKind::Asset {
                rates: Default::default(),
            },
            TaxKind::Income {
                rates: Default::default(),
            },
        ] {
            let (universe, id) = Universe::default()
                .with_scoped_tax_rule(TaxScope::LocationType(Location::Field), "Original", kind)
                .unwrap();
            let mut app = ui_app(
                universe,
                State {
                    location_type: Some(Location::Field),
                    ..default()
                },
            );
            press(&mut app, Choice::Edit(id));
            press(&mut app, Choice::Kind(Kind::FlatFee));
            app.world_mut()
                .resource_mut::<State>()
                .draft
                .as_mut()
                .unwrap()
                .coins = "1".into();
            let draft = app.world().resource::<State>().draft.as_ref().unwrap();
            assert_eq!(
                tax_kind(draft, &Good::ALL).unwrap(),
                TaxKind::FlatFee {
                    payer: TaxPayer::LocationOwner,
                    coins: 1
                }
            );
            assert_eq!(draft.name, "Original");
        }
    }

    #[test]
    fn individual_view_distinguishes_shared_and_local_rules() {
        let (universe, owner) = Universe::default()
            .with_citizen("Ada", learning_lord_simulation::Citizen::new(0.0).unwrap())
            .unwrap();
        let (universe, place) = universe.with_property(owner, Location::Field).unwrap();
        let fee = TaxKind::FlatFee {
            payer: TaxPayer::LocationOwner,
            coins: 5,
        };
        let (universe, _) = universe
            .with_scoped_tax_rule(
                TaxScope::LocationType(Location::Field),
                "Shared fee",
                fee.clone(),
            )
            .unwrap();
        let (universe, _) = universe
            .with_tax_rule(
                place,
                "Local fee",
                TaxKind::FlatFee {
                    payer: TaxPayer::Agent(owner),
                    coins: 5,
                },
            )
            .unwrap();
        let worker = SimulationWorker::spawn(universe, 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(State {
                place: Some(place),
                ..default()
            })
            .init_resource::<market::Selection>()
            .add_systems(Update, refresh);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        let labels: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(
            labels
                .iter()
                .any(|text| text.contains("Shared fee | Flat fee | Inherited"))
        );
        assert!(
            labels
                .iter()
                .any(|text| text.contains("Local fee | Flat fee | Individual"))
        );
        assert!(labels.iter().any(|text| text == "View type to edit"));
    }

    #[test]
    fn individual_default_name_uses_actual_property_name() {
        let (universe, owner) = Universe::default()
            .with_citizen("Ada", learning_lord_simulation::Citizen::new(0.0).unwrap())
            .unwrap();
        let (universe, place) = universe.with_property(owner, Location::Field).unwrap();
        assert_eq!(
            default_tax_name(&universe, TaxScope::Location(place), Kind::Socage),
            format!("{} Socage", universe.map().place(place).unwrap().name)
        );
    }

    #[test]
    fn dirty_refresh_preserves_both_scroll_offsets() {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .init_resource::<State>()
            .init_resource::<market::Selection>()
            .add_systems(Update, refresh);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        for (panel, mut scroll) in app
            .world_mut()
            .query::<(&ScrollPanel, &mut ScrollPosition)>()
            .iter_mut(app.world_mut())
        {
            scroll.0.y = match panel {
                ScrollPanel::List => 123.0,
                ScrollPanel::Details => 456.0,
            };
        }
        app.world_mut().resource_mut::<State>().dirty = true;
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        for (panel, scroll) in app
            .world_mut()
            .query::<(&ScrollPanel, &ScrollPosition)>()
            .iter(app.world())
        {
            assert_eq!(
                scroll.0.y,
                match panel {
                    ScrollPanel::List => 123.0,
                    ScrollPanel::Details => 456.0,
                }
            );
        }
    }

    #[test]
    fn a_pending_transfer_blocks_save_until_its_own_acknowledgment() {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<market::Selection>()
            .insert_resource(State {
                awaiting_mutation: Some((0, false)),
                draft: Some(Draft {
                    name: "Keep this".into(),
                    ..default()
                }),
                ..default()
            })
            .add_message::<KeyboardInput>()
            .add_systems(Update, handle);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        app.world_mut().spawn((Choice::Save, Interaction::Pressed));
        app.update();
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .mutation_revision,
            0
        );
        assert!(app.world().resource::<State>().draft.is_some());
    }

    #[test]
    fn keyboard_input_keeps_named_rules_with_spaces() {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<market::Selection>()
            .insert_resource(State::focused_fixture())
            .add_message::<KeyboardInput>()
            .add_systems(Update, handle);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        for logical_key in [
            Key::Character("Harvest".into()),
            Key::Space,
            Key::Character("fee".into()),
        ] {
            app.world_mut()
                .resource_mut::<Messages<KeyboardInput>>()
                .write(KeyboardInput {
                    key_code: KeyCode::KeyH,
                    logical_key,
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                });
        }
        app.update();
        assert_eq!(
            app.world().resource::<State>().draft.as_ref().unwrap().name,
            "Harvest fee"
        );
    }

    #[test]
    fn selecting_and_clearing_goods_restores_the_default_all_rate() {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<market::Selection>()
            .insert_resource(State {
                draft: Some(Draft {
                    kind: Kind::Asset,
                    all_rate: "5".into(),
                    ..default()
                }),
                ..default()
            })
            .add_message::<KeyboardInput>()
            .add_systems(Update, handle);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        let button = app
            .world_mut()
            .spawn((Choice::Good(Good::Wheat), Interaction::Pressed))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<State>()
                .draft
                .as_ref()
                .unwrap()
                .rates,
            vec![(Good::Wheat, "5".into())]
        );
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();
        let draft = app.world().resource::<State>().draft.as_ref().unwrap();
        assert!(draft.rates.is_empty());
        assert_eq!(draft.all_rate, "5");
    }

    #[test]
    fn switching_to_socage_removes_goods_the_location_cannot_produce() {
        let (universe, owner) = Universe::default()
            .with_citizen("Ada", learning_lord_simulation::Citizen::new(0.0).unwrap())
            .unwrap();
        let (universe, place) = universe.with_property(owner, Location::Field).unwrap();
        let worker = SimulationWorker::spawn(universe, 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<market::Selection>()
            .insert_resource(State {
                place: Some(place),
                draft: Some(Draft {
                    kind: Kind::Asset,
                    rates: vec![(Good::Wheat, "5".into()), (Good::Berries, "10".into())],
                    ..default()
                }),
                ..default()
            })
            .add_message::<KeyboardInput>()
            .add_systems(Update, handle);
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Locations;
        app.world_mut()
            .spawn((Choice::Kind(Kind::Socage), Interaction::Pressed));
        app.update();
        let draft = app.world().resource::<State>().draft.as_ref().unwrap();
        assert!(draft.kind == Kind::Socage);
        assert_eq!(draft.rates, vec![(Good::Wheat, "5".into())]);
    }

    #[test]
    fn existing_mixed_socage_goods_require_explicit_replacement() {
        let mut draft = Draft {
            name: "Renamed harvest".into(),
            kind: Kind::Socage,
            rates: vec![(Good::Wheat, "5".into()), (Good::Berries, "5".into())],
            ..default()
        };
        let goods = tax_goods(Kind::Socage, Location::Field);
        assert!(
            tax_kind(&draft, &goods)
                .unwrap_err()
                .contains("not produced here")
        );
        assert_eq!(draft.rates.len(), 2);
        draft.select_all();
        let TaxKind::Socage { rates } = tax_kind(&draft, &goods).unwrap() else {
            panic!("expected socage");
        };
        assert_eq!(rates.len(), 4);
        assert_eq!(rates[&Good::Flax].basis_points(), 500);
    }

    #[test]
    fn existing_rates_survive_returning_to_all_goods() {
        let mut draft = Draft {
            rates: vec![(Good::Wheat, "5".into())],
            ..default()
        };
        draft.remember_common_rate();
        assert_eq!(draft.all_rate, "5");
        draft.rates[0].1 = "7".into();
        draft.toggle_good(Good::Wheat);
        assert!(draft.rates.is_empty());
        assert_eq!(draft.all_rate, "7");
        draft.rates = vec![(Good::Wheat, "9".into()), (Good::Flax, "9".into())];
        draft.select_all();
        assert_eq!(draft.all_rate, "9");
        draft.rates = vec![(Good::Wheat, "3".into()), (Good::Flax, "4".into())];
        draft.select_all();
        assert_eq!(draft.all_rate, "9");
    }

    #[test]
    fn all_goods_defaults_to_one_rate_for_the_taxable_catalog() {
        let draft = Draft {
            name: "Harvest".into(),
            kind: Kind::Socage,
            all_rate: "12.34".into(),
            ..default()
        };
        let goods = tax_goods(Kind::Socage, Location::Field);
        assert_eq!(
            goods,
            vec![Good::Wheat, Good::Flax, Good::Cabbage, Good::Hops]
        );
        let TaxKind::Socage { rates } = tax_kind(&draft, &goods).unwrap() else {
            panic!("expected socage");
        };
        assert_eq!(rates.len(), 4);
        assert!(rates.values().all(|rate| rate.basis_points() == 1234));
        let selected = Draft {
            rates: vec![(Good::Wheat, "5".into())],
            ..draft
        };
        let TaxKind::Socage { rates } = tax_kind(&selected, &goods).unwrap() else {
            panic!("expected socage");
        };
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[&Good::Wheat].basis_points(), 500);
    }

    #[test]
    fn only_socage_is_limited_to_production_goods() {
        assert!(tax_goods(Kind::Socage, Location::Home).is_empty());
        for kind in [Kind::Asset, Kind::Income] {
            assert_eq!(tax_goods(kind, Location::Field), Good::ALL);
            assert_eq!(tax_goods(kind, Location::Home), Good::ALL);
        }
        let draft = Draft {
            name: "Harvest".into(),
            kind: Kind::Socage,
            all_rate: "5".into(),
            ..default()
        };
        assert!(tax_kind(&draft, &tax_goods(Kind::Socage, Location::Home)).is_err());
    }

    #[test]
    fn rates_preserve_distinct_goods_and_exact_hundredths() {
        assert_eq!(percentage("12.34").unwrap().basis_points(), 1234);
        assert_eq!(percentage("0.01").unwrap().basis_points(), 1);
        assert!(percentage("100.01").is_err());
        assert!(percentage("5.001").is_err());
        let draft = Draft {
            name: "Harvest".into(),
            kind: Kind::Socage,
            rates: vec![(Good::Wheat, "12.34".into()), (Good::Berries, "5".into())],
            ..default()
        };
        let TaxKind::Socage { rates } = tax_kind(&draft, &Good::ALL).unwrap() else {
            panic!("expected socage");
        };
        assert_eq!(rates[&Good::Wheat].basis_points(), 1234);
        assert_eq!(rates[&Good::Berries].basis_points(), 500);
        let blank = Draft {
            name: "Harvest".into(),
            kind: Kind::Socage,
            rates: vec![(Good::Wheat, String::new())],
            ..default()
        };
        assert!(tax_kind(&blank, &Good::ALL).is_err());
    }

    #[test]
    fn stock_readout_keeps_owners_and_escrow_separate() {
        let (universe, id) = Universe::default()
            .with_citizen("Ada", learning_lord_simulation::Citizen::new(0.0).unwrap())
            .unwrap();
        let place = universe
            .map()
            .public_place(learning_lord_simulation::locations::Location::Warehouse);
        let universe = universe
            .with_stored_good(place, GoodsOwner::Agent(id), Good::Wheat, 10)
            .unwrap()
            .with_stored_good(place, GoodsOwner::Town, Good::Wheat, 20)
            .unwrap();
        let readout = stocks(&universe, place);
        assert!(readout.contains("Ada | Wheat | 10 g available"));
        assert!(readout.contains("Town | Wheat | 20 g available"));
    }

    #[test]
    fn failed_save_preserves_draft_and_restart_clears_it() {
        let universe = Universe::default();
        let worker = SimulationWorker::spawn(universe.clone(), 60.try_into().unwrap());
        let mut snapshot = worker.snapshot();
        snapshot.mutation_revision = 1;
        snapshot.mutation_error = Some("Invalid settings".into());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(snapshot))
            .insert_resource(State {
                draft: Some(Draft {
                    name: "Keep this".into(),
                    ..default()
                }),
                awaiting_mutation: Some((0, true)),
                field: Some(Field::Name),
                ..default()
            })
            .init_resource::<market::Selection>()
            .add_systems(Update, refresh);
        app.update();
        assert_eq!(
            app.world().resource::<State>().draft.as_ref().unwrap().name,
            "Keep this"
        );
        assert!(app.world().resource::<State>().editing());
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation = 1;
        app.update();
        assert!(app.world().resource::<State>().draft.is_none());
        assert!(!app.world().resource::<State>().editing());
    }
    #[test]
    fn object_rows_distinguish_persistent_batches_from_workers_and_count_room_workers_once() {
        use learning_lord_simulation::{Citizen, CitizenAction, StartingRole};
        let mut brewer = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Brewer);
        for &(good, units) in Recipe::BrewBeer.inputs() {
            brewer = brewer.with_good(good, units).unwrap();
        }
        let (world, worker) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_citizen("Brewer", brewer)
                .unwrap();
        let (world, site) = world.with_property(worker, Location::Brewery).unwrap();
        let world = world
            .start_action(
                worker,
                CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::BrewBeer,
                },
            )
            .unwrap();
        let map = world.map();
        let place = map.place(site).unwrap();
        let room = &place.buildings[0].rooms[0];
        assert_eq!(room_readout(room), "Workroom | Objects 5/5 | Workers 1");
        let vessel = room
            .objects
            .iter()
            .find(|object| {
                matches!(
                    object.kind,
                    ObjectKind::FermentationVessel {
                        batch: Some(Batch::Preparing)
                    }
                )
            })
            .unwrap();
        let (fragments, _) = object_row(&world, place, vessel);
        assert_eq!(
            crate::navigation::plain(&fragments),
            "Vessel 1 | Preparing | Workers 1/1: Brewer"
        );
        assert!(fragments.iter().any(|fragment| fragment.target == Some(crate::navigation::Target::Citizen(worker))));
        let world = world
            .advance(learning_lord_simulation::brewing::PREPARATION_MS)
            .unwrap();
        let map = world.map();
        let place = map.place(site).unwrap();
        let vessel = place
            .objects()
            .find(|object| object.id == vessel.id)
            .unwrap();
        let (fragments, tooltip) = object_row(&world, place, vessel);
        assert_eq!(
            crate::navigation::plain(&fragments),
            "Vessel 1 | Fermenting | 48h 00m remaining | Workers 0/1"
        );
        assert!(tooltip.starts_with("Ready: "));
        assert_eq!(
            room_readout(&place.buildings[0].rooms[0]),
            "Workroom | Objects 5/5 | Workers 0"
        );
    }

    #[test]
    fn hierarchy_renders_resident_owner_links_and_keeps_crop_care_in_tooltips() {
        use learning_lord_simulation::{Citizen, CitizenAction, StartingRole, farming::CropKind};
        let (world, worker) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_citizen(
                    "Farmer",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_starting_role(StartingRole::Farmer),
                )
                .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents()[&worker].kind;
        let home = citizen.home();
        let (world, field) = world.with_property(worker, Location::Field).unwrap();
        let world = world
            .start_action(
                worker,
                CitizenAction::Plant {
                    field,
                    plot: 0,
                    crop: CropKind::Wheat,
                },
            )
            .unwrap()
            .advance(learning_lord_simulation::farming::HOUR_MS)
            .unwrap();
        let map = world.map();
        let places = [
            map.place(home).unwrap().clone(),
            map.place(field).unwrap().clone(),
            map.place(map.public_place(Location::Forest))
                .unwrap()
                .clone(),
        ];
        let mut app = App::new();
        app.add_systems(Startup, move |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                for place in &places {
                    render_hierarchy(parent, &world, place);
                }
            });
        });
        app.update();
        let labels: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(
            labels
                .iter()
                .any(|label| label == "Bedroom | Objects 1/1 | Workers 0")
        );
        assert!(labels.iter().any(|label| label == "Bed | Resident: "));
        assert!(labels.iter().any(|label| label == "Berry bushes"));
        assert!(labels.iter().any(|label| label == " | Workers 0/unlimited"));
        assert!(!labels.iter().any(|label| label.contains("Tend 1:")));
        assert!(
            app.world_mut()
                .query::<&crate::tooltip::Tooltip>()
                .iter(app.world())
                .any(|tooltip| tooltip.0.contains("Tend 1:") && tooltip.0.contains("Tend 2:"))
        );
        assert!(
            app.world_mut()
                .query::<&crate::navigation::Target>()
                .iter(app.world())
                .filter(|target| **target == crate::navigation::Target::Citizen(worker))
                .count()
                >= 2
        );
        assert!(
            app.world_mut()
                .query::<&crate::navigation::Target>()
                .iter(app.world())
                .any(|target| *target == crate::navigation::Target::Good(Good::Wheat))
        );
    }
}
