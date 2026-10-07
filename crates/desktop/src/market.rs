use crate::{DisplaySnapshot, MUTED, PANEL, SELECTED, TEXT, format_clock, text};
use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
    ui::RelativeCursorPosition,
};
use learning_lord_simulation::{
    AgentId, Universe,
    marketplace::{Good, GoodActivity, MarketPeriod},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum View {
    #[default]
    Citizens,
    Market,
    Locations,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Period {
    #[default]
    Current,
    Previous,
}

#[derive(Resource, Default)]
pub struct Selection {
    pub(super) view: View,
    good: Option<Good>,
    period: Period,
    generation: u64,
}

impl Selection {
    pub(super) fn good(&self) -> Good {
        self.good.unwrap_or(Good::Berries)
    }
}

#[derive(Component)]
pub struct ViewPanel(pub View);

#[derive(Component, Clone, Copy)]
pub enum Choice {
    View(View),
    Good(Good),
    Period(Period),
}

#[derive(Component)]
pub enum Readout {
    Price(Good),
    Title,
    PriceNow,
    Period,
    Metrics,
    Buyers,
}

#[derive(Component)]
pub struct Orders;

#[derive(Resource, Default)]
pub struct OrderDisplay(Vec<SellerOrders>);

#[derive(Clone, Debug, PartialEq)]
struct SellerOrders {
    id: AgentId,
    name: String,
    location: String,
    orders: Vec<(u64, u64, f64)>,
}

fn choice(value: Choice) -> impl Bundle {
    (
        Button,
        value,
        Node {
            min_height: px(36),
            padding: UiRect::axes(px(12), px(8)),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(PANEL),
    )
}

pub fn spawn_tabs(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            column_gap: px(8),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (view, label) in [
                (View::Citizens, "Citizens"),
                (View::Market, "Market"),
                (View::Locations, "Locations"),
            ] {
                row.spawn(choice(Choice::View(view)))
                    .with_children(|button| {
                        button.spawn(text(label, 16.0, TEXT));
                    });
            }
        });
}

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent.spawn((ViewPanel(View::Market), Visibility::Hidden, Node {
        display: Display::None, width: percent(100), flex_grow: 1.0,
        min_height: px(0), column_gap: px(14), ..default()
    })).with_children(|row| {
        row.spawn((Node {
            width: px(240), flex_shrink: 0.0, min_height: px(0),
            flex_direction: FlexDirection::Column, row_gap: px(8),
            overflow: Overflow::scroll_y(), ..default()
        }, ScrollPosition::default(), RelativeCursorPosition::default())).with_children(|list| {
            list.spawn(text("GOODS  |  current prices", 14.0, MUTED));
            for good in Good::ALL {
                list.spawn((Button, Choice::Good(good), BackgroundColor(PANEL), Node {
                    width: percent(100), flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart, padding: UiRect::all(px(12)),
                    row_gap: px(3), flex_shrink: 0.0,
                    border_radius: BorderRadius::all(px(8)), ..default()
                })).with_children(|button| {
                    button.spawn(text(good.name(), 16.0, TEXT));
                    button.spawn((text("", 13.0, MUTED), Readout::Price(good)));
                });
            }
        });
        row.spawn((Node {
            flex_basis: px(0), flex_grow: 1.0, min_width: px(0), min_height: px(0),
            padding: UiRect::all(px(16)), flex_direction: FlexDirection::Column,
            row_gap: px(12), overflow: Overflow::scroll_y(),
            border_radius: BorderRadius::all(px(12)), ..default()
        }, BackgroundColor(PANEL), ScrollPosition::default(), RelativeCursorPosition::default()))
        .with_children(|details| {
            details.spawn((text("", 24.0, TEXT), Readout::Title));
            details.spawn((text("", 16.0, TEXT), Readout::PriceNow));
            crate::market_history::spawn(details);
            details.spawn(Node { column_gap: px(8), row_gap: px(8), flex_wrap: FlexWrap::Wrap, flex_shrink: 0.0, ..default() })
                .with_children(|row| {
                    for (period, label) in [(Period::Current, "Current market day"), (Period::Previous, "Previous market day")] {
                        row.spawn(choice(Choice::Period(period))).with_children(|button| { button.spawn(text(label, 14.0, TEXT)); });
                    }
                });
            details.spawn((text("", 14.0, MUTED), Readout::Period));
            details.spawn((text("", 17.0, TEXT), Readout::Metrics));
            details.spawn(text("Buy = traded + affordable unfulfilled demand\nSell = traded + listed stock\nUnfulfilled demand may exist while stock is available.", 13.0, MUTED));
            details.spawn(text("CURRENT BUY REQUESTS  |  live in either period view", 14.0, MUTED));
            details.spawn((text("", 14.0, TEXT), Node { flex_shrink: 0.0, ..default() }, Readout::Buyers));
            details.spawn(text("CURRENT SELL ORDERS  |  live in either period view", 14.0, MUTED));
            details.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(10), flex_shrink: 0.0, ..default() }, Orders));
        });
    });
}

