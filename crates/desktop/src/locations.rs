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
    locations::{Location, PlaceId},
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
    #[cfg(test)]
    pub fn focused_fixture() -> Self {
        Self {
            draft: Some(Draft::default()),
            field: Some(Field::Name),
            ..default()
        }
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

const PRIVATE_TYPES: [Location; 6] = [
    Location::Home,
    Location::Field,
    Location::Mill,
    Location::Bakery,
    Location::Weavery,
    Location::Tailory,
];
const WEEKLY_TAX_TIME: &str = "Weekly taxes: Sunday at 04:00 | first collection Day 6";

fn type_name(location: Location) -> String {
    let plural = match location {
        Location::Bakery => "bakeries".into(),
        Location::Weavery => "weaveries".into(),
        Location::Tailory => "tailories".into(),
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
            if value.is_empty() {
                "[click to enter]"
            } else {
                value
            },
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

fn name(universe: &Universe, id: AgentId) -> String {
    universe
        .agents()
        .get(&id)
        .map_or_else(|| "Unknown citizen".into(), |agent| agent.name.clone())
}

fn owner_name(universe: &Universe, owner: GoodsOwner) -> String {
    match owner {
        GoodsOwner::Town => "Town".into(),
        GoodsOwner::Agent(id) => name(universe, id),
    }
}

fn place_owner(universe: &Universe, place: &learning_lord_simulation::locations::Place) -> String {
    if place.kind.is_town_owned() {
        "Town".into()
    } else {
        place
            .owner
            .map_or_else(|| "Public".into(), |id| name(universe, id))
    }
}

fn stocks(universe: &Universe, place: PlaceId) -> String {
    let mut lines = Vec::new();
    for (key, units) in universe
        .storage()
        .stock()
        .iter()
        .filter(|(key, _)| key.place == place)
    {
        lines.push(format!(
            "{} | {} | {} available | {} tax reserved",
            owner_name(universe, key.owner),
            key.good.name(),
            crate::quantity_label(
                key.good,
                units.saturating_sub(
                    universe
                        .storage()
                        .reserved_units(place, key.owner, key.good)
                )
            ),
            crate::quantity_label(
                key.good,
                universe
                    .storage()
                    .reserved_units(place, key.owner, key.good)
            )
        ));
    }
    for order in universe
        .market()
        .orders()
        .filter(|order| order.place == place)
    {
        lines.push(format!(
            "{} | {} | {} sale escrow",
            match order.seller {
                learning_lord_simulation::marketplace::MarketParty::Citizen(id) =>
                    name(universe, id),
                learning_lord_simulation::marketplace::MarketParty::Caravan => "Caravan".into(),
            },
            order.good.name(),
            crate::quantity_label(order.good, order.units)
        ));
    }
    lines.sort();
    if lines.is_empty() {
        "No stored goods or sale escrow.".into()
    } else {
        lines.join("\n")
    }
}

fn kind_label(kind: &TaxKind) -> &'static str {
    match kind {
        TaxKind::FlatFee { .. } => "Flat fee",
        TaxKind::Socage { .. } => "Socage",
        TaxKind::Asset { .. } => "Asset",
        TaxKind::Income { .. } => "Income",
    }
}

fn treasury(universe: &Universe) -> String {
    let mut lines = vec![format!("Town treasury: {} coins", universe.town_treasury())];
    for rule in universe.tax_rules().values() {
        for (id, _) in universe.agents() {
            let owed = universe.tax_arrears(rule.id, *id);
            if owed > 0 {
                lines.push(format!(
                    "{} owes {owed} coins | {}{}",
                    name(universe, *id),
                    rule.name,
                    if rule.active { "" } else { " (retired)" }
                ));
            }
        }
    }
    if lines.len() == 1 {
        lines.push("No coin arrears.".into());
    }
    for receipt in universe.tax_history().iter().rev().take(12) {
        let amount = match receipt.amount {
            TaxAmount::Coins {
                assessed,
                paid,
                arrears,
            } => format!("{assessed} coins assessed, {paid} paid, {arrears} owed"),
            TaxAmount::Reservation {
                assessed,
                reserved,
                outstanding,
            } => format!(
                "{}: {assessed} units assessed, {reserved} reserved, {outstanding} outstanding",
                receipt.good.map_or("Goods", |good| good.name())
            ),
            TaxAmount::Goods {
                assessed,
                collected,
                outstanding,
            } => format!(
                "{}: {assessed} units assessed, {collected} collected, {outstanding} owed",
                receipt.good.map_or("Goods", |good| good.name())
            ),
        };
        lines.push(format!(
            "{} | {} | {} | {} | {amount}",
            crate::format_clock(receipt.time_ms),
            universe
                .tax_rules()
                .get(&receipt.rule)
                .map_or("Retired rule", |rule| rule.name.as_str()),
            name(universe, receipt.payer),
            universe
                .map()
                .place(receipt.place)
                .map_or("Unknown location", |place| place.name.as_str())
        ));
    }
    lines.join("\n")
}

pub fn refresh(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    mut state: ResMut<State>,
    roots: Query<Entity, With<Content>>,
    panels: Query<(&ScrollPanel, &ScrollPosition)>,
    view: Res<market::Selection>,
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
        if saved && snapshot.0.mutation_error.is_none() {
            state.draft = None;
            state.field = None;
        }
        state.dirty = true;
    }
    if view.view != market::View::Locations {
        return;
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
                details.spawn(text(format!("Owner: {}", place_owner(universe, place)), 15.0, MUTED));
                details.spawn(text("STOCK BY OWNER", 15.0, MUTED));
                details.spawn(text(stocks(universe, place_id), 15.0, TEXT));
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
                        details.spawn(text(match &rule.kind {
                            TaxKind::FlatFee { payer, coins } => format!("{} coins weekly · {}", coins, match payer { TaxPayer::Agent(id) => name(universe, *id), TaxPayer::LocationOwner => "Property owner".into() }),
                            TaxKind::Socage { rates } | TaxKind::Asset { rates } | TaxKind::Income { rates } => {
                                let mut rates: Vec<_> = rates.iter().map(|(good, rate)| format!("{}: {:.2}%", good.name(), f64::from(rate.basis_points()) / 100.0)).collect(); rates.sort(); rates.join(" · ")
                            }
                        }, 14.0, MUTED));
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
                details.spawn(text(treasury(universe), 14.0, TEXT));
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

fn percentage(value: &str) -> Result<TaxRate, String> {
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
    snapshot: Res<DisplaySnapshot>,
    worker: Res<SimulationWorker>,
    view: Res<market::Selection>,
    buttons: Query<(&Interaction, &Choice), Changed<Interaction>>,
    mut keys: MessageReader<KeyboardInput>,
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
    if view.view != market::View::Locations {
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
                state.place = Some(place);
                state.location_type = None;
                state.draft = None;
                None
            }
            Choice::IndividualPlaces => {
                state.location_type = None;
                state.draft = None;
                None
            }
            Choice::LocationTypes | Choice::LocationType(_) => {
                state.location_type = Some(match *choice {
                    Choice::LocationType(kind) => kind,
                    _ => Location::Home,
                });
                state.draft = None;
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
                    owner_payer: state.location_type.is_some(),
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
            state.awaiting_mutation =
                Some((snapshot.0.mutation_revision, matches!(choice, Choice::Save)));
            if let Err(error) = worker.send(Command::Mutate {
                generation: snapshot.0.generation,
                mutation,
            }) {
                state.error = Some(error);
                state.awaiting_mutation = None;
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(rates.len(), 2);
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
        assert_eq!(goods, vec![Good::Wheat, Good::Flax]);
        let TaxKind::Socage { rates } = tax_kind(&draft, &goods).unwrap() else {
            panic!("expected socage");
        };
        assert_eq!(rates.len(), 2);
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
}
