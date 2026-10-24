use crate::{
    MUTED, PANEL, SELECTED, TEXT, citizens,
    collectives::{label, place_link, row},
    locations,
    navigation::{self, Target},
    text,
};
use bevy::prelude::*;
use learning_lord_simulation::{
    Universe, household_history::DailyHouseholdHistory, marketplace::Good,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum View {
    #[default]
    Current,
    History,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    View(View),
    Day(Option<u64>),
}

fn button<C: Component>(
    parent: &mut ChildSpawnerCommands,
    choice: C,
    caption: &str,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            choice,
            BackgroundColor(if selected { SELECTED } else { PANEL }),
            Node {
                padding: UiRect::all(px(8)),
                min_height: px(32),
                ..default()
            },
        ))
        .with_children(|button| {
            button.spawn(text(caption, 14.0, TEXT));
        });
}

pub(crate) fn controls(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    id: learning_lord_simulation::AgentId,
    view: View,
    selected: Option<u64>,
) {
    let Some(history) = universe.household_history(id) else {
        return;
    };
    day_controls(
        parent,
        view,
        selected,
        (history.current.start_ms, history.current.end_ms),
        &history
            .completed
            .iter()
            .map(|day| (day.start_ms, day.end_ms))
            .collect::<Vec<_>>(),
        Choice::View,
        Choice::Day,
    );
}

pub(crate) fn day_controls<C: Component>(
    parent: &mut ChildSpawnerCommands,
    view: View,
    selected: Option<u64>,
    current: (u64, u64),
    completed: &[(u64, u64)],
    view_choice: impl Fn(View) -> C,
    day_choice: impl Fn(Option<u64>) -> C,
) {
    row(parent, |row| {
        button(
            row,
            view_choice(View::Current),
            "Current",
            view == View::Current,
        );
        button(
            row,
            view_choice(View::History),
            "Daily history",
            view == View::History,
        );
    });
    if view != View::History {
        return;
    }
    parent.spawn(text(
        "Days close at 04:00 · latest 365 completed days",
        14.0,
        MUTED,
    ));
    let index = selected
        .and_then(|start| completed.iter().position(|day| day.0 == start))
        .unwrap_or(completed.len());
    let day = completed.get(index).copied().unwrap_or(current);
    row(parent, |row| {
        if index > 0 {
            button(
                row,
                day_choice(Some(completed[index - 1].0)),
                "Previous day",
                false,
            );
        }
        label(
            row,
            format!(
                "{} to {}",
                crate::format_clock(day.0),
                crate::format_clock(day.1)
            ),
        );
        if index + 1 < completed.len() {
            button(
                row,
                day_choice(Some(completed[index + 1].0)),
                "Next day",
                false,
            );
        }
        if selected.is_some() {
            button(row, day_choice(None), "Today / current", false);
        }
    });
    if completed.is_empty() {
        label(parent, "No completed days yet.");
    }
}

pub(crate) fn selected_record(
    universe: &Universe,
    id: learning_lord_simulation::AgentId,
    view: View,
    selected: Option<u64>,
) -> Option<&DailyHouseholdHistory> {
    let history = universe.household_history(id)?;
    Some(record(history, view, selected))
}

fn record(
    history: &learning_lord_simulation::household_history::HouseholdHistory,
    view: View,
    selected: Option<u64>,
) -> &DailyHouseholdHistory {
    if view == View::Current {
        return &history.current;
    }
    selected
        .and_then(|start| history.completed.iter().find(|day| day.start_ms == start))
        .unwrap_or(&history.current)
}

fn goods(
    parent: &mut ChildSpawnerCommands,
    produced: &[u64; Good::COUNT],
    bought: &[u64; Good::COUNT],
    sold: &[u64; Good::COUNT],
    consumed: &[u64; Good::COUNT],
    taxed: &[u64; Good::COUNT],
) {
    let mut any = false;
    for good in Good::ALL {
        let i = good as usize;
        if [produced[i], bought[i], sold[i], consumed[i], taxed[i]]
            .iter()
            .all(|n| *n == 0)
        {
            continue;
        }
        any = true;
        row(parent, |row| {
            navigation::link(row, good.name(), Target::Good(good));
            label(
                row,
                format!(
                    "Produced {} | Bought {} | Sold {} | Consumed {} | Tax collected {}",
                    crate::quantity_label(good, produced[i]),
                    crate::quantity_label(good, bought[i]),
                    crate::quantity_label(good, sold[i]),
                    crate::quantity_label(good, consumed[i]),
                    crate::quantity_label(good, taxed[i])
                ),
            );
        });
    }
    if !any {
        label(parent, "No goods activity.");
    }
}

