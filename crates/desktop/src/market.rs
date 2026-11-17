use crate::{
    DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT,
    floating_ui::{FloatingWindow, WindowKind, Windows},
    text,
};
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
    ui::RelativeCursorPosition,
};
use learning_lord_simulation::{
    Universe,
    locations::PlaceId,
    marketplace::{DAY_MS, Good, MarketParty},
    production::Recipe,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tab {
    #[default]
    Overview,
    Listings,
    CaravanPolicy,
}

#[derive(Component)]
pub struct TabPanel(pub Tab);

#[derive(Resource, Default)]
pub struct Selection {
    pub(super) good: Option<Good>,
    pub(super) tab: Tab,
    pub(super) carpentry: bool,
}

impl Selection {
    pub(super) fn chart_window(&self) -> WindowKind {
        if self.carpentry {
            WindowKind::Carpentry
        } else {
            WindowKind::Good(self.good())
        }
    }

    pub(super) fn good(&self) -> Good {
        self.good.unwrap_or(Good::Berries)
    }
}

#[derive(Component, Clone, Copy)]
pub enum Choice {
    Good(Good),
    Tab(Tab),
    Carpentry,
}

#[derive(Component)]
pub enum Readout {
    GoodPrice(Good),
    Margin(Good),
    GoodChange(Good),
    CarpentryPrice,
    CarpentryChange,
    PriceNow,
    Unfulfilled,
}

#[derive(Component)]
pub struct Orders;

#[derive(Component)]
pub struct BuyerRows;

#[derive(Component)]
pub struct RecipeRows;

#[derive(Component, Default)]
pub(crate) struct MarketCache {
    orders: Vec<SellerOrders>,
    rows: Option<MarketRows>,
}

#[derive(Clone, Debug, PartialEq)]
struct SellerOrders {
    id: MarketParty,
    name: String,
    location: String,
    place: PlaceId,
    orders: Vec<(u64, u64, f64, bool)>,
    adjustment_rate: Option<u8>,
}

pub(crate) fn spawn_overview(parent: &mut ChildSpawnerCommands) {
    spawn_market(parent, (), Page::Market);
}

pub(crate) fn spawn_entity(parent: &mut ChildSpawnerCommands, good: Good) {
    spawn_market(
        parent,
        (
            GoodSelection(Selection {
                good: Some(good),
                ..default()
            }),
            crate::market_history::GoodChart::default(),
            MarketCache::default(),
        ),
        Page::Good(good),
    );
}

pub(crate) fn spawn_carpentry(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            GoodSelection(Selection {
                carpentry: true,
                ..default()
            }),
            crate::market_history::GoodChart::default(),
            Node {
                width: percent(100),
                min_height: px(0),
                padding: UiRect::all(px(16)),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            RelativeCursorPosition::default(),
            BackgroundColor(PANEL),
        ))
        .with_children(|content| {
            crate::market_history::spawn(content, "CARPENTRY HISTORY  |  recent 30 market days");
        });
}

#[derive(Component)]
pub(crate) struct GoodSelection(pub(crate) Selection);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Market,
    Good(Good),
}

pub(crate) fn pane_owner(
    entity: Entity,
    parents: &Query<&ChildOf>,
    contains: impl Fn(Entity) -> bool,
) -> Option<Entity> {
    parents
        .iter_ancestors(entity)
        .find(|&ancestor| contains(ancestor))
}

