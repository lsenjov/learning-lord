use crate::{DisplaySnapshot, MUTED, TEXT, format_clock, market, text};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use learning_lord_simulation::{
    Universe,
    marketplace::{DAY_MS, Good, GoodActivity, UPDATE_TIME_MS},
};

const WINDOW: usize = 30;
const HEIGHT: f32 = 280.0;
const PRICE: Color = Color::srgb(0.38, 0.78, 0.91);
const DEMAND: Color = Color::srgb(0.96, 0.70, 0.35);
const STOCK: Color = Color::srgb(0.67, 0.57, 0.87);
const BODY: Color = Color::srgb(0.69, 0.75, 0.77);

#[derive(Clone, Debug, PartialEq)]
struct Day {
    start: u64,
    end: u64,
    price: f64,
    activity: GoodActivity,
    partial: bool,
}

fn days(universe: &Universe, good: Good) -> Vec<Day> {
    let market = universe.market();
    let current = market.current_period();
    let (first, created) = market.initial_period();
    let mut start = current.start_ms;
    let mut result: Vec<Day> = Vec::new();
    let mut price = universe.prices().price(good).unwrap();
    let mut history = market
        .history()
        .iter()
        .rev()
        .filter(|entry| entry.good == good)
        .peekable();
    for slot in 0..WINDOW {
        let end = if slot == 0 {
            universe.current_time_ms()
        } else {
            result.last().unwrap().start.max(first)
        };
        let activity = if slot == 0 {
            current.goods[good as usize]
        } else if history.peek().is_some_and(|entry| entry.start_ms == start) {
            let entry = history.next().unwrap();
            price = entry.price_before;
            GoodActivity {
                traded_units: entry.traded_units,
                listed_units: entry.remaining_supply_units,
                affordable_demand_units: entry.unmet_demand_units,
                traded_coins: entry.traded_coins,
                caravan_listed_units: entry.caravan_remaining_supply_units,
                local_traded_units: entry.local_traded_units,
                local_traded_coins: entry.local_traded_coins,
                exported_units: entry.exported_units,
                exported_coins: entry.exported_coins,
                exported_receipts: entry.exported_receipts,
                export_tariff_coins: entry.export_tariff_coins,
                import_tariff_coins: entry.import_tariff_coins,
                imported_units: entry.imported_units,
                caravan_purchased_units: entry.caravan_purchased_units,
                caravan_purchased_coins: entry.caravan_purchased_coins,
            }
        } else {
            GoodActivity::default()
        };
        result.push(Day {
            start: start.max(created),
            end,
            price,
            activity,
            partial: slot == 0,
        });
        if start == first {
            break;
        }
        start = if start == UPDATE_TIME_MS {
            0
        } else {
            start.saturating_sub(DAY_MS).max(first)
        };
    }
    result.reverse();
    result
}

struct Scale {
    low: f64,
    high: f64,
    price_max: f64,
    quantity_max: f64,
    extent: f64,
}
impl Scale {
    fn new(days: &[Day]) -> Self {
        let price_max = days.iter().map(|day| day.price).fold(1.0, f64::max);
        let low = days
            .iter()
            .map(|day| day.price / price_max)
            .fold(1.0, f64::min);
        let high = days
            .iter()
            .map(|day| day.price / price_max)
            .fold(0.0, f64::max);
        let quantity_max = days
            .iter()
            .flat_map(|day| {
                [
                    day.activity.traded_units,
                    day.activity.affordable_demand_units,
                    day.activity.listed_units,
                ]
            })
            .max()
            .unwrap_or(1)
            .max(1) as f64;
        let extent = days
            .iter()
            .map(|day| {
                day.activity.traded_units as f64 / quantity_max / 2.0
                    + day
                        .activity
                        .affordable_demand_units
                        .max(day.activity.listed_units) as f64
                        / quantity_max
            })
            .fold(1.0, f64::max);
        Self {
            low,
            high,
            price_max,
            quantity_max,
            extent,
        }
    }
    fn y(&self, price: f64) -> f32 {
        if self.high == self.low {
            HEIGHT / 2.0
        } else {
            180.0 - ((price / self.price_max - self.low) / (self.high - self.low)) as f32 * 80.0
        }
    }
    fn quantity(&self, units: f64) -> f32 {
        ((units / self.quantity_max) / self.extent) as f32 * 80.0
    }
}

#[derive(Resource, Default)]
pub struct ChartDisplay {
    days: Vec<Day>,
    good: Option<Good>,
}
#[derive(Component)]
pub(super) struct Plot;
#[derive(Component)]
pub(super) struct Axis;
#[derive(Component)]
pub(super) struct Legend;
#[derive(Component)]
pub(super) struct Hover;

pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent.spawn(text(
        "MARKET HISTORY  |  recent 30 market days",
        14.0,
        MUTED,
    ));
    parent.spawn(text("Blue step line + body center: price (coins per kg or item). Body height: total traded quantity (local + exports + caravan purchases).\nAmber upper wick: affordable unmet demand. Purple lower wick: unsold stock.\nBody and wicks use the quantity size scale, not the price axis. Close pressure affects the next 04:00 price.", 13.0, MUTED));
    parent.spawn((text("", 13.0, TEXT), Legend));
    parent
        .spawn(Node {
            width: percent(100),
            height: px(HEIGHT + 24.0),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(100),
                    height: px(HEIGHT),
                    flex_shrink: 0.0,
                    ..default()
                },
                Axis,
            ));
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    height: px(HEIGHT),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.065, 0.09, 0.115)),
                RelativeCursorPosition::default(),
                Plot,
            ));
        });
    parent.spawn((
        text(
            "Hover a market day for exact figures. Cyan body marks the current partial day.",
            13.0,
            MUTED,
        ),
        Node {
            min_height: px(64),
            flex_shrink: 0.0,
            ..default()
        },
        Hover,
    ));
}

fn rect(parent: &mut ChildSpawnerCommands, x: f32, y: f32, width: f32, height: f32, color: Color) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(x),
            top: px(y),
            width: percent(width),
            height: px(height),
            ..default()
        },
        BackgroundColor(color),
    ));
}
fn body_mark(
    parent: &mut ChildSpawnerCommands,
    center: f32,
    y: f32,
    slot: f32,
    height: f32,
    color: Color,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(center),
            top: px(y),
            width: percent(slot * 0.40),
            max_width: px(18),
            height: px(height),
            ..default()
        },
        UiTransform::from_xy(percent(-50), px(0)),
        BackgroundColor(color),
    ));
}

fn label(parent: &mut ChildSpawnerCommands, value: String, x: f32, y: f32, color: Color) {
    parent.spawn((
        text(value, 11.0, color),
        Node {
            position_type: PositionType::Absolute,
            left: percent(x),
            top: px(y),
            ..default()
        },
    ));
}