pub fn handle_selection(
    snapshot: Res<DisplaySnapshot>,
    keyboard: Res<ButtonInput<KeyCode>>,
    buttons: Query<(Entity, &Interaction, &Choice), Changed<Interaction>>,
    mut selection: ResMut<Selection>,
    mut focus: ResMut<InputFocus>,
    editor: Option<Res<crate::locations::State>>,
) {
    if selection.generation != snapshot.0.generation {
        selection.generation = snapshot.0.generation;
        selection.period = Period::Current;
    }
    for (entity, interaction, choice) in &buttons {
        if *interaction == Interaction::Pressed {
            focus.set(entity, FocusCause::Pressed);
            match *choice {
                Choice::View(view) => {
                    if selection.view != view {
                        selection.view = view;
                    }
                }
                Choice::Good(good) => {
                    if selection.good() != good {
                        selection.good = Some(good);
                    }
                }
                Choice::Period(period) => {
                    if selection.period != period {
                        selection.period = period;
                    }
                }
            }
        }
    }
    if editor.is_some_and(|editor| editor.editing()) {
        return;
    }
    for (key, view) in [
        (KeyCode::KeyC, View::Citizens),
        (KeyCode::KeyM, View::Market),
        (KeyCode::KeyL, View::Locations),
    ] {
        if keyboard.just_pressed(key) && selection.view != view {
            selection.view = view;
        }
    }
    if selection.view == View::Market {
        let index = selection.good() as usize;
        let next = if keyboard.just_pressed(KeyCode::ArrowDown) {
            Good::ALL[(index + 1) % Good::COUNT]
        } else if keyboard.just_pressed(KeyCode::ArrowUp) {
            Good::ALL[(index + Good::COUNT - 1) % Good::COUNT]
        } else {
            selection.good()
        };
        if next != selection.good() {
            selection.good = Some(next);
        }
    }
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
            .position(|group| group.id == order.seller)
            .unwrap_or_else(|| {
                groups.push(SellerOrders {
                    id: order.seller,
                    name: universe
                        .agents()
                        .get(&order.seller)
                        .map_or_else(|| "Unknown seller".into(), |agent| agent.name.clone()),
                    location: universe
                        .map()
                        .place(order.place)
                        .map_or_else(|_| "Unknown location".into(), |place| place.name.clone()),
                    orders: Vec::new(),
                });
                groups.len() - 1
            });
        groups[index]
            .orders
            .push((order.id.0, order.units, order.quoted_price));
    }
    for group in &mut groups {
        group.orders.sort_by_key(|order| order.0);
    }
    groups.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.0.cmp(&b.id.0)));
    groups
}

fn order_label(good: Good, units: u64, price: f64) -> String {
    format!(
        "{} remaining  |  {price:.3} coins/{}  |  {:.3} coins quoted value",
        crate::quantity_label(good, units),
        good.price_unit_name(),
        units as f64 / good.units_per_price_unit() as f64 * price
    )
}