pub(crate) fn render(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    day: &DailyHouseholdHistory,
    partial: bool,
) {
    parent.spawn(text(
        if partial {
            "CURRENT DAY · partial totals"
        } else {
            "COMPLETED DAY"
        },
        14.0,
        MUTED,
    ));
    parent.spawn(text("CASH", 14.0, MUTED));
    label(
        parent,
        format!(
            "Opening {} | Sales receipts {} | Purchases {} | Taxes paid {} | Workshop fees paid {} | Closing {} coins",
            day.opening_coins,
            day.sales_receipts,
            day.purchases,
            day.taxes_paid,
            day.workshop_fees_paid,
            day.closing_coins
        ),
    );
    if let Some(cashflow) = crate::construction::cashflow(day.construction) {
        label(parent, cashflow);
    }
    parent.spawn(text("GOODS", 14.0, MUTED));
    goods(
        parent,
        &day.produced,
        &day.bought,
        &day.sold,
        &day.consumed,
        &day.tax_collected,
    );
    parent.spawn(text("MEMBER CONTRIBUTIONS", 14.0, MUTED));
    let mut members = day.members.iter().collect::<Vec<_>>();
    members.sort_by_key(|(id, _)| id.0);
    if members.is_empty() {
        label(parent, "No member contributions yet.");
    }
    for (id, member) in members {
        row(parent, |row| {
            navigation::link(row, locations::name(universe, *id), Target::Citizen(*id));
            label(
                row,
                format!(
                    "Production / gathering {} | Travel {} | Supply work {}",
                    citizens::duration(member.production_ms),
                    citizens::duration(member.travel_ms),
                    citizens::duration(member.supply_ms)
                ),
            );
        });
        for good in Good::ALL {
            let i = good as usize;
            if member.produced[i] == 0 && member.consumed[i] == 0 {
                continue;
            }
            row(parent, |row| {
                navigation::link(row, good.name(), Target::Good(good));
                label(
                    row,
                    format!(
                        "Produced {} | Consumed {}",
                        crate::quantity_label(good, member.produced[i]),
                        crate::quantity_label(good, member.consumed[i])
                    ),
                );
            });
        }
    }
    parent.spawn(text(
        if partial {
            "CURRENT SUPPLIES"
        } else {
            "CLOSING SUPPLIES"
        },
        14.0,
        MUTED,
    ));
    if day.closing_stocks.is_empty() {
        label(parent, "None");
    }
    for stock in &day.closing_stocks {
        row(parent, |row| {
            if let Some(place) = stock.place {
                place_link(row, universe, place);
            }
            if let Some(carrier) = stock.carrier {
                navigation::link(
                    row,
                    locations::name(universe, carrier),
                    Target::Citizen(carrier),
                );
            }
            navigation::link(row, stock.good.name(), Target::Good(stock.good));
            let available = stock
                .units
                .saturating_sub(stock.listed_units)
                .saturating_sub(stock.tax_reserved_units)
                .saturating_sub(stock.work_reserved_units);
            label(
                row,
                format!(
                    "{} total | {} available | {} listed | {} tax reserved | {} work reserved",
                    crate::quantity_label(stock.good, stock.units),
                    crate::quantity_label(stock.good, available),
                    crate::quantity_label(stock.good, stock.listed_units),
                    crate::quantity_label(stock.good, stock.tax_reserved_units),
                    crate::quantity_label(stock.good, stock.work_reserved_units)
                ),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_selection_survives_day_rollover_and_expiration_returns_current() {
        let universe = crate::simulation::new_universe().unwrap();
        let id = *universe.households().keys().next().unwrap();
        let mut history = universe.household_history(id).unwrap().clone();
        let day = history.current.clone();
        let start = day.start_ms;
        let selected = Some(start);
        history.completed.push_back(day);
        history.current.start_ms += 86_400_000;
        assert_eq!(record(&history, View::History, selected).start_ms, start);
        let next = history.current.clone();
        history.completed.push_back(next);
        history.current.start_ms += 86_400_000;
        assert_eq!(record(&history, View::History, selected).start_ms, start);
        history.completed.pop_front();
        assert_eq!(record(&history, View::History, selected), &history.current);
        assert_eq!(record(&history, View::History, None), &history.current);
    }
}