fn spawn_market(parent: &mut ChildSpawnerCommands, selection: impl Bundle, page: Page) {
    parent
        .spawn((
            selection,
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(14),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    flex_basis: px(0),
                    flex_grow: 1.0,
                    min_width: px(0),
                    min_height: px(0),
                    padding: UiRect::all(px(16)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|details| {
                details
                    .spawn(Node {
                        column_gap: px(8),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: px(6),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|tabs| {
                        for (tab, label) in [
                            (
                                Tab::Overview,
                                if page == Page::Market {
                                    "Goods and services"
                                } else {
                                    "Overview"
                                },
                            ),
                            (Tab::Listings, "Listings"),
                            (Tab::CaravanPolicy, "Caravan policy"),
                        ]
                        .into_iter()
                        .filter(|(tab, _)| page != Page::Market || *tab != Tab::Listings)
                        {
                            tabs.spawn((
                                Button,
                                Choice::Tab(tab),
                                BackgroundColor(PANEL),
                                Node {
                                    padding: UiRect::axes(px(12), px(8)),
                                    border_radius: BorderRadius::all(px(6)),
                                    ..default()
                                },
                            ))
                            .with_children(|button| {
                                button.spawn(text(label, 14.0, TEXT));
                            });
                        }
                    });
                for tab in [Tab::Overview, Tab::Listings, Tab::CaravanPolicy]
                    .into_iter()
                    .filter(|tab| page != Page::Market || *tab != Tab::Listings)
                {
                    details
                        .spawn((
                            TabPanel(tab),
                            ScrollPosition::default(),
                            RelativeCursorPosition::default(),
                            Node {
                                width: percent(100),
                                min_width: px(0),
                                flex_direction: FlexDirection::Column,
                                row_gap: if page == Page::Market && tab == Tab::Overview {
                                    px(0)
                                } else {
                                    px(12)
                                },
                                min_height: px(0),
                                flex_grow: 1.0,
                                flex_basis: px(0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                        ))
                        .with_children(|content| match tab {
                            Tab::Overview => {
                                if page == Page::Market {
                                    spawn_goods_and_services(content);
                                } else {
                                    content.spawn((text("", 16.0, TEXT), Readout::PriceNow));
                                    content.spawn((
                                        Node {
                                            flex_direction: FlexDirection::Column,
                                            row_gap: px(4),
                                            ..default()
                                        },
                                        RecipeRows,
                                    ));
                                    crate::market_history::spawn(
                                        content,
                                        "MARKET HISTORY  |  recent 30 market days",
                                    );
                                }
                            }
                            Tab::Listings => {
                                content.spawn(text("CURRENT BUY REQUESTS  |  live", 14.0, MUTED));
                                content.spawn((text("", 14.0, MUTED), Readout::Unfulfilled));
                                content.spawn((
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(4),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    BuyerRows,
                                ));
                                content.spawn(text("CURRENT SELL ORDERS  |  live", 14.0, MUTED));
                                content.spawn((
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(10),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    Orders,
                                ));
                            }
                            Tab::CaravanPolicy => match page {
                                Page::Market => crate::caravan_controls::spawn_global(content),
                                Page::Good(good) => {
                                    crate::caravan_controls::spawn_good(content, good)
                                }
                            },
                        });
                }
            });
        });
}

fn market_row() -> Node {
    Node {
        display: Display::Grid,
        width: percent(100),
        flex_shrink: 0.0,
        align_items: AlignItems::Center,
        grid_template_columns: vec![
            GridTrack::flex(1.4),
            GridTrack::flex(1.5),
            GridTrack::flex(0.8),
            GridTrack::flex(0.8),
        ],
        column_gap: px(8),
        padding: UiRect::axes(px(8), px(4)),
        ..default()
    }
}

fn spawn_goods_and_services(parent: &mut ChildSpawnerCommands) {
    parent.spawn(market_row()).with_children(|row| {
        for heading in ["Good or service", "Price", "Margin", "7-day change"] {
            row.spawn(text(heading, 13.0, MUTED));
        }
    });
    for good in Good::ALL {
        parent
            .spawn((
                Button,
                Choice::Good(good),
                BackgroundColor(PANEL),
                market_row(),
            ))
            .with_children(|row| {
                row.spawn(text(good.name(), 14.0, TEXT));
                row.spawn((text("", 13.0, TEXT), Readout::GoodPrice(good)));
                row.spawn((text("", 13.0, TEXT), Readout::Margin(good)));
                row.spawn((text("", 13.0, TEXT), Readout::GoodChange(good)));
            });
    }
    parent
        .spawn((
            Button,
            Choice::Carpentry,
            BackgroundColor(PANEL),
            market_row(),
        ))
        .with_children(|row| {
            row.spawn(text("Carpentry", 14.0, TEXT));
            row.spawn((text("", 13.0, TEXT), Readout::CarpentryPrice));
            row.spawn(text("-", 13.0, TEXT));
            row.spawn((text("", 13.0, TEXT), Readout::CarpentryChange));
        });
}

pub fn handle_selection(
    shortcuts: crate::KeyboardShortcuts,
    buttons: Query<(Entity, &Interaction, &Choice), Changed<Interaction>>,
    mut selection: ResMut<Selection>,
    mut focus: ResMut<InputFocus>,
    mut windows: Option<ResMut<Windows>>,
    mut panes: Query<&mut GoodSelection>,
    roots: Query<&FloatingWindow>,
    parents: Query<&ChildOf>,
) {
    let workspace = windows.is_some();
    for (entity, interaction, choice) in &buttons {
        if *interaction == Interaction::Pressed {
            focus.set(entity, FocusCause::Pressed);
            let kind = match *choice {
                Choice::Tab(tab) => {
                    let mut pane =
                        pane_owner(entity, &parents, |ancestor| panes.contains(ancestor))
                            .and_then(|owner| panes.get_mut(owner).ok());
                    let selection = pane
                        .as_deref_mut()
                        .map(|pane| &mut pane.0)
                        .unwrap_or(&mut selection);
                    selection.tab = tab;
                    if !workspace && tab != Tab::Overview {
                        selection.carpentry = false;
                    }
                    continue;
                }
                Choice::Good(good) => {
                    selection.carpentry = false;
                    selection.good = Some(good);
                    WindowKind::Good(good)
                }
                Choice::Carpentry => {
                    selection.carpentry = true;
                    if !workspace {
                        selection.tab = Tab::Overview;
                    }
                    WindowKind::Carpentry
                }
            };
            if let Some(windows) = windows.as_mut() {
                let source = parents
                    .iter_ancestors(entity)
                    .find_map(|root| roots.get(root).ok())
                    .map(|source| source.0);
                windows.open_link(source, kind);
            }
        }
    }
    if shortcuts.editing() {
        return;
    }
    for (key, kind) in [
        (KeyCode::KeyC, WindowKind::Citizens),
        (KeyCode::KeyM, WindowKind::Market),
        (KeyCode::KeyL, WindowKind::Locations),
    ] {
        if shortcuts.keyboard.just_pressed(key)
            && let Some(windows) = windows.as_mut()
        {
            windows.open(kind);
        }
    }
    let Some(windows) = windows.as_mut() else {
        return;
    };
    if !windows.is_open(WindowKind::Market) {
        return;
    }
    let current = match windows.focused() {
        WindowKind::Market => selection.chart_window(),
        focused => focused,
    };
    let index = match current {
        WindowKind::Good(good) => good as usize,
        WindowKind::Carpentry => Good::COUNT,
        _ => return,
    };
    let count = Good::COUNT + 1;
    let next = if shortcuts.keyboard.just_pressed(KeyCode::ArrowDown) {
        (index + 1) % count
    } else if shortcuts.keyboard.just_pressed(KeyCode::ArrowUp) {
        (index + count - 1) % count
    } else {
        return;
    };
    if let Some(&good) = Good::ALL.get(next) {
        selection.good = Some(good);
        selection.carpentry = false;
    } else {
        selection.carpentry = true;
    }
    windows.link(WindowKind::Market, selection.chart_window());
}

fn seller_orders(universe: &Universe, good: Good) -> Vec<SellerOrders> {
    let mut groups: Vec<SellerOrders> = Vec::new();
    for order in universe
        .market()
        .orders()
        .filter(|order| order.good == good)
    {
        let index = groups
            .iter()
            .position(|group| group.id == order.seller && group.place == order.place)
            .unwrap_or_else(|| {
                groups.push(SellerOrders {
                    id: order.seller,
                    name: match order.seller {
                        MarketParty::Caravan => {
                            if universe.market().effective_import_threshold(good).is_none() {
                                "Caravan (imports disabled)".into()
                            } else {
                                "Caravan".into()
                            }
                        }
                        MarketParty::Citizen(id) | MarketParty::Household(id) => {
                            crate::locations::name(universe, id)
                        }
                    },
                    location: universe
                        .map()
                        .place(order.place)
                        .map_or_else(|_| "Unknown location".into(), |place| place.name.clone()),
                    place: order.place,
                    orders: Vec::new(),
                    adjustment_rate: order
                        .seller
                        .account()
                        .and_then(|account| universe.market().seller_adjustment_rate(account)),
                });
                groups.len() - 1
            });
        groups[index].orders.push((
            order.id.0,
            order.units,
            order.quoted_price,
            universe.market().order_is_trial(order.id),
        ));
    }
    for group in &mut groups {
        group.orders.sort_by_key(|order| order.0);
    }
    groups.sort_by(|a, b| {
        a.name.cmp(&b.name).then_with(|| {
            a.id.account()
                .map(|id| id.0)
                .cmp(&b.id.account().map(|id| id.0))
        })
    });
    groups
}

fn raw_material_margin(universe: &Universe, recipe: Recipe) -> Option<f64> {
    if recipe.inputs().is_empty() {
        return None;
    }
    let output: f64 = recipe
        .outputs()
        .iter()
        .map(|&(good, units)| universe.prices().value(good, units).unwrap())
        .sum();
    let materials: f64 = recipe
        .inputs()
        .iter()
        .map(|&(good, units)| universe.prices().value(good, units).unwrap())
        .sum();
    (output > 0.0).then_some((output - materials) / output * 100.0)
}

fn recipe_margins(universe: &Universe, good: Good) -> impl Iterator<Item = (Recipe, f64)> + '_ {
    Recipe::ALL
        .into_iter()
        .filter(move |recipe| recipe.outputs().iter().any(|&(output, _)| output == good))
        .filter_map(move |recipe| {
            raw_material_margin(universe, recipe).map(|margin| (recipe, margin))
        })
}

fn material_margins(universe: &Universe, good: Good) -> String {
    let rows: Vec<_> = recipe_margins(universe, good)
        .map(|(recipe, margin)| format!("{}: {margin:.1}%", recipe.name()))
        .collect();
    if rows.is_empty() {
        return String::new();
    }
    format!("RAW MATERIAL MARGIN\n{}", rows.join("\n"))
}

fn margin_range(universe: &Universe, good: Good) -> String {
    let mut margins = recipe_margins(universe, good).map(|(_, margin)| margin.round() + 0.0);
    let Some(first) = margins.next() else {
        return "-".into();
    };
    let (min, max) = margins.fold((first, first), |(min, max), margin| {
        (min.min(margin), max.max(margin))
    });
    if min == max {
        format!("{min:.0}%")
    } else {
        format!("{min:.0}% to {max:.0}%")
    }
}

fn weekly_change(current: Option<f64>, previous: Option<f64>) -> String {
    let (Some(current), Some(previous)) = (current, previous.filter(|price| *price > 0.0)) else {
        return "-".into();
    };
    let change = ((current / previous - 1.0) * 100.0).round();
    if change == 0.0 {
        "0%".into()
    } else {
        format!("{change:+.0}%")
    }
}

fn good_weekly_change(universe: &Universe, good: Good) -> String {
    let current = universe.prices().price(good);
    let previous = universe
        .current_time_ms()
        .checked_sub(7 * DAY_MS)
        .and_then(|time| {
            let market = universe.market();
            let (_, created) = market.initial_period();
            if time < created {
                return None;
            }
            market
                .history()
                .iter()
                .rev()
                .take_while(|entry| entry.end_ms > time)
                .filter(|entry| entry.good == good)
                .fold(current, |_, entry| Some(entry.price_before))
        });
    weekly_change(current, previous)
}

fn order_label(good: Good, units: u64, price: f64) -> String {
    format!(
        "{} remaining  |  {price:.3} coins/{}  |  {:.3} coins quoted value",
        crate::quantity_label(good, units),
        good.price_unit_name(),
        units as f64 / good.units_per_price_unit() as f64 * price
    )
}

#[cfg(test)]
pub(crate) fn buyer_requests(universe: &Universe, good: Good) -> String {
    let market = universe.market();
    let mut buyers: Vec<_> = universe
        .households()
        .iter()
        .filter_map(|(id, agent)| {
            let requested = market.requested(*id).units(good);
            (requested > 0).then(|| {
                (
                    *id,
                    &agent.name,
                    requested,
                    market.affordable_request(*id).units(good),
                )
            })
        })
        .collect();
    buyers.sort_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.0.cmp(&b.0.0)));
    if buyers.is_empty() {
        return "No current buy requests for this good.".into();
    }
    buyers
        .iter()
        .map(|(id, name, requested, affordable)| {
            let identity = if buyers.iter().filter(|buyer| buyer.1 == *name).count() > 1 {
                format!("{name}\nBuyer {}", id.0)
            } else {
                (*name).clone()
            };
            format!(
                "{identity}\n{} requested  |  {} affordable",
                crate::quantity_label(good, *requested),
                crate::quantity_label(good, *affordable)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn price_details(universe: &Universe, good: Good) -> String {
    let market = universe.market();
    let policy = universe.caravan_policy();
    if !good.caravan_eligible() {
        return format!(
            "Reference estimate: {:.3} coins/{}\nCore price: {:.3} coins/{}\nCaravan trade unavailable\nTown stock: {}",
            universe.prices().price(good).unwrap(),
            good.price_unit_name(),
            good.core_price(),
            good.price_unit_name(),
            crate::quantity_label(good, universe.town_stock(good))
        );
    }
    let export = if !policy.exports[good as usize].allowed
        || market.effective_export_threshold(good).is_none()
    {
        "Exports disabled".into()
    } else {
        format!(
            "Export below {:.3} coins/{}",
            market.effective_export_threshold(good).unwrap_or_default(),
            good.price_unit_name()
        )
    };
    let import = if market.effective_import_threshold(good).is_none() {
        "Imports disabled".into()
    } else {
        format!(
            "Import ask {:.3} coins/{}",
            market.effective_import_threshold(good).unwrap_or_default(),
            good.price_unit_name()
        )
    };
    let ask = if market.effective_import_threshold(good).is_none() {
        "disabled".into()
    } else {
        format!(
            "{:.3} coins/{}",
            market.effective_import_threshold(good).unwrap_or_default(),
            good.price_unit_name()
        )
    };
    format!(
        "Reference estimate: {:.3} coins/{}\nCore price: {:.3} coins/{}\n{} | {}\nCaravan asking price: {}\nTown stock: {} | Saved minimum reserve: {}",
        universe.prices().price(good).unwrap(),
        good.price_unit_name(),
        good.core_price(),
        good.price_unit_name(),
        export,
        import,
        ask,
        crate::quantity_label(good, universe.town_stock(good)),
        crate::quantity_label(good, policy.exports[good as usize].minimum_reserve)
    )
}

pub fn refresh_choices(
    selection: Res<Selection>,
    mut buttons: Query<(Entity, &Choice, &Interaction, &mut BackgroundColor)>,
    panes: Query<&GoodSelection>,
    parents: Query<&ChildOf>,
) {
    for (entity, choice, interaction, mut color) in &mut buttons {
        let pane = pane_owner(entity, &parents, |ancestor| panes.contains(ancestor));
        let selection = pane
            .and_then(|owner| panes.get(owner).ok())
            .map(|pane| &pane.0)
            .unwrap_or(&selection);
        let selected = match *choice {
            Choice::Good(good) => !selection.carpentry && selection.good == Some(good),
            Choice::Carpentry => selection.carpentry,
            Choice::Tab(tab) => selection.tab == tab,
        };
        let value = if *interaction == Interaction::Pressed {
            Color::srgb(0.18, 0.48, 0.39)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.20, 0.27, 0.29)
        } else if selected {
            SELECTED
        } else {
            PANEL
        };
        color.set_if_neq(BackgroundColor(value));
    }
}

type MarketRows = (
    Vec<(learning_lord_simulation::AgentId, String, u64, u64)>,
    String,
);
type RowPanels<'w, 's> = (
    Query<'w, 's, Entity, With<Orders>>,
    Query<'w, 's, Entity, With<BuyerRows>>,
    Query<'w, 's, Entity, With<RecipeRows>>,
);

pub fn refresh(
    mut commands: Commands,
    (snapshot, overview): (Res<DisplaySnapshot>, Res<Selection>),
    mut panes: Query<(Entity, Ref<GoodSelection>, &mut MarketCache)>,
    mut tab_panels: Query<(Entity, &TabPanel, &mut Node, &mut Visibility)>,
    mut readouts: Query<(Entity, Ref<Readout>, &mut Text)>,
    (orders, buyers, recipes): RowPanels,
    parents: Query<&ChildOf>,
    roots: Query<&FloatingWindow>,
    windows: Option<Res<Windows>>,
    mut last_visible: Local<[Option<WindowKind>; 2]>,
) {
    let previous = *last_visible;
    if let Some(windows) = &windows {
        *last_visible = windows.visible();
    }
    let owner = |entity| pane_owner(entity, &parents, |ancestor| panes.contains(ancestor));
    for (entity, panel, mut node, mut visibility) in &mut tab_panels {
        let pane = owner(entity).and_then(|owner| panes.get(owner).ok());
        let active = panel.0
            == pane
                .as_ref()
                .map_or(overview.tab, |(_, selection, _)| selection.0.tab);
        let display = if active { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        visibility.set_if_neq(if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    let universe = &snapshot.0.universe;
    for (entity, readout, mut value) in &mut readouts {
        let root = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| roots.get(ancestor).ok());
        let restored = if let (Some(windows), Some(root)) = (&windows, root) {
            if !windows.is_open(root.0) {
                continue;
            }
            !previous.contains(&Some(root.0))
        } else {
            false
        };
        if !snapshot.is_changed() && !readout.is_added() && !restored {
            continue;
        }
        let next = match *readout {
            Readout::GoodPrice(good) => format!(
                "{:.3} coins/{}",
                universe.prices().price(good).unwrap(),
                good.price_unit_name()
            ),
            Readout::Margin(good) => margin_range(universe, good),
            Readout::GoodChange(good) => good_weekly_change(universe, good),
            Readout::CarpentryPrice => universe
                .construction()
                .carpentry_rate()
                .map_or_else(|| "-".into(), |rate| format!("{rate:.3} coins/actual hour")),
            Readout::CarpentryChange => {
                let construction = universe.construction();
                let previous = universe
                    .current_time_ms()
                    .checked_sub(7 * DAY_MS)
                    .and_then(|time| construction.carpentry_rate_at(time));
                weekly_change(construction.carpentry_rate(), previous)
            }
            Readout::PriceNow | Readout::Unfulfilled => {
                let Some((_, selection, _)) = owner(entity).and_then(|owner| panes.get(owner).ok())
                else {
                    continue;
                };
                let good = selection.0.good();
                match *readout {
                    Readout::PriceNow => price_details(universe, good),
                    _ => format!(
                        "Affordable unfulfilled demand: {}",
                        crate::quantity_label(
                            good,
                            universe.market().current_period().goods[good as usize]
                                .affordable_demand_units
                        )
                    ),
                }
            }
        };
        value.set_if_neq(Text(next));
    }
    for (entity, selection, mut cache) in &mut panes {
        let added = selection.is_added();
        let good = selection.0.good();
        let kind = WindowKind::Good(good);
        if windows
            .as_ref()
            .is_some_and(|windows| !windows.is_open(kind))
        {
            continue;
        }
        let restored = windows.is_some() && !previous.contains(&Some(kind));
        if !snapshot.is_changed() && !added && !restored {
            continue;
        }
        let belongs = |child| {
            parents
                .iter_ancestors(child)
                .any(|ancestor| ancestor == entity)
        };
        refresh_rows(
            &mut commands,
            universe,
            good,
            &mut cache,
            added,
            orders.iter().filter(|&entity| belongs(entity)),
            buyers.iter().filter(|&entity| belongs(entity)),
            recipes.iter().filter(|&entity| belongs(entity)),
        );
    }
}

fn refresh_rows(
    commands: &mut Commands,
    universe: &Universe,
    good: Good,
    cache: &mut MarketCache,
    added: bool,
    orders: impl Iterator<Item = Entity>,
    buyers: impl Iterator<Item = Entity>,
    recipes: impl Iterator<Item = Entity>,
) {
    let buyer_key = universe
        .households()
        .iter()
        .filter_map(|(id, agent)| {
            let units = universe.market().requested(*id).units(good);
            (units > 0).then(|| {
                (
                    *id,
                    agent.name.clone(),
                    units,
                    universe.market().affordable_request(*id).units(good),
                )
            })
        })
        .collect();
    let row_key = (buyer_key, material_margins(universe, good));
    if cache.rows.as_ref() != Some(&row_key) || added {
        for entity in buyers {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|content| {
                    let mut requested: Vec<_> = universe
                        .households()
                        .iter()
                        .filter(|(id, _)| universe.market().requested(**id).units(good) > 0)
                        .collect();
                    requested
                        .sort_by(|a, b| a.1.name.cmp(&b.1.name).then_with(|| a.0.0.cmp(&b.0.0)));
                    if requested.is_empty() {
                        content.spawn(text("No current buy requests for this good.", 14.0, TEXT));
                    }
                    for (id, agent) in &requested {
                        crate::navigation::link(
                            content,
                            &agent.name,
                            crate::navigation::Target::Household(**id),
                        );
                        if requested
                            .iter()
                            .filter(|(_, other)| other.name == agent.name)
                            .count()
                            > 1
                        {
                            content.spawn(text(format!("Buyer {}", id.0), 12.0, MUTED));
                        }
                        content.spawn(text(
                            format!(
                                "{} requested  |  {} affordable",
                                crate::quantity_label(
                                    good,
                                    universe.market().requested(**id).units(good)
                                ),
                                crate::quantity_label(
                                    good,
                                    universe.market().affordable_request(**id).units(good)
                                )
                            ),
                            14.0,
                            TEXT,
                        ));
                    }
                });
        }
        for entity in recipes {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|content| {
                    content.spawn(text(&row_key.1, 14.0, MUTED));
                    for recipe in Recipe::ALL
                        .into_iter()
                        .filter(|recipe| !recipe.inputs().is_empty())
                        .filter(|recipe| recipe.outputs().iter().any(|&(output, _)| output == good))
                    {
                        content
                            .spawn(Node {
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: px(6),
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn(text(format!("{}:", recipe.name()), 14.0, MUTED));
                                for (index, &(input, units)) in recipe.inputs().iter().enumerate() {
                                    if index > 0 {
                                        row.spawn(text("+", 14.0, MUTED));
                                    }
                                    crate::navigation::link(
                                        row,
                                        format!(
                                            "{} {}",
                                            crate::quantity_label(input, units),
                                            input.name()
                                        ),
                                        crate::navigation::Target::Good(input),
                                    );
                                }
                                row.spawn(text("->", 14.0, MUTED));
                                for (index, &(output, units)) in recipe.outputs().iter().enumerate()
                                {
                                    if index > 0 {
                                        row.spawn(text("+", 14.0, MUTED));
                                    }
                                    crate::navigation::link(
                                        row,
                                        format!(
                                            "{} {}",
                                            crate::quantity_label(output, units),
                                            output.name()
                                        ),
                                        crate::navigation::Target::Good(output),
                                    );
                                }
                            });
                    }
                });
        }
        cache.rows = Some(row_key);
    }
    let groups = seller_orders(universe, good);
    if groups != cache.orders || added {
        for entity in orders {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|content| {
                    if groups.is_empty() {
                        content.spawn(text("No current sell orders for this good.", 14.0, MUTED));
                    }
                    let mut ladder: Vec<_> = groups
                        .iter()
                        .flat_map(|group| group.orders.iter())
                        .collect();
                    ladder.sort_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
                    let mut cumulative = 0u64;
                    let supply = ladder
                        .into_iter()
                        .map(|order| {
                            cumulative = cumulative.saturating_add(order.1);
                            format!(
                                "{:.3} coins/{} → {} cumulative",
                                order.2,
                                good.price_unit_name(),
                                crate::quantity_label(good, cumulative)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !supply.is_empty() {
                        content.spawn(text(
                            format!("AVAILABLE SUPPLY BY ASK\n{supply}"),
                            13.0,
                            MUTED,
                        ));
                    }
                    for group in &groups {
                        content
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(4),
                                flex_shrink: 0.0,
                                ..default()
                            })
                            .with_children(|seller| {
                                match group.id {
                                    MarketParty::Citizen(id) => crate::navigation::link(
                                        seller,
                                        &group.name,
                                        crate::navigation::Target::Citizen(id),
                                    ),
                                    MarketParty::Household(id) => crate::navigation::link(
                                        seller,
                                        &group.name,
                                        crate::navigation::Target::Household(id),
                                    ),
                                    MarketParty::Caravan => {
                                        seller.spawn(text(&group.name, 16.0, TEXT));
                                    }
                                }
                                crate::navigation::link(
                                    seller,
                                    &group.location,
                                    crate::navigation::Target::Location(group.place),
                                );
                                if let Some(id) = group.id.account()
                                    && groups
                                        .iter()
                                        .filter(|other| other.name == group.name)
                                        .count()
                                        > 1
                                {
                                    seller.spawn(text(format!("Seller {}", id.0), 12.0, MUTED));
                                }
                                if let Some(rate) = group.adjustment_rate {
                                    seller.spawn(text(
                                        format!("Price adjustment rate: {rate}%"),
                                        13.0,
                                        MUTED,
                                    ));
                                }
                                for &(_, units, price, trial) in &group.orders {
                                    if trial {
                                        seller.spawn(text(
                                            "CANARY • locked speculative price",
                                            13.0,
                                            Color::srgb(0.96, 0.70, 0.35),
                                        ));
                                    } else if group.id != MarketParty::Caravan {
                                        seller.spawn(text("Ordinary listing", 12.0, MUTED));
                                    }
                                    seller.spawn(text(
                                        order_label(good, units, price),
                                        14.0,
                                        MUTED,
                                    ));
                                }
                            });
                    }
                });
        }
        cache.orders = groups;
    }
}

#[cfg(test)]
fn order_text(universe: &Universe, good: Good) -> String {
    let groups = seller_orders(universe, good);
    if groups.is_empty() {
        return "No current sell orders for this good.".into();
    }
    groups
        .into_iter()
        .map(|group| {
            format!(
                "{} · {}\n{}",
                group.name,
                group.location,
                group
                    .orders
                    .into_iter()
                    .map(|(_, units, price, _)| order_label(good, units, price))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{self, Snapshot};
    use learning_lord_simulation::{
        Citizen, CitizenAction, TRADE_DURATION_MS, locations::Map, marketplace::Prices,
    };

    #[test]
    fn carpentry_row_opens_its_chart_beside_the_market_table() {
        use crate::market_history::{self, Detail, Plot};

        let mut app = app();
        app.init_resource::<market_history::ChartDisplay>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(
                Update,
                (market_history::refresh, market_history::hover).chain(),
            );
        press_in(&mut app, WindowKind::Market, Choice::Carpentry);
        assert_eq!(
            app.world().resource::<Windows>().visible(),
            [Some(WindowKind::Market), Some(WindowKind::Carpentry)]
        );
        let world = app.world_mut();
        let market = root(world, WindowKind::Market);
        let carpentry = root(world, WindowKind::Carpentry);
        let mut entities = Vec::new();
        descendants(world, world.get::<Children>(market).unwrap(), &mut entities);
        assert!(entities.iter().any(|&entity| {
            matches!(world.get::<TabPanel>(entity), Some(TabPanel(Tab::Overview)))
                && world.get::<Node>(entity).unwrap().display == Display::Flex
        }));
        assert!(entities.iter().any(|&entity| {
            matches!(world.get::<Readout>(entity), Some(Readout::CarpentryPrice))
                && world.get::<Text>(entity).unwrap().0 == "-"
        }));
        entities.clear();
        descendants(
            world,
            world.get::<Children>(carpentry).unwrap(),
            &mut entities,
        );
        let plot = *entities
            .iter()
            .find(|&&entity| world.get::<Plot>(entity).is_some())
            .unwrap();
        let readout = *entities
            .iter()
            .find(|&&entity| world.get::<Detail>(entity).is_some())
            .unwrap();
        let mut cursor = world.get_mut::<RelativeCursorPosition>(plot).unwrap();
        cursor.normalized = Some(Vec2::new(-0.49, 0.0));
        cursor.cursor_over = true;
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let world = app.world_mut();
        *world.get_mut::<RelativeCursorPosition>(plot).unwrap() = RelativeCursorPosition::default();
        world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        app.update();
        let pinned = app.world().get::<Text>(readout).unwrap().0.clone();
        assert!(pinned.starts_with("PINNED"));
        assert!(pinned.contains("Actual work:"));

        press_in(
            &mut app,
            WindowKind::Market,
            Choice::Tab(Tab::CaravanPolicy),
        );
        press_in(&mut app, WindowKind::Market, Choice::Tab(Tab::Overview));
        press_in(&mut app, WindowKind::Market, Choice::Good(Good::BreadRoll));
        assert_eq!(
            app.world().resource::<Windows>().visible(),
            [
                Some(WindowKind::Market),
                Some(WindowKind::Good(Good::BreadRoll))
            ]
        );
        let world = app.world_mut();
        let bread = root(world, WindowKind::Good(Good::BreadRoll));
        entities.clear();
        descendants(world, world.get::<Children>(bread).unwrap(), &mut entities);
        let bread_detail = entities
            .iter()
            .find(|&&entity| world.get::<Detail>(entity).is_some())
            .unwrap();
        assert!(
            !world
                .get::<Text>(*bread_detail)
                .unwrap()
                .0
                .starts_with("PINNED")
        );
        press_in(&mut app, WindowKind::Market, Choice::Carpentry);
        assert_eq!(app.world().get::<Text>(readout).unwrap().0, pinned);
    }

    #[test]
    fn market_keyboard_wraps_through_carpentry_from_both_panes() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<Windows>()
            .open(WindowKind::Market);
        app.update();
        for (key, expected) in [
            (KeyCode::ArrowUp, WindowKind::Carpentry),
            (
                KeyCode::ArrowUp,
                WindowKind::Good(Good::ALL[Good::COUNT - 1]),
            ),
            (KeyCode::ArrowDown, WindowKind::Carpentry),
            (KeyCode::ArrowDown, WindowKind::Good(Good::ALL[0])),
        ] {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.reset_all();
            keyboard.press(key);
            app.update();
            let windows = app.world().resource::<Windows>();
            assert_eq!(
                windows.visible(),
                [Some(WindowKind::Market), Some(expected)]
            );
            assert_eq!(windows.focused(), expected);
        }
    }

    #[test]
    fn market_table_uses_whole_margins_and_a_seven_day_reference_price() {
        use learning_lord_simulation::marketplace::ShoppingList;

        let prices = Prices::default()
            .with_price(Good::Wheat, 120.0)
            .unwrap()
            .with_price(Good::Flour, 100.0)
            .unwrap()
            .with_price(Good::Water, 100.0)
            .unwrap();
        let universe = Universe::with_map(Map::default()).with_prices(prices);
        assert_eq!(good_weekly_change(&universe, Good::Berries), "-");
        assert_eq!(margin_range(&universe, Good::Wheat), "-");
        assert_eq!(margin_range(&universe, Good::Flour), "-50%");

        let idle = universe.advance(7 * DAY_MS).unwrap();
        assert!(idle.market().history().is_empty());
        assert_eq!(good_weekly_change(&idle, Good::Water), "0%");

        let (universe, buyer) = universe
            .with_citizen(
                "Buyer",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(1000)
                    .unwrap(),
            )
            .unwrap();
        let universe = universe
            .advance(2 * DAY_MS)
            .unwrap()
            .with_purchase_request(buyer, ShoppingList::single(Good::Water, 100))
            .unwrap()
            .advance(5 * DAY_MS)
            .unwrap()
            .with_prices(prices.with_price(Good::Water, 125.0).unwrap());
        assert!(
            universe
                .market()
                .history()
                .iter()
                .any(|entry| entry.good == Good::Water)
        );
        assert!(
            universe
                .market()
                .history()
                .iter()
                .all(|entry| entry.start_ms > 0)
        );
        assert_eq!(good_weekly_change(&universe, Good::Water), "+25%");
        assert_eq!(weekly_change(Some(3.0), Some(0.0)), "-");
        assert_eq!(weekly_change(Some(3.0), Some(3.0)), "0%");
        assert_eq!(weekly_change(Some(3.0), Some(4.0)), "-25%");
    }

    #[test]
    fn good_tabs_and_live_readouts_belong_to_each_pane() {
        let mut app = app();
        app.world_mut().resource_mut::<Windows>().link(
            WindowKind::Good(Good::Berries),
            WindowKind::Good(Good::BreadRoll),
        );
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = Universe::default()
            .with_prices(Prices::default().with_price(Good::BreadRoll, 9.0).unwrap());
        app.update();

        press_in(
            &mut app,
            WindowKind::Good(Good::Berries),
            Choice::Tab(Tab::Listings),
        );
        press_in(
            &mut app,
            WindowKind::Good(Good::BreadRoll),
            Choice::Tab(Tab::CaravanPolicy),
        );
        let world = app.world_mut();
        let panes = world
            .query::<(&GoodSelection, &Children)>()
            .iter(world)
            .map(|(pane, children)| {
                let good = pane.0.good();
                let mut tabs = Vec::new();
                descendants(world, children, &mut tabs);
                let active = tabs
                    .iter()
                    .find_map(|entity| {
                        let panel = world.get::<TabPanel>(*entity)?;
                        (world.get::<Node>(*entity)?.display == Display::Flex).then_some(panel.0)
                    })
                    .unwrap();
                (good, active, tabs)
            })
            .collect::<Vec<_>>();
        assert_eq!(panes.len(), 2);
        assert!(
            panes
                .iter()
                .any(|(good, tab, _)| *good == Good::Berries && *tab == Tab::Listings)
        );
        let (_, tab, tabs) = panes
            .iter()
            .find(|(good, _, _)| *good == Good::BreadRoll)
            .unwrap();
        assert_eq!(*tab, Tab::CaravanPolicy);
        assert!(tabs.iter().any(|&entity| {
            matches!(world.get::<Readout>(entity), Some(Readout::PriceNow))
                && world
                    .get::<Text>(entity)
                    .unwrap()
                    .0
                    .contains("Reference estimate: 9.000 coins/roll")
        }));
    }

    #[test]
    fn displaced_good_catches_up_when_reopened_while_paused() {
        let mut app = app();
        let kind = WindowKind::Good(Good::Berries);
        let readout = {
            let world = app.world_mut();
            let root = root(world, kind);
            let mut entities = Vec::new();
            descendants(world, world.get::<Children>(root).unwrap(), &mut entities);
            entities
                .into_iter()
                .find(|&entity| matches!(world.get::<Readout>(entity), Some(Readout::PriceNow)))
                .unwrap()
        };

        press_in(&mut app, WindowKind::Market, Choice::Good(Good::BreadRoll));
        let old = app.world().get::<Text>(readout).unwrap().0.clone();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = Universe::default()
            .with_prices(Prices::default().with_price(Good::Berries, 9.0).unwrap());
        app.update();
        assert_eq!(app.world().get::<Text>(readout).unwrap().0, old);

        press_in(&mut app, WindowKind::Market, Choice::Good(Good::Berries));
        assert!(
            app.world()
                .get::<Text>(readout)
                .unwrap()
                .0
                .contains("Reference estimate: 9.000 coins/kg")
        );
    }

    fn descendants(world: &World, children: &Children, entities: &mut Vec<Entity>) {
        for &child in children {
            entities.push(child);
            if let Some(children) = world.get::<Children>(child) {
                descendants(world, children, entities);
            }
        }
    }

    #[test]
    fn universal_goods_show_reference_prices_and_unavailable_caravan_trade() {
        for good in [Good::Berries, Good::Water] {
            let label = price_details(&Universe::default(), good);
            assert!(label.contains("Reference estimate:"));
            assert!(label.contains("Core price:"));
            assert!(label.contains("Caravan trade unavailable"));
            assert!(!label.contains("Export below"));
            assert!(!label.contains("Import above"));
        }
    }

    #[test]
    fn effective_thresholds_stock_and_disabled_trade_are_finite_labels() {
        let mut policy = Universe::default().caravan_policy();
        policy.import_tariff_basis_points = 5000;
        policy.export_tariff_basis_points = 5000;
        policy.exports[Good::BreadRoll as usize].minimum_reserve = 7;
        let universe = Universe::default().with_caravan_policy(policy).unwrap();
        let label = price_details(&universe, Good::BreadRoll);
        assert!(label.contains("Export below 1.003 coins/roll | Import ask 12.033"));
        assert!(label.contains("Town stock: 0 rolls | Saved minimum reserve: 7 rolls"));
        policy.import_tariff_basis_points = 10_000;
        policy.export_tariff_basis_points = 10_000;
        let label = price_details(
            &universe.with_caravan_policy(policy).unwrap(),
            Good::BreadRoll,
        );
        assert!(label.contains("Exports disabled | Imports disabled"));
        assert!(label.contains("Caravan asking price: disabled"));
        assert!(!label.to_lowercase().contains("inf"));
        assert!(!label.contains("NaN"));
    }

    #[test]
    fn core_threshold_readout_stays_fixed_when_local_prices_change() {
        let universe = Universe::default()
            .with_prices(Prices::default().with_price(Good::BreadRoll, 1.0).unwrap());
        let label = price_details(&universe, Good::BreadRoll);
        assert!(label.contains("Reference estimate: 1.000 coins/roll"));
        assert!(label.contains("Core price: 4.011 coins/roll"));
        assert!(label.contains("Export below 2.006 coins/roll | Import ask 6.017 coins/roll"));
        assert!(label.contains("Caravan asking price: 6.017 coins/roll"));
    }

    #[test]
    fn caravan_orders_have_external_identity_and_fixed_asks() {
        use learning_lord_simulation::marketplace::{ShoppingList, UPDATE_TIME_MS};
        let (universe, buyer) = Universe::default()
            .with_prices(
                Prices::default()
                    .with_price(Good::BreadRoll, Good::BreadRoll.core_price() * 2.0)
                    .unwrap(),
            )
            .with_citizen(
                "Caravan",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(1000)
                    .unwrap(),
            )
            .unwrap();
        let universe = universe
            .with_purchase_request(buyer, ShoppingList::single(Good::BreadRoll, 6))
            .unwrap()
            .advance(UPDATE_TIME_MS)
            .unwrap();
        let groups = seller_orders(&universe, Good::BreadRoll);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, MarketParty::Caravan);
        assert_eq!(groups[0].name, "Caravan");
        assert_eq!(groups[0].orders[0].1, 3);
        assert_eq!(groups[0].orders[0].2, Good::BreadRoll.import_threshold());
        assert_eq!(universe.agents().len(), 1);
    }

    #[test]
    fn material_margin_uses_reference_quantities_and_keeps_losses() {
        use learning_lord_simulation::marketplace::Prices;
        let prices = Prices::default()
            .with_price(Good::Wheat, 40.0)
            .unwrap()
            .with_price(Good::Flour, 100.0)
            .unwrap();
        let universe = Universe::default().with_prices(prices);
        assert_eq!(
            raw_material_margin(&universe, Recipe::MillFlour),
            Some(50.0)
        );
        assert_eq!(raw_material_margin(&universe, Recipe::GrowWheat), None);
        assert!(material_margins(&universe, Good::Wheat).is_empty());
        assert!(material_margins(&universe, Good::Flour).contains("Mill flour: 50.0%"));
        let universe = universe.with_prices(prices.with_price(Good::Wheat, 120.0).unwrap());
        assert_eq!(
            raw_material_margin(&universe, Recipe::MillFlour),
            Some(-50.0)
        );
    }

    #[test]
    fn material_margin_ignores_socage_rules() {
        use learning_lord_simulation::{
            locations::Location,
            taxation::{TaxKind, TaxRate, TaxScope},
        };
        let universe = Universe::default();
        let before = raw_material_margin(&universe, Recipe::MillFlour);
        let (universe, _) = universe
            .with_scoped_tax_rule(
                TaxScope::LocationType(Location::Mill),
                "Socage",
                TaxKind::Socage {
                    rates: [(Good::Flour, TaxRate::new(9000).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        assert_eq!(raw_material_margin(&universe, Recipe::MillFlour), before);
    }

    #[test]
    fn counted_goods_display_integer_quantities_and_item_prices() {
        assert_eq!(
            order_label(Good::BreadRoll, 2, 15.0),
            "2 rolls remaining  |  15.000 coins/roll  |  30.000 coins quoted value"
        );
        assert_eq!(
            order_label(Good::BerryPie, 1, 25.0),
            "1 pie remaining  |  25.000 coins/pie  |  25.000 coins quoted value"
        );
    }

    #[test]
    fn buy_requests_include_cashless_buyers_and_stay_live() {
        use learning_lord_simulation::marketplace::ShoppingList;

        let (universe, funded) = Universe::with_map(Map::default())
            .with_prices(Prices::new(10.0).unwrap())
            .with_citizen(
                "Same",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(1)
                    .unwrap(),
            )
            .unwrap();
        let (universe, cashless) = universe
            .with_citizen(
                "Same",
                Citizen::new(0.0).unwrap().with_furnished_home().unwrap(),
            )
            .unwrap();
        let (universe, water_buyer) = universe
            .with_citizen(
                "Water buyer",
                Citizen::new(0.0).unwrap().with_furnished_home().unwrap(),
            )
            .unwrap();
        let universe = universe
            .with_purchase_request(funded, ShoppingList::single(Good::Berries, 200))
            .unwrap()
            .with_purchase_request(cashless, ShoppingList::single(Good::Berries, 300))
            .unwrap()
            .with_purchase_request(water_buyer, ShoppingList::single(Good::Water, 400))
            .unwrap();
        let mut expected = [
            (
                universe.economic_id(funded).unwrap(),
                "200 g requested  |  100 g affordable",
            ),
            (
                universe.economic_id(cashless).unwrap(),
                "300 g requested  |  0 g affordable",
            ),
        ];
        expected.sort_by_key(|(id, _)| id.0);
        let expected = expected
            .iter()
            .map(|(id, quantities)| format!("Same\nBuyer {}\n{quantities}", id.0))
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_eq!(buyer_requests(&universe, Good::Berries), expected);
        assert_eq!(
            buyer_requests(&universe, Good::BreadRoll),
            "No current buy requests for this good."
        );

        let mut app = app();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = universe.clone();
        app.update();
        let displayed = |app: &mut App, good: Good| {
            fn labels(world: &World, entity: Entity, values: &mut Vec<String>) {
                if let Some(value) = world.get::<Text>(entity) {
                    values.push(value.0.clone());
                }
                if let Some(children) = world.get::<Children>(entity) {
                    for child in children {
                        labels(world, *child, values);
                    }
                }
            }
            let world = app.world_mut();
            let root = root(world, WindowKind::Good(good));
            let mut entities = Vec::new();
            descendants(world, world.get::<Children>(root).unwrap(), &mut entities);
            let entity = entities
                .into_iter()
                .find(|&entity| world.get::<BuyerRows>(entity).is_some())
                .unwrap();
            let mut values = Vec::new();
            labels(app.world(), entity, &mut values);
            values.join("\n")
        };
        assert_eq!(
            displayed(&mut app, Good::Berries),
            expected.replace("\n\n", "\n")
        );
        let targets = app
            .world_mut()
            .query::<&crate::navigation::Target>()
            .iter(app.world())
            .copied()
            .collect::<Vec<_>>();
        assert!(targets.contains(&crate::navigation::Target::Household(
            universe.economic_id(funded).unwrap()
        )));
        assert!(targets.contains(&crate::navigation::Target::Household(
            universe.economic_id(cashless).unwrap()
        )));
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = universe
            .with_purchase_request(cashless, ShoppingList::default())
            .unwrap();
        app.update();
        assert_eq!(
            displayed(&mut app, Good::Berries),
            "Same\n200 g requested  |  100 g affordable"
        );
        press_in(&mut app, WindowKind::Market, Choice::Good(Good::Water));
        assert_eq!(
            displayed(&mut app, Good::Water),
            "Water buyer\n400 g requested  |  0 g affordable"
        );
        assert_eq!(
            displayed(&mut app, Good::Water),
            "Water buyer\n400 g requested  |  0 g affordable"
        );
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0.starts_with("CURRENT BUY REQUESTS"))
        );
    }

    #[test]
    fn orders_preserve_seller_identity_remaining_stock_and_distinct_asks() {
        let (universe, first) = Universe::with_map(Map::default())
            .with_prices(Prices::new(1.0).unwrap())
            .with_citizen(
                "Same",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_berries(1_000)
                    .unwrap(),
            )
            .unwrap();
        let (universe, second) = universe
            .with_citizen(
                "Same",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_berries(1_000)
                    .unwrap(),
            )
            .unwrap();
        let (universe, buyer) = universe
            .with_citizen(
                "Buyer",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(10)
                    .unwrap(),
            )
            .unwrap();
        let universe = universe
            .start_action(first, CitizenAction::List(Good::Berries, 200))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap()
            .start_action(second, CitizenAction::List(Good::Berries, 300))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let universe = universe
            .advance(learning_lord_simulation::marketplace::UPDATE_TIME_MS)
            .unwrap()
            .with_prices(Prices::new(2.0).unwrap())
            .start_action(first, CitizenAction::List(Good::Berries, 100))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let universe = universe
            .start_action(
                buyer,
                CitizenAction::Buy(learning_lord_simulation::marketplace::ShoppingList::single(
                    Good::Berries,
                    50,
                )),
            )
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let groups = seller_orders(&universe, Good::Berries);
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| group.name == "Same household"));
        let first_group = groups
            .iter()
            .find(|group| group.id == MarketParty::Household(universe.economic_id(first).unwrap()))
            .unwrap();
        let second_group = groups
            .iter()
            .find(|group| group.id == MarketParty::Household(universe.economic_id(second).unwrap()))
            .unwrap();
        for group in [first_group, second_group] {
            let rate = universe
                .market()
                .seller_adjustment_rate(group.id.account().unwrap())
                .unwrap();
            assert_eq!(group.adjustment_rate, Some(rate));
            assert!((group.orders[0].2 - (1.0 + f64::from(rate) / 200.0)).abs() < 1e-12);
            assert!(group.orders.iter().all(|order| !order.3));
        }
        assert_eq!(first_group.orders.len(), 2);
        assert_eq!(second_group.orders.len(), 1);
        let first_fills = first_group.orders[0].2 <= second_group.orders[0].2;
        assert_eq!(first_group.orders[0].1, if first_fills { 150 } else { 200 });
        assert_eq!(
            second_group.orders[0].1,
            if first_fills { 300 } else { 250 }
        );
        assert_eq!(first_group.orders[1].1, 100);
        assert_eq!(
            first_group.orders[1].2,
            1.0 + f64::from(first_group.adjustment_rate.unwrap()) / 100.0
        );
        assert_eq!(
            groups
                .iter()
                .flat_map(|group| &group.orders)
                .map(|order| order.1)
                .sum::<u64>(),
            550
        );
        let readout = order_text(&universe, Good::Berries);
        for group in &groups {
            for &(_, units, quote, _) in &group.orders {
                assert!(readout.contains(&order_label(Good::Berries, units, quote)));
            }
        }
        assert!(!readout.contains("reserved"));
        assert!(seller_orders(&universe, Good::Water).is_empty());
        assert_eq!(
            order_label(Good::Berries, 150, 1.0),
            "150 g remaining  |  1.000 coins/kg  |  0.150 coins quoted value"
        );
        assert_eq!(
            order_label(Good::Berries, 100, 2.0),
            "100 g remaining  |  2.000 coins/kg  |  0.200 coins quoted value"
        );
        let mut app = app();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = universe;
        app.update();
        let labels = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        assert!(
            labels
                .iter()
                .any(|text| text.starts_with("CURRENT SELL ORDERS"))
        );
        assert_eq!(
            labels
                .iter()
                .filter(|text| text.starts_with("CANARY"))
                .count(),
            0
        );
        assert!(
            labels
                .iter()
                .any(|text| text.starts_with("AVAILABLE SUPPLY BY ASK")
                    && text.contains("550 g cumulative"))
        );
        for group in &groups {
            assert!(labels.contains(&format!(
                "Price adjustment rate: {}%",
                group.adjustment_rate.unwrap()
            )));
            for order in &group.orders {
                assert!(labels.contains(&order_label(Good::Berries, order.1, order.2)));
            }
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(Snapshot {
            universe: simulation::new_universe().unwrap(),
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
        }))
        .init_resource::<Selection>()
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Windows>()
        .add_systems(
            Update,
            (
                handle_selection,
                crate::inspectors::open_windows,
                refresh_choices,
                refresh,
            )
                .chain(),
        );
        let mut windows = app.world_mut().resource_mut::<Windows>();
        windows.open(WindowKind::Market);
        windows.link(WindowKind::Market, WindowKind::Good(Good::Berries));
        app.update();
        app
    }

    fn root(world: &mut World, kind: WindowKind) -> Entity {
        world
            .query::<(Entity, &FloatingWindow)>()
            .iter(world)
            .find(|(_, window)| window.0 == kind)
            .unwrap()
            .0
    }

    fn press_in(app: &mut App, kind: WindowKind, choice: Choice) {
        let world = app.world_mut();
        let root = root(world, kind);
        let mut entities = Vec::new();
        descendants(world, world.get::<Children>(root).unwrap(), &mut entities);
        let entity = entities
            .into_iter()
            .find(|&entity| match (choice, world.get::<Choice>(entity)) {
                (Choice::Good(a), Some(Choice::Good(b))) => a == *b,
                (Choice::Tab(a), Some(Choice::Tab(b))) => a == *b,
                (Choice::Carpentry, Some(Choice::Carpentry)) => true,
                _ => false,
            })
            .unwrap();
        world.entity_mut(entity).insert(Interaction::Pressed);
        app.update();
        app.world_mut().entity_mut(entity).insert(Interaction::None);
        app.update();
    }

    #[test]
    fn typing_in_caravan_editor_suppresses_market_navigation_shortcuts() {
        let mut app = app();
        app.insert_resource(crate::caravan_controls::State::focused_fixture());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();
        assert!(
            app.world()
                .resource::<Windows>()
                .is_open(WindowKind::Market)
        );
        assert!(
            !app.world()
                .resource::<Windows>()
                .is_open(WindowKind::Citizens)
        );
        assert_eq!(app.world().resource::<Selection>().good(), Good::Berries);
    }

    #[test]
    fn tab_good_keyboard_restart_and_idle_updates() {
        let mut app = app();
        let goods = app
            .world_mut()
            .query::<&Choice>()
            .iter(app.world())
            .filter(|choice| matches!(choice, Choice::Good(_)))
            .count();
        assert_eq!(goods, Good::COUNT);
        press_in(&mut app, WindowKind::Market, Choice::Good(Good::BreadRoll));
        app.update();
        assert_eq!(app.world().resource::<Selection>().good(), Good::BreadRoll);
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(app.world().resource::<Selection>().good(), Good::BreadRoll);
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert!(
            app.world()
                .resource::<Windows>()
                .is_open(WindowKind::Good(Good::BreadRoll))
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();
        assert_eq!(app.world().resource::<Selection>().good(), Good::BerryPie);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        app.world_mut().clear_trackers();
        app.update();
        assert!(!app.world().resource_ref::<Selection>().is_changed());
        for (text, node, color, visibility) in app
            .world_mut()
            .query::<(
                Option<Ref<Text>>,
                Option<Ref<Node>>,
                Option<Ref<BackgroundColor>>,
                Option<Ref<Visibility>>,
            )>()
            .iter(app.world())
        {
            assert!(text.is_none_or(|value| !value.is_changed()));
            assert!(node.is_none_or(|value| !value.is_changed()));
            assert!(color.is_none_or(|value| !value.is_changed()));
            assert!(visibility.is_none_or(|value| !value.is_changed()));
        }
    }
}