pub fn refresh(
    mut commands: Commands,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<market::Selection>,
    mut cached: ResMut<ChartDisplay>,
    plots: Query<Entity, With<Plot>>,
    axes: Query<Entity, With<Axis>>,
    mut legends: Query<&mut Text, With<Legend>>,
) {
    if selection.view != market::View::Market || (!snapshot.is_changed() && !selection.is_changed())
    {
        return;
    }
    let good = selection.good();
    let values = days(&snapshot.0.universe, good);
    let same_geometry = cached.good == Some(good)
        && cached.days.len() == values.len()
        && cached.days.iter().zip(&values).all(|(old, new)| {
            old.start == new.start
                && old.price == new.price
                && old.activity == new.activity
                && old.partial == new.partial
        });
    if same_geometry {
        if cached.days != values {
            cached.days = values;
        }
        return;
    }
    let scale = Scale::new(&values);
    for mut legend in &mut legends {
        legend.0 = format!(
            "Quantity size key at left: {}  |  one linear scale across these days",
            chart_quantity_label(good, scale.quantity_max)
        );
    }
    for entity in &axes {
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|axis| {
                label(
                    axis,
                    format!("coins/{}", good.price_unit_name()),
                    0.0,
                    5.0,
                    PRICE,
                );
                rect(
                    axis,
                    78.0,
                    195.0,
                    3.0,
                    scale.quantity(scale.quantity_max),
                    BODY,
                );
                label(
                    axis,
                    chart_quantity_label(good, scale.quantity_max),
                    0.0,
                    218.0,
                    BODY,
                );
                if scale.high == scale.low {
                    label(
                        axis,
                        format!("{:.3}", scale.price_max * scale.low),
                        0.0,
                        HEIGHT / 2.0 - 7.0,
                        PRICE,
                    );
                } else {
                    for (value, y) in [
                        (scale.high, 100.0),
                        ((scale.high + scale.low) / 2.0, 140.0),
                        (scale.low, 180.0),
                    ] {
                        label(
                            axis,
                            format!("{:.3}", value * scale.price_max),
                            0.0,
                            y - 7.0,
                            PRICE,
                        );
                    }
                }
            });
    }
    for entity in &plots {
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|plot| {
                if values.is_empty() {
                    label(plot, "No market history yet".into(), 2.0, 120.0, MUTED);
                    return;
                }
                let slot = 100.0 / values.len() as f32;
                for y in [100.0, 140.0, 180.0] {
                    rect(plot, 0.0, y, 100.0, 1.0, Color::srgb(0.16, 0.20, 0.23));
                }
                for (index, day) in values.iter().enumerate() {
                    let left = index as f32 * slot;
                    let center = left + slot / 2.0;
                    let y = scale.y(day.price);
                    rect(plot, left, y, slot, 2.0, PRICE);
                    if let Some(next) = values.get(index + 1) {
                        let next_y = scale.y(next.price);
                        rect(
                            plot,
                            left + slot,
                            y.min(next_y),
                            0.18,
                            (y - next_y).abs().max(2.0),
                            PRICE,
                        );
                    }
                    let body = scale.quantity(day.activity.traded_units as f64);
                    let top = y - body / 2.0;
                    rect(
                        plot,
                        center - 0.1,
                        top - scale.quantity(day.activity.affordable_demand_units as f64),
                        0.2,
                        scale.quantity(day.activity.affordable_demand_units as f64),
                        DEMAND,
                    );
                    rect(
                        plot,
                        center - 0.1,
                        y + body / 2.0,
                        0.2,
                        scale.quantity(day.activity.listed_units as f64),
                        STOCK,
                    );
                    body_mark(
                        plot,
                        center,
                        if body == 0.0 { y - 0.5 } else { top },
                        slot,
                        if body == 0.0 { 1.0 } else { body },
                        if day.partial { PRICE } else { BODY },
                    );
                    body_mark(plot, center, y, slot, 2.0, PRICE);
                    if index == 0
                        || index == values.len() - 1
                        || (values.len() > 10 && index == values.len() / 2)
                    {
                        let value = format!(
                            "Day {}{}",
                            day.start / DAY_MS,
                            if day.partial { " *" } else { "" }
                        );
                        if index == values.len() - 1 {
                            plot.spawn((
                                text(value, 11.0, MUTED),
                                Node {
                                    position_type: PositionType::Absolute,
                                    right: px(0),
                                    top: px(HEIGHT + 2.0),
                                    ..default()
                                },
                            ));
                        } else {
                            label(plot, value, left, HEIGHT + 2.0, MUTED);
                        }
                    }
                }
            });
    }
    cached.days = values;
    cached.good = Some(good);
}

fn chart_quantity_label(good: Good, units: f64) -> String {
    if good.units_per_price_unit() == 1 {
        let unit = if units == 1.0 {
            match good {
                Good::FlaxBlock => "block",
                Good::FlaxGarment => "garment",
                _ => good.price_unit_name(),
            }
        } else {
            good.unit_name()
        };
        format!("{units:.0} {unit}")
    } else {
        format!("{:.3} kg", units / good.units_per_price_unit() as f64)
    }
}

fn hover_index(position: Option<Vec2>, over: bool, count: usize) -> Option<usize> {
    position
        .filter(|point| {
            over && count > 0
                && point.x >= -0.5
                && point.x <= 0.5
                && point.y >= -0.5
                && point.y <= 0.5
        })
        .map(|point| (((point.x + 0.5) * count as f32) as usize).min(count - 1))
}

fn hover_label(good: Good, day: &Day) -> String {
    format!(
        "{} to {} | {}\nPrice: {:.6} coins/{} | total traded: {}\n{}\nAffordable unmet demand: {} | unsold stock: {} (caravan: {})",
        format_clock(day.start),
        format_clock(day.end),
        if day.partial {
            "PARTIAL | live quantities so far"
        } else {
            "CLOSED | quantities at close"
        },
        day.price,
        good.price_unit_name(),
        crate::quantity_label(good, day.activity.traded_units),
        market::trade_breakdown(good, day.activity),
        crate::quantity_label(good, day.activity.affordable_demand_units),
        crate::quantity_label(good, day.activity.listed_units),
        crate::quantity_label(good, day.activity.caravan_listed_units)
    )
}

