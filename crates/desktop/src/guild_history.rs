use crate::{
    MUTED, TEXT,
    collectives::{label, place_link, row},
    household_history::{self, View},
    locations,
    navigation::{self, Target},
    text,
};
use bevy::prelude::*;
use learning_lord_simulation::{
    AgentId, Universe,
    guild_history::{DailyGuildHistory, ProductionContribution},
    marketplace::Good,
};

#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    View(View),
    Day(Option<u64>),
}

pub(crate) fn controls(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    id: AgentId,
    view: View,
    selected: Option<u64>,
) {
    let Some(history) = universe.guild_history(id) else {
        return;
    };
    household_history::day_controls(
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

pub(crate) fn selected_record(
    universe: &Universe,
    id: AgentId,
    selected: Option<u64>,
) -> Option<&DailyGuildHistory> {
    let history = universe.guild_history(id)?;
    Some(record(history, selected))
}

fn record(
    history: &learning_lord_simulation::guild_history::GuildHistory,
    selected: Option<u64>,
) -> &DailyGuildHistory {
    selected
        .and_then(|start| history.completed.iter().find(|day| day.start_ms == start))
        .unwrap_or(&history.current)
}

fn activity(parent: &mut ChildSpawnerCommands, contribution: &ProductionContribution) {
    label(
        parent,
        format!(
            "Production worker-hours: {:.3}",
            contribution.production_ms as f64 / 3_600_000.0
        ),
    );
    let mut any = false;
    for good in Good::ALL {
        let units = contribution.produced[good as usize];
        if units == 0 {
            continue;
        }
        any = true;
        row(parent, |row| {
            navigation::link(row, good.name(), Target::Good(good));
            label(
                row,
                format!("Produced {}", crate::quantity_label(good, units)),
            );
        });
    }
    if !any {
        label(parent, "No completed output.");
    }
}

pub(crate) fn render(
    parent: &mut ChildSpawnerCommands,
    universe: &Universe,
    day: &DailyGuildHistory,
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
    parent.spawn(text("CASH AND FEES", 14.0, MUTED));
    label(
        parent,
        format!(
            "Opening {} | Fees accrued {:.3} | Payments received {} | Expenses paid {} | Closing {} coins",
            day.opening_coins,
            day.fees_accrued.liability(),
            day.payments_received,
            day.expenses_paid,
            day.closing_coins
        ),
    );
    if let Some(cashflow) = crate::construction::cashflow(day.construction) {
        label(parent, cashflow);
    }
    for (caption, debts) in [
        ("OPENING OUTSTANDING FEES", &day.opening_outstanding),
        (
            if partial {
                "CURRENT OUTSTANDING FEES"
            } else {
                "CLOSING OUTSTANDING FEES"
            },
            &day.closing_outstanding,
        ),
    ] {
        parent.spawn(text(caption, 14.0, MUTED));
        let mut accounts = debts.iter().collect::<Vec<_>>();
        accounts.sort_by_key(|(id, _)| id.0);
        if accounts.is_empty() {
            label(parent, "None");
        }
        for (id, debt) in accounts {
            row(parent, |row| {
                navigation::render(row, &[navigation::account(universe, *id)], 14.0, TEXT);
                label(
                    row,
                    format!(
                        "Owed {:.3} coins | Whole debt {} | Fractional carry {:.3} coins",
                        debt.liability(),
                        debt.unpaid,
                        debt.liability() - debt.unpaid as f64
                    ),
                );
            });
        }
    }
    parent.spawn(text("MEMBER PRODUCTION", 14.0, MUTED));
    let mut contributions = day.contributions.iter().collect::<Vec<_>>();
    contributions.sort_by_key(|((member, workshop), _)| (member.0, workshop.0));
    if contributions.is_empty() {
        label(parent, "No production yet.");
    }
    for ((member, workshop), contribution) in contributions {
        row(parent, |row| {
            navigation::link(
                row,
                locations::name(universe, *member),
                Target::Citizen(*member),
            );
            label(row, "at");
            place_link(row, universe, *workshop);
        });
        activity(parent, contribution);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_survives_rollover_and_pruning_returns_current() {
        let universe = crate::simulation::new_universe().unwrap();
        let id = *universe.guilds().keys().next().unwrap();
        let mut history = universe.guild_history(id).unwrap().clone();
        let selected_start = history.current.start_ms;
        let selected = Some(selected_start);
        history.completed.push_back(history.current.clone());
        history.current.start_ms += 86_400_000;
        assert_eq!(record(&history, selected).start_ms, selected_start);
        history.completed.push_back(history.current.clone());
        history.current.start_ms += 86_400_000;
        assert_eq!(record(&history, selected).start_ms, selected_start);
        history.completed.pop_front();
        assert_eq!(record(&history, selected), &history.current);
        assert_eq!(record(&history, None), &history.current);
    }

    #[test]
    fn day_controls_link_adjacent_days_and_current() {
        let mut world = World::new();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        Commands::new(&mut queue, &world)
            .spawn(Node::default())
            .with_children(|parent| {
                household_history::day_controls(
                    parent,
                    View::History,
                    Some(20),
                    (40, 50),
                    &[(10, 20), (20, 30), (30, 40)],
                    Choice::View,
                    Choice::Day,
                );
            });
        queue.apply(&mut world);
        let days = world
            .query::<&Choice>()
            .iter(&world)
            .filter_map(|choice| match choice {
                Choice::Day(day) => Some(*day),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(days, vec![Some(10), Some(30), None]);
    }

    #[test]
    fn history_links_output_members_workshops_and_debt_accounts() {
        let universe = crate::simulation::new_universe().unwrap();
        let guild = universe.guilds().values().next().unwrap();
        let member = guild.members[0];
        let workshop = guild.workshops[0];
        let mut day = universe.guild_history(guild.id).unwrap().current.clone();
        let mut contribution = ProductionContribution {
            production_ms: 5_400_000,
            ..default()
        };
        contribution.produced[Good::Cloth as usize] = 100;
        day.contributions.insert((member, workshop), contribution);
        day.closing_outstanding.insert(
            member,
            learning_lord_simulation::guilds::WorkshopFeeAccount {
                unpaid: 2,
                remainder: 1_800_000,
            },
        );
        let mut world = World::new();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        Commands::new(&mut queue, &world)
            .spawn(Node::default())
            .with_children(|parent| render(parent, &universe, &day, false));
        queue.apply(&mut world);
        let links = world
            .query::<&Target>()
            .iter(&world)
            .copied()
            .collect::<Vec<_>>();
        assert!(links.contains(&Target::Citizen(member)));
        assert!(links.contains(&Target::Location(workshop)));
        assert!(links.contains(&Target::Good(Good::Cloth)));
        let labels = world
            .query::<&Text>()
            .iter(&world)
            .map(|t| t.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(labels.contains("Production worker-hours: 1.500"));
        assert!(labels.contains("Owed 2.500 coins"));
        assert!(labels.contains("COMPLETED DAY"));
    }
}