fn buyer_requests(universe: &Universe, good: Good) -> String {
    let market = universe.market();
    let mut buyers: Vec<_> = universe
        .agents()
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

fn metrics(good: Good, activity: GoodActivity) -> String {
    format!(
        "Buy volume: {}\nSell volume: {}\nTraded volume: {}\nUnfulfilled demand: {}",
        crate::quantity_label(
            good,
            activity
                .traded_units
                .saturating_add(activity.affordable_demand_units)
        ),
        crate::quantity_label(
            good,
            activity.traded_units.saturating_add(activity.listed_units)
        ),
        crate::quantity_label(good, activity.traded_units),
        crate::quantity_label(good, activity.affordable_demand_units)
    )
}

fn period_label(period: &MarketPeriod, selected: Period, now: u64) -> String {
    match selected {
        Period::Current => format!(
            "CURRENT MARKET DAY  |  {} to now ({})\nCloses at {}",
            format_clock(period.start_ms),
            format_clock(now),
            format_clock(period.end_ms)
        ),
        Period::Previous => format!(
            "CLOSED MARKET DAY  |  {} to {}\nStock and demand are recorded at the close.",
            format_clock(period.start_ms),
            format_clock(period.end_ms)
        ),
    }
}

pub fn refresh_choices(
    selection: Res<Selection>,
    mut buttons: Query<(&Choice, &Interaction, &mut BackgroundColor)>,
) {
    for (choice, interaction, mut color) in &mut buttons {
        let selected = match *choice {
            Choice::View(view) => selection.view == view,
            Choice::Good(good) => selection.good() == good,
            Choice::Period(period) => selection.period == period,
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

pub fn refresh(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<Selection>,
    mut cached: ResMut<OrderDisplay>,
    mut panels: Query<(&ViewPanel, &mut Node, &mut Visibility)>,
    mut readouts: Query<(&Readout, &mut Text)>,
    orders: Query<Entity, With<Orders>>,
) {
    if !snapshot.is_changed() && !selection.is_changed() {
        return;
    }
    for (panel, mut node, mut visibility) in &mut panels {
        let display = if panel.0 == selection.view {
            Display::Flex
        } else {
            Display::None
        };
        visibility.set_if_neq(if panel.0 == selection.view {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if node.display != display {
            node.display = display;
        }
    }
    let universe = &snapshot.0.universe;
    let selected_period = match selection.period {
        Period::Current => Some(universe.market().current_period()),
        Period::Previous => universe.market().previous_period(),
    };
    for (readout, mut value) in &mut readouts {
        let next = match *readout {
            Readout::Price(good) => format!(
                "{:.3} coins/{}",
                universe.prices().price(good).unwrap(),
                good.price_unit_name()
            ),
            Readout::Title => selection.good().name().into(),
            Readout::PriceNow => format!(
                "Current price: {:.3} coins/{}",
                universe.prices().price(selection.good()).unwrap(),
                selection.good().price_unit_name()
            ),
            Readout::Period => selected_period.as_ref().map_or_else(
                || "No closed market day yet. The first period closes at Day 0 | 04:00:00.".into(),
                |period| period_label(period, selection.period, universe.current_time_ms()),
            ),
            Readout::Metrics => selected_period.as_ref().map_or_else(
                || "No previous-day figures available.".into(),
                |period| metrics(selection.good(), period.goods[selection.good() as usize]),
            ),
            Readout::Buyers => buyer_requests(universe, selection.good()),
        };
        if value.0 != next {
            value.0 = next;
        }
    }
    let groups = seller_orders(universe, selection.good());
    if groups != cached.0 || cached.is_added() {
        for entity in &orders {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|content| {
                    if groups.is_empty() {
                        content.spawn(text("No current sell orders for this good.", 14.0, MUTED));
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
                                seller.spawn(text(&group.name, 16.0, TEXT));
                                seller.spawn(text(&group.location, 14.0, MUTED));
                                if groups
                                    .iter()
                                    .filter(|other| other.name == group.name)
                                    .count()
                                    > 1
                                {
                                    seller.spawn(text(
                                        format!("Seller {}", group.id.0),
                                        12.0,
                                        MUTED,
                                    ));
                                }
                                for &(_, units, price) in &group.orders {
                                    seller.spawn(text(
                                        order_label(selection.good(), units, price),
                                        14.0,
                                        MUTED,
                                    ));
                                }
                            });
                    }
                });
        }
        cached.0 = groups;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{self, Snapshot};
    use learning_lord_simulation::{
        Citizen, CitizenAction, TRADE_DURATION_MS, locations::Map, marketplace::Prices,
    };

    #[test]
    fn quantities_include_transactions_once_and_label_periods() {
        let value = metrics(
            Good::Berries,
            GoodActivity {
                traded_units: 200,
                listed_units: 800,
                affordable_demand_units: 500,
            },
        );
        assert_eq!(
            value,
            "Buy volume: 700 g\nSell volume: 1000 g\nTraded volume: 200 g\nUnfulfilled demand: 500 g"
        );
        let period = Universe::default().market().current_period();
        assert!(period_label(&period, Period::Current, 1_000).contains("CURRENT MARKET DAY"));
        assert!(period_label(&period, Period::Previous, 1_000).contains("CLOSED MARKET DAY"));
    }

    #[test]
    fn counted_goods_display_integer_quantities_and_item_prices() {
        assert_eq!(
            order_label(Good::Bread, 2, 15.0),
            "2 loaves remaining  |  15.000 coins/loaf  |  30.000 coins quoted value"
        );
        assert_eq!(
            order_label(Good::BerryPie, 1, 25.0),
            "1 pie remaining  |  25.000 coins/pie  |  25.000 coins quoted value"
        );
        assert!(
            metrics(
                Good::Bread,
                GoodActivity {
                    traded_units: 1,
                    listed_units: 2,
                    affordable_demand_units: 3
                }
            )
            .contains("Buy volume: 4 loaves")
        );
    }

    #[test]
    fn buy_requests_include_cashless_buyers_and_stay_live_in_previous_view() {
        use learning_lord_simulation::marketplace::ShoppingList;

        let (universe, funded) = Universe::with_map(Map::default())
            .with_prices(Prices::new(10.0).unwrap())
            .with_citizen("Same", Citizen::new(0.0).unwrap().with_coins(1).unwrap())
            .unwrap();
        let (universe, cashless) = universe
            .with_citizen("Same", Citizen::new(0.0).unwrap())
            .unwrap();
        let (universe, water_buyer) = universe
            .with_citizen("Water buyer", Citizen::new(0.0).unwrap())
            .unwrap();
        let universe = universe
            .with_purchase_request(funded, ShoppingList::single(Good::Berries, 200))
            .unwrap()
            .with_purchase_request(cashless, ShoppingList::single(Good::Berries, 300))
            .unwrap()
            .with_purchase_request(water_buyer, ShoppingList::single(Good::Water, 400))
            .unwrap();
        let mut expected = [
            (funded, "200 g requested  |  100 g affordable"),
            (cashless, "300 g requested  |  0 g affordable"),
        ];
        expected.sort_by_key(|(id, _)| id.0);
        let expected = expected
            .iter()
            .map(|(id, quantities)| format!("Same\nBuyer {}\n{quantities}", id.0))
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_eq!(buyer_requests(&universe, Good::Berries), expected);
        assert_eq!(
            buyer_requests(&universe, Good::Bread),
            "No current buy requests for this good."
        );

        let mut app = app();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = universe.clone();
        press(&mut app, Choice::Period(Period::Previous));
        let displayed = |app: &mut App| {
            app.world_mut()
                .query::<(&Readout, &Text)>()
                .iter(app.world())
                .find(|(readout, _)| matches!(readout, Readout::Buyers))
                .unwrap()
                .1
                .0
                .clone()
        };
        assert_eq!(displayed(&mut app), expected);
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe = universe
            .with_purchase_request(cashless, ShoppingList::default())
            .unwrap();
        app.update();
        assert_eq!(
            displayed(&mut app),
            "Same\n200 g requested  |  100 g affordable"
        );
        assert_eq!(app.world().resource::<Selection>().period, Period::Previous);
        press(&mut app, Choice::Good(Good::Water));
        assert_eq!(
            displayed(&mut app),
            "Water buyer\n400 g requested  |  0 g affordable"
        );
        press(&mut app, Choice::Period(Period::Current));
        assert_eq!(
            displayed(&mut app),
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
                Citizen::new(0.0).unwrap().with_berries(1_000).unwrap(),
            )
            .unwrap();
        let (universe, second) = universe
            .with_citizen(
                "Same",
                Citizen::new(0.0).unwrap().with_berries(1_000).unwrap(),
            )
            .unwrap();
        let (universe, buyer) = universe
            .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(10).unwrap())
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
        assert!(groups.iter().all(|group| group.name == "Same"));
        let first_group = groups.iter().find(|group| group.id == first).unwrap();
        assert_eq!(first_group.orders.len(), 2);
        assert_eq!(first_group.orders[0].1, 150);
        assert_eq!(first_group.orders[0].2, 1.0);
        assert_eq!(first_group.orders[1].1, 100);
        assert_eq!(first_group.orders[1].2, 2.0);
        assert_eq!(
            groups
                .iter()
                .find(|group| group.id == second)
                .unwrap()
                .orders[0]
                .1,
            300
        );
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
        press(&mut app, Choice::Period(Period::Previous));
        assert_eq!(app.world().resource::<OrderDisplay>().0, groups);
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == order_label(Good::Berries, 150, 1.0))
        );
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0.starts_with("CURRENT SELL ORDERS"))
        );
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
        .init_resource::<OrderDisplay>()
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, (handle_selection, refresh_choices, refresh).chain());
        app.world_mut()
            .spawn((ViewPanel(View::Citizens), Node::default()));
        app.add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                spawn_tabs(parent);
                spawn(parent);
            });
        });
        app.update();
        app
    }

    fn press(app: &mut App, choice: Choice) {
        let entity = app
            .world_mut()
            .query::<(Entity, &Choice)>()
            .iter(app.world())
            .find(|(_, value)| match (choice, **value) {
                (Choice::View(a), Choice::View(b)) => a == b,
                (Choice::Good(a), Choice::Good(b)) => a == b,
                (Choice::Period(a), Choice::Period(b)) => a == b,
                _ => false,
            })
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
        app.world_mut().entity_mut(entity).insert(Interaction::None);
        app.update();
    }

    #[test]
    fn tab_good_period_keyboard_restart_and_idle_updates() {
        let mut app = app();
        let goods = app
            .world_mut()
            .query::<&Choice>()
            .iter(app.world())
            .filter(|choice| matches!(choice, Choice::Good(_)))
            .count();
        assert_eq!(goods, Good::COUNT);
        press(&mut app, Choice::View(View::Market));
        for (panel, node, visibility) in app
            .world_mut()
            .query::<(&ViewPanel, &Node, &Visibility)>()
            .iter(app.world())
        {
            assert_eq!(
                node.display,
                if panel.0 == View::Market {
                    Display::Flex
                } else {
                    Display::None
                }
            );
            assert_eq!(
                *visibility,
                if panel.0 == View::Market {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                }
            );
        }
        press(&mut app, Choice::Good(Good::Bread));
        press(&mut app, Choice::Period(Period::Previous));
        assert_eq!(app.world().resource::<Selection>().good(), Good::Bread);
        assert!(
            app.world_mut()
                .query::<(&Readout, &Text)>()
                .iter(app.world())
                .any(|(readout, text)| matches!(readout, Readout::Period)
                    && text.0.starts_with("No closed market day"))
        );
        app.world_mut().resource_mut::<DisplaySnapshot>().0.revision += 1;
        app.update();
        assert_eq!(app.world().resource::<Selection>().good(), Good::Bread);
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert_eq!(app.world().resource::<Selection>().period, Period::Current);
        assert_eq!(app.world().resource::<Selection>().good(), Good::Bread);
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