pub fn hover(
    selection: Res<market::Selection>,
    cached: Res<ChartDisplay>,
    plots: Query<&RelativeCursorPosition, With<Plot>>,
    mut readouts: Query<&mut Text, With<Hover>>,
) {
    if selection.view != market::View::Market {
        return;
    }
    let selected = plots
        .iter()
        .find_map(|cursor| hover_index(cursor.normalized, cursor.cursor_over, cached.days.len()));
    let value = selected.map_or_else(
        || "Hover a market day for exact figures. Cyan body marks the current partial day.".into(),
        |index| {
            let day = &cached.days[index];
            hover_label(selection.good(), day)
        },
    );
    for mut text in &mut readouts {
        if text.0 != value {
            text.0.clone_from(&value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use learning_lord_simulation::{
        Citizen,
        marketplace::{Prices, ShoppingList},
    };

    #[test]
    fn hover_distinguishes_trades_from_deliveries_for_closed_and_live_days() {
        let mut day = Day {
            start: 0,
            end: UPDATE_TIME_MS,
            price: 20.0,
            partial: false,
            activity: GoodActivity {
                traded_units: 6,
                local_traded_units: 1,
                local_traded_coins: 20,
                exported_units: 2,
                exported_coins: 15,
                imported_units: 4,
                caravan_purchased_units: 3,
                caravan_purchased_coins: 91,
                caravan_listed_units: 1,
                ..default()
            },
        };
        let label = hover_label(Good::Bread, &day);
        assert!(label.contains("CLOSED | quantities at close"));
        assert!(label.contains("total traded: 6 loaves"));
        assert!(label.contains("Local trades: 1 loaf | 20 coins"));
        assert!(label.contains("Exports: 2 loaves | 15 gross coins entering town"));
        assert!(label.contains("Import deliveries: 4 loaves"));
        assert!(label.contains("Bought from caravans: 3 loaves | 91 gross coins spent"));
        assert!(label.contains("caravan: 1 loaf"));
        day.partial = true;
        assert!(hover_label(Good::Bread, &day).contains("PARTIAL | live quantities so far"));
    }

    #[test]
    fn closed_history_preserves_caravan_delivery_and_remaining_stock() {
        let (universe, buyer) = Universe::default()
            .with_prices(
                Prices::default()
                    .with_price(Good::Bread, Good::Bread.core_price() * 2.0)
                    .unwrap(),
            )
            .with_citizen(
                "Buyer",
                Citizen::new(0.0).unwrap().with_coins(1000).unwrap(),
            )
            .unwrap();
        let universe = universe
            .with_purchase_request(buyer, ShoppingList::single(Good::Bread, 6))
            .unwrap()
            .advance(UPDATE_TIME_MS + DAY_MS)
            .unwrap();
        let values = days(&universe, Good::Bread);
        let closed = values
            .iter()
            .find(|day| day.start == UPDATE_TIME_MS)
            .unwrap();
        assert!(!closed.partial);
        assert_eq!(closed.activity.imported_units, 3);
        assert_eq!(closed.activity.caravan_listed_units, 3);
        assert_eq!(closed.activity.traded_units, 0);
        assert!(hover_label(Good::Bread, closed).contains("Import deliveries: 3 loaves"));
    }

    #[test]
    fn fills_inactive_days_without_precreation_history_and_carries_prices() {
        let (universe, buyer) = Universe::starting_at(6 * 60 * 60 * 1000)
            .with_citizen("Buyer", Citizen::new(0.0).unwrap().with_coins(10).unwrap())
            .unwrap();
        let universe = universe
            .with_purchase_request(buyer, ShoppingList::single(Good::Water, 100))
            .unwrap();
        let universe = universe.advance(DAY_MS).unwrap();
        let old_price = universe
            .market()
            .history()
            .iter()
            .find(|entry| entry.good == Good::Water)
            .unwrap()
            .price_before;
        let new_price = universe.prices().price(Good::Water).unwrap();
        let values = days(&universe, Good::Water);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].start, 6 * 60 * 60 * 1000);
        assert_eq!(values[0].end, DAY_MS + UPDATE_TIME_MS);
        assert_eq!(values[0].price, old_price);
        assert_eq!(values[0].activity.affordable_demand_units, 100);
        assert_eq!(values[1].price, new_price);
        let universe = universe
            .with_purchase_request(buyer, ShoppingList::default())
            .unwrap()
            .advance(3 * DAY_MS)
            .unwrap();
        let values = days(&universe, Good::Water);
        assert_eq!(values.len(), 5);
        assert!(
            values[1..]
                .iter()
                .all(|day| day.price == new_price && day.activity == GoodActivity::default())
        );
        assert!(values.last().unwrap().partial);
        let berries = days(&universe, Good::Berries);
        assert!(
            berries
                .iter()
                .all(|day| day.activity == GoodActivity::default())
        );
    }

    #[test]
    fn initial_short_period_and_recent_window_are_bounded() {
        let universe = Universe::default().advance(UPDATE_TIME_MS).unwrap();
        let values = days(&universe, Good::Bread);
        assert_eq!(values.len(), 2);
        assert_eq!((values[0].start, values[0].end), (0, UPDATE_TIME_MS));
        let values = days(&universe.advance(40 * DAY_MS).unwrap(), Good::Bread);
        assert_eq!(values.len(), WINDOW);
        assert_eq!(values[1].start - values[0].start, DAY_MS);
        assert_eq!(
            days(&Universe::starting_at(6 * 60 * 60 * 1000), Good::Water).len(),
            1
        );
    }

    #[test]
    fn separate_scales_preserve_linear_quantities_and_finite_geometry() {
        let mut values = days(
            &Universe::default().with_prices(Prices::new(f64::MAX).unwrap()),
            Good::Water,
        );
        values[0].activity = GoodActivity {
            traded_units: u64::MAX,
            affordable_demand_units: u64::MAX,
            listed_units: u64::MAX,
            ..default()
        };
        let scale = Scale::new(&values);
        assert_eq!(scale.y(f64::MAX), HEIGHT / 2.0);
        assert!(scale.quantity(u64::MAX as f64).is_finite());
        assert_eq!(
            scale.quantity(u64::MAX as f64 / 2.0),
            scale.quantity(u64::MAX as f64) / 2.0
        );
        assert!(scale.quantity(u64::MAX as f64) * 1.5 <= 80.0);
        assert_eq!(scale.quantity(0.0), 0.0);
        assert!(Scale::new(&[]).y(0.0).is_finite());
    }

    #[test]
    fn hidden_charts_and_paused_visible_charts_do_not_rebuild() {
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(crate::simulation::Snapshot {
            universe: Universe::default(),
            error: None,
            mutation_error: None,
            mutation_revision: 0,
            planning_history: Default::default(),
            generation: 0,
            revision: 0,
        }))
        .init_resource::<market::Selection>()
        .init_resource::<ChartDisplay>()
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(spawn);
        })
        .add_systems(Update, (refresh, hover).chain());
        app.update();
        assert!(app.world().resource::<ChartDisplay>().days.is_empty());
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Market;
        app.update();
        assert_eq!(app.world().resource::<ChartDisplay>().days.len(), 1);
        let entities: Vec<Entity> = app
            .world_mut()
            .query::<Entity>()
            .iter(app.world())
            .collect();
        app.world_mut().resource_mut::<DisplaySnapshot>().0.universe =
            Universe::default().advance(1).unwrap();
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<Entity>()
                .iter(app.world())
                .collect::<Vec<_>>(),
            entities
        );
        assert_eq!(app.world().resource::<ChartDisplay>().days[0].end, 1);
        app.world_mut().clear_trackers();
        app.update();
        assert!(!app.world().resource_ref::<ChartDisplay>().is_changed());
        for (node, text) in app
            .world_mut()
            .query::<(Ref<Node>, Option<Ref<Text>>)>()
            .iter(app.world())
        {
            assert!(!node.is_changed());
            assert!(text.is_none_or(|value| !value.is_changed()));
        }
    }

    #[test]
    fn chart_quantities_distinguish_bulk_and_counted_goods() {
        assert_eq!(chart_quantity_label(Good::Berries, 1500.0), "1.500 kg");
        assert_eq!(chart_quantity_label(Good::Bread, 1.0), "1 loaf");
        assert_eq!(chart_quantity_label(Good::Bread, 2.0), "2 loaves");
        assert_eq!(chart_quantity_label(Good::BerryPie, 1.0), "1 pie");
        assert_eq!(chart_quantity_label(Good::BerryPie, 3.0), "3 pies");
        assert_eq!(chart_quantity_label(Good::FlaxBlock, 1.0), "1 block");
        assert_eq!(chart_quantity_label(Good::FlaxGarment, 2.0), "2 garments");
    }

    #[test]
    fn hover_respects_plot_bounds_and_day_slots() {
        assert_eq!(
            hover_index(Some(Vec2::new(-0.49, -0.49)), true, 30),
            Some(0)
        );
        assert_eq!(hover_index(Some(Vec2::new(0.5, 0.5)), true, 30), Some(29));
        assert_eq!(hover_index(Some(Vec2::new(0.5, 0.5)), true, 1), Some(0));
        assert_eq!(hover_index(Some(Vec2::new(0.0, 0.0)), false, 30), None);
        assert_eq!(hover_index(Some(Vec2::new(0.0, -0.6)), true, 30), None);
        assert_eq!(hover_index(Some(Vec2::new(0.5, 0.5)), true, 0), None);
    }
}
