mod caravan_controls;
mod citizen_history;
mod citizens;
mod debug_export;
mod locations;
mod map_view;
mod market;
mod market_history;
mod simulation;

use bevy::{
    ecs::system::SystemParam,
    input_focus::{FocusCause, InputFocus},
    prelude::*,
    render::pipelined_rendering::PipelinedRenderingPlugin,
    winit::{EventLoopProxyWrapper, UpdateMode, WinitSettings, WinitUserEvent},
};
use learning_lord_simulation::{AgentKind, CitizenAction, Universe, marketplace::Good};
use simulation::{Command as SimulationCommand, SPEEDS, SimulationWorker, Snapshot};

const BACKGROUND: Color = Color::srgb(0.06, 0.08, 0.10);
const PANEL: Color = Color::srgb(0.10, 0.13, 0.16);
const TEXT: Color = Color::srgb(0.92, 0.95, 0.94);
const MUTED: Color = Color::srgb(0.58, 0.66, 0.69);
const SELECTED: Color = Color::srgb(0.12, 0.38, 0.30);

#[derive(SystemParam)]
struct KeyboardShortcuts<'w> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    locations: Option<Res<'w, locations::State>>,
    caravan: Option<Res<'w, caravan_controls::State>>,
}

impl KeyboardShortcuts<'_> {
    fn editing(&self) -> bool {
        self.locations
            .as_ref()
            .is_some_and(|editor| editor.editing())
            || self.caravan.as_ref().is_some_and(|editor| editor.editing())
    }
}

#[derive(Resource)]
struct Controls {
    running: bool,
    speed: u32,
    error: Option<String>,
    generation: u64,
    restart_pending: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            running: false,
            speed: 1,
            error: None,
            generation: 0,
            restart_pending: false,
        }
    }
}

#[derive(Resource)]
struct DisplaySnapshot(Snapshot);

#[derive(Component, Clone, Copy)]
enum Control {
    ToggleRunning,
    Step,
    NextDay,
    Restart,
    Export,
    Speed(u32),
}

#[derive(Component)]
enum Readout {
    Clock,
    Market,
    Decision,
    Status,
    Citizen,
    Error,
    RunButton,
    ExportStatus,
}

fn main() -> Result<(), String> {
    let rate = simulation::update_rate()?;
    let universe = simulation::new_universe().map_err(|error| error.to_string())?;
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Learning Lord".into(),
                    name: Some("learning-lord".into()),
                    resolution: (1280, 800).into(),
                    ..default()
                }),
                ..default()
            })
            // Reactive updates must render now; another frame is not guaranteed.
            .disable::<PipelinedRenderingPlugin>(),
    );
    let proxy = (**app.world().resource::<EventLoopProxyWrapper>()).clone();
    let worker = SimulationWorker::spawn_with_wakeup(universe, rate, move || {
        let _ = proxy.send_event(WinitUserEvent::WakeUp);
    });
    app.insert_resource(WinitSettings {
        focused_mode: UpdateMode::reactive_low_power(std::time::Duration::MAX),
        unfocused_mode: UpdateMode::reactive_low_power(std::time::Duration::MAX),
    })
    .insert_resource(ClearColor(BACKGROUND))
    .insert_resource(DisplaySnapshot(worker.snapshot()))
    .insert_resource(worker)
    .init_resource::<Controls>()
    .init_resource::<citizens::Selection>()
    .init_resource::<citizens::RosterPage>()
    .init_resource::<citizens::PlanDisplay>()
    .init_resource::<citizen_history::Selection>()
    .init_resource::<market::Selection>()
    .init_resource::<locations::State>()
    .init_resource::<caravan_controls::State>()
    .init_resource::<market::OrderDisplay>()
    .init_resource::<market_history::ChartDisplay>()
    .init_resource::<debug_export::DebugExport>()
    .init_resource::<InputFocus>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            poll_worker,
            locations::handle,
            caravan_controls::handle,
            handle_controls,
            citizens::handle_selection,
            citizen_history::handle_selection,
            citizen_history::refresh,
            market::handle_selection,
            refresh_display,
            citizens::refresh_cards,
            citizens::refresh_page_label,
            citizens::refresh_plan,
            citizens::scroll_panels,
            map_view::refresh,
            market::refresh_choices,
            market::refresh,
            locations::refresh,
            caravan_controls::refresh,
            market_history::refresh,
            market_history::hover,
        )
            .chain(),
    )
    .run();
    Ok(())
}

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn button(control: Control) -> impl Bundle {
    (
        Button,
        control,
        Node {
            min_width: px(76),
            height: px(36),
            padding: UiRect::horizontal(px(12)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(PANEL),
    )
}

fn setup(mut commands: Commands, snapshot: Res<DisplaySnapshot>) {
    commands.spawn(Camera2d);
    commands.spawn(Node {
        width: percent(100), height: percent(100), padding: UiRect::all(px(16)),
        flex_direction: FlexDirection::Column, row_gap: px(10), ..default()
    }).with_children(|root| {
        root.spawn(Node { align_items: AlignItems::Center, column_gap: px(20), ..default() })
            .with_children(|row| {
                row.spawn(text("LEARNING LORD", 17.0, MUTED));
                row.spawn((text("", 26.0, TEXT), Readout::Clock));
                row.spawn((text("", 16.0, MUTED), Readout::Status));
            });
        root.spawn(Node { column_gap: px(8), row_gap: px(8), flex_wrap: FlexWrap::Wrap, ..default() })
            .with_children(|row| {
                for (control, label) in [(Control::ToggleRunning, "Run"), (Control::Step, "Advance 30 min"),
                    (Control::Restart, "Restart universe"), (Control::Export, "Export universe")] {
                    row.spawn(button(control)).with_children(|button| {
                        if matches!(control, Control::ToggleRunning) {
                            button.spawn((text(label, 16.0, TEXT), Readout::RunButton));
                        } else { button.spawn(text(label, 16.0, TEXT)); }
                    });
                }
                for speed in SPEEDS {
                    row.spawn(button(Control::Speed(speed))).with_children(|button| {
                        button.spawn(text(format!("{speed}x"), 16.0, TEXT));
                    });
                }
            });
        root.spawn((text("", 14.0, MUTED), Readout::Market));
        root.spawn((text("", 13.0, MUTED), Readout::ExportStatus));
        market::spawn_tabs(root);
        root.spawn((market::ViewPanel(market::View::Citizens), Node {
            width: percent(100), flex_grow: 1.0, min_height: px(0),
            flex_direction: FlexDirection::Column, row_gap: px(10), ..default()
        })).with_children(|root| {
        citizens::spawn_roster(root);
        root.spawn(Node { width: percent(100), flex_grow: 1.0, min_height: px(0), column_gap: px(14), ..default() })
            .with_children(|row| {
                row.spawn((Node { width: px(324), flex_shrink: 0.0, flex_direction: FlexDirection::Column,
                    row_gap: px(12), min_height: px(0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default(), bevy::ui::RelativeCursorPosition::default()))
                    .with_children(|left| {
                        map_view::spawn(left, snapshot.0.universe.map().places().len().max(14), snapshot.0.universe.agents().len().max(simulation::STARTING_CITIZENS.len()));
                    });
                citizens::spawn_details(row);
            });
        });
        market::spawn(root);
        locations::spawn(root);
        root.spawn(text("C/M/L: tabs   |   Up/Down: goods   |   Space: run / pause   |   Right: advance 30 min   |   Shift+Right: next day 04:00   |   1-6: select on page   |   1x = 1 minute / second   |   Scroll panels for more", 13.0, MUTED));
        root.spawn((text("", 14.0, Color::srgb(1.0, 0.55, 0.48)), Readout::Error));
    });
}

fn poll_worker(
    worker: Res<SimulationWorker>,
    mut snapshot: ResMut<DisplaySnapshot>,
    mut controls: ResMut<Controls>,
) {
    let Some(next) = worker.snapshot_after(snapshot.0.revision) else {
        return;
    };
    if controls.restart_pending && next.generation == controls.generation {
        return;
    }
    controls.generation = next.generation;
    controls.restart_pending = false;
    snapshot.0 = next;
    if let Some(error) = &snapshot.0.error {
        controls.running = false;
        controls.error = Some(error.clone());
    }
}

fn apply_control(control: Control, controls: &mut Controls, worker: &SimulationWorker) {
    if controls.restart_pending || controls.error.is_some() && !matches!(control, Control::Restart)
    {
        return;
    }
    let command = match control {
        Control::Export => return,
        Control::Restart => {
            controls.running = false;
            controls.speed = 1;
            controls.error = None;
            controls.restart_pending = true;
            SimulationCommand::Restart
        }
        Control::ToggleRunning => {
            controls.running = !controls.running;
            SimulationCommand::SetRunning(controls.running)
        }
        Control::Step if !controls.running => SimulationCommand::Step,
        Control::NextDay if !controls.running => SimulationCommand::NextDay,
        Control::Step | Control::NextDay => return,
        Control::Speed(speed) => {
            controls.speed = speed;
            SimulationCommand::SetSpeed(speed)
        }
    };
    if let Err(error) = worker.send(command) {
        controls.running = false;
        controls.error = Some(error);
        controls.restart_pending = false;
    }
}

fn handle_controls(
    shortcuts: KeyboardShortcuts,
    buttons: Query<(Entity, &Interaction, &Control), Changed<Interaction>>,
    mut focus: ResMut<InputFocus>,
    mut controls: ResMut<Controls>,
    worker: Res<SimulationWorker>,
    snapshot: Res<DisplaySnapshot>,
    mut exporter: ResMut<debug_export::DebugExport>,
) {
    let editing = shortcuts.editing();
    let keyboard = &shortcuts.keyboard;
    if !editing && keyboard.just_pressed(KeyCode::Space) {
        apply_control(Control::ToggleRunning, &mut controls, &worker);
    }
    if !editing && keyboard.just_pressed(KeyCode::ArrowRight) {
        let control =
            if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
                Control::NextDay
            } else {
                Control::Step
            };
        apply_control(control, &mut controls, &worker);
    }
    for (entity, interaction, control) in &buttons {
        if *interaction == Interaction::Pressed {
            focus.set(entity, FocusCause::Pressed);
            if matches!(control, Control::Export) {
                exporter.export(&snapshot.0.universe, std::time::SystemTime::now());
            } else {
                apply_control(*control, &mut controls, &worker);
            }
        }
    }
}

fn format_clock(time_ms: u64) -> String {
    let seconds = time_ms / 1000;
    format!(
        "Day {} | {} | {:02}:{:02}:{:02}",
        seconds / 86_400,
        learning_lord_simulation::calendar::Weekday::at(time_ms).name(),
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn citizen_readout(universe: &Universe, id: Option<learning_lord_simulation::AgentId>) -> String {
    let Some(agent) = citizens::selected_agent(universe, id) else {
        return "No citizens".into();
    };
    let AgentKind::Citizen(citizen) = &agent.kind;
    let wellbeing = citizen
        .personal_wellbeing()
        .map(|score| format!("{:.1}", if score == 0.0 { 0.0 } else { score }))
        .unwrap_or_else(|error| error.to_string());
    let action = citizen
        .active_action()
        .map(|active| {
            let name = match active.action() {
                CitizenAction::Eat => "Eating".into(),
                CitizenAction::Wait => "Waiting".into(),
                CitizenAction::Sleep => "Sleeping".into(),
                CitizenAction::EquipClothing => "Equipping clothing".into(),
                CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage) => {
                    "Foraging".into()
                }
                CitizenAction::BuyFood(good) => format!("Buying {}", good.name().to_lowercase()),
                CitizenAction::Produce(recipe) => recipe.name().into(),
                CitizenAction::ListExcess => "List excess goods".into(),
                CitizenAction::List(good, units) => {
                    format!("Listing {} {}", quantity_label(good, units), good.name())
                }
                CitizenAction::Buy(list) => format!("Buying {}", shopping_list_label(list)),
                CitizenAction::BuyAt { place, list } => format!(
                    "Buying {} at {}",
                    shopping_list_label(list),
                    citizen.map().place(place).unwrap().name
                ),
                CitizenAction::Withdraw(good, units) => {
                    format!(
                        "Withdrawing {} {}",
                        quantity_label(good, units),
                        good.name()
                    )
                }
                CitizenAction::Travel(id) => {
                    format!("Walking to {}", citizen.map().place(id).unwrap().name)
                }
            };
            let seconds = active.remaining_ms().div_ceil(1000);
            format!(
                "{name} | {:02}:{:02}:{:02} remaining",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )
        })
        .unwrap_or_else(|| "Idle".into());
    let replanning = citizen
        .active_plan()
        .map(|active| {
            let goal = active
                .plan()
                .goals()
                .first()
                .map(|goal| citizens::goal_label(goal.goal).to_string())
                .unwrap_or_else(|| "current goal".into());
            format!("Replan after {goal} completes")
        })
        .unwrap_or_else(|| "No active plan".into());
    let role = citizen
        .starting_role()
        .map_or_else(String::new, |role| format!(" | {}", role.name()));
    let inventory = Good::ALL
        .into_iter()
        .filter(|good| citizen.units(*good) != 0 || citizen.town_carried_units(*good) != 0)
        .map(|good| {
            format!(
                "{}: {} available | {} tax reserved | {} town-owned",
                good.name(),
                quantity_label(good, citizen.available_units(good)),
                quantity_label(good, citizen.tax_reserved_units(good)),
                quantity_label(good, citizen.town_carried_units(good))
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut properties = citizen
        .owned_properties()
        .map(|place| place.name.as_str())
        .collect::<Vec<_>>();
    properties.sort();
    let properties = properties.join(", ");
    let garment = citizen.garment_condition().map_or_else(
        || "None".into(),
        |condition| format!("{:.1}%", condition * 100.0),
    );
    format!(
        "{}{role}\n{inventory}\nCoins: {} | Wealth: {} coins\nHunger: {:.1}     Tiredness: {:.1}     Wellbeing: {wellbeing}\nClothing need: {:.1} | Garment condition: {garment}\nFood reserves: {:.1} nutrition | Reserve wellbeing: +{:.2}\nLocation: {}\n{action}\n{replanning}\nOwns: {properties}",
        agent.name,
        citizen.coins(),
        citizen
            .wealth()
            .map_or_else(|error| error.to_string(), |wealth| format!("{wealth:.3}")),
        citizen.hunger(),
        citizen.tiredness(),
        citizen.clothing_need(),
        citizen.food_nutrition(),
        citizen.food_reserve_wellbeing(),
        citizens::location_label(citizen)
    )
}

fn quantity_label(good: Good, units: u64) -> String {
    let unit = if units == 1 && good.units_per_price_unit() == 1 {
        match good {
            Good::FlaxBlock => "block",
            Good::FlaxGarment => "garment",
            _ => good.price_unit_name(),
        }
    } else {
        good.unit_name()
    };
    format!("{units} {unit}")
}

fn shopping_list_label(list: learning_lord_simulation::marketplace::ShoppingList) -> String {
    list.items()
        .map(|(good, units)| format!("{} {}", quantity_label(good, units), good.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn action_label(action: CitizenAction, citizen: &learning_lord_simulation::Citizen) -> String {
    match action {
        CitizenAction::Eat => "Eat".into(),
        CitizenAction::Wait => "Wait".into(),
        CitizenAction::Sleep => "Sleep".into(),
        CitizenAction::EquipClothing => "Equip clothing".into(),
        CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage) => {
            "Forage".into()
        }
        CitizenAction::BuyFood(good) => format!("Buy {}", good.name().to_lowercase()),
        CitizenAction::Produce(recipe) => recipe.name().into(),
        CitizenAction::ListExcess => "List excess goods".into(),
        CitizenAction::List(good, units) => {
            format!("List {} {}", quantity_label(good, units), good.name())
        }
        CitizenAction::Buy(list) => format!("Buy {}", shopping_list_label(list)),
        CitizenAction::BuyAt { place, list } => format!(
            "Buy {} at {}",
            shopping_list_label(list),
            citizen.map().place(place).unwrap().name
        ),
        CitizenAction::Withdraw(good, units) => {
            format!("Withdraw {} {}", quantity_label(good, units), good.name())
        }
        CitizenAction::Travel(id) => format!("Travel to {}", citizen.map().place(id).unwrap().name),
    }
}

fn sequence_readout(
    actions: &[CitizenAction],
    citizen: &learning_lord_simulation::Citizen,
) -> String {
    let mut labels = Vec::new();
    let mut index = 0;
    while index < actions.len() {
        let action = actions[index];
        let count = actions[index..]
            .iter()
            .take_while(|&&next| next == action)
            .count();
        let label = action_label(action, citizen);
        labels.push(if count == 1 {
            label.to_string()
        } else {
            format!("{label} x{count}")
        });
        index += count;
    }
    labels.join(" > ")
}

fn decision_readout(universe: &Universe, id: Option<learning_lord_simulation::AgentId>) -> String {
    use learning_lord_simulation::planning::goals::Effect;
    let Some(agent) = citizens::selected_agent(universe, id) else {
        return "Last planning decision\nNo citizen selected".into();
    };
    let AgentKind::Citizen(citizen) = &agent.kind;
    let Some(decision) = citizen
        .active_plan()
        .and_then(|active| active.plan().decision())
    else {
        return "Last planning decision\nNo planning decision yet".into();
    };
    let mut lines = vec![
        "Last planning decision".to_string(),
        format!(
            "Prices used: berries {:.3} coins/kg",
            decision.prices.price(Good::Berries).unwrap()
        ),
    ];
    for candidate in &decision.candidates {
        let name = match candidate.goal {
            Effect::ReduceHunger => "Hunger",
            Effect::ReduceTiredness => "Sleep",
            Effect::ReduceClothingNeed => "Clothing",
            Effect::Production => "Production",
            Effect::ReplenishReserves => "Replenish reserves",
            Effect::ListExcess => "List excess",
            _ => unreachable!("decision candidates are goals"),
        };
        let selected = if candidate.goal == decision.selected_goal {
            " [chosen first]"
        } else {
            ""
        };
        lines.push(format!("\n{name}{selected}"));
        if let Some(forecast) = &candidate.forecast {
            let seconds = forecast.duration_ms.div_ceil(1000);
            lines.push(format!(
                "{:02}:{:02}:{:02} | Goal avg {:.2} | Plan avg {:.2}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60,
                forecast.average_wellbeing,
                forecast.full_plan_wellbeing
            ));
            lines.push(sequence_readout(&forecast.actions, citizen));
        } else {
            lines.push(format!(
                "Unavailable: {}",
                candidate.unavailable_reason().unwrap()
            ));
        }
    }
    if !decision.production.is_empty() {
        lines.push("\nProduction recipes | last decision snapshot".into());
        lines.push("Inputs and targets below are from that decision, not current inventory.\nFeasibility reflects recorded root sequences and completed forecasts, including later goals; ordered prefixes can skip a recipe.".into());
        for recipe in &decision.production {
            let profit = recipe
                .profit_per_hour
                .map_or_else(|| "unknown".into(), |value| format!("{value:.3} coins/h"));
            let target = recipe
                .remaining_batches
                .map_or_else(|| "not tracked".into(), |value| format!("{value} batches"));
            lines.push(format!(
                "\n{} | Estimated profit {} | Target remaining {}",
                recipe.recipe.name(),
                profit,
                target
            ));
            if recipe.inputs.is_empty() {
                lines.push("Inputs per batch: none".into());
            } else {
                for input in &recipe.inputs {
                    lines.push(format!(
                        "Inputs per batch — {}: {} / {} carried | {} missing{}",
                        input.good.name(),
                        quantity_label(input.good, input.carried_units),
                        quantity_label(input.good, input.required_units),
                        quantity_label(
                            input.good,
                            input.required_units.saturating_sub(input.carried_units)
                        ),
                        if input.supply_shortfall {
                            " | insufficient accessible supplies"
                        } else {
                            ""
                        }
                    ));
                }
            }
            let status = if recipe.selected {
                "Included in selected full plan (possibly after other goals)."
            } else if recipe.best_competing_score.is_some_and(|score| {
                score < citizen.active_plan().unwrap().plan().average_wellbeing()
            }) {
                "Feasible sequence found; another full plan scored better."
            } else if recipe.feasible_sequence_found {
                "Feasible sequence found; absent from selected full plan (ties can retain the earlier plan)."
            } else if recipe.inputs.iter().any(|input| input.supply_shortfall) {
                "No feasible sequence found; insufficient accessible input supplies."
            } else if recipe.remaining_batches == Some(0) {
                "No sequence found; production target already met (prerequisites or food goals may still use this recipe)."
            } else if recipe.profit_per_hour.is_some_and(|profit| profit <= 0.0) {
                "No sequence found; nonpositive estimated profit excludes this recipe from the production target queue."
            } else if recipe.prefix_attempted {
                "No feasible sequence found in explored prefixes; supplies, four-hour preparation limit or action rules blocked them."
            } else {
                "No sequence found; recipe not reached in production prefixes or production goal not compared."
            };
            lines.push(status.into());
            if recipe.preparation_limit_observed {
                lines.push("Observed exclusion: a tested prefix reached the four-hour preparation limit before a production step.".into());
            }
        }
    }
    lines.push(
        "\nChosen by full-plan time-weighted action endpoint averages.\nEach sequence leads to its goal's best full plan.\nGoal avg scores only the sequence shown.\nRepeated gathering segments form one order.".into(),
    );
    lines.join("\n")
}

fn refresh_display(
    controls: Res<Controls>,
    exporter: Res<debug_export::DebugExport>,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<citizens::Selection>,
    mut readouts: Query<(&Readout, &mut Text)>,
    mut buttons: Query<(&Control, &Interaction, &mut BackgroundColor)>,
) {
    for (readout, mut text) in &mut readouts {
        let value = match readout {
            Readout::Market => format!(
                "Market: {} goods | {} sell orders | {} trades",
                Good::COUNT,
                snapshot.0.universe.market().orders().count(),
                snapshot.0.universe.market().trades().len()
            ),
            Readout::Clock => format_clock(snapshot.0.universe.current_time_ms()),
            Readout::Status => format!(
                "{} | {}x",
                if controls.running {
                    "Running"
                } else {
                    "Paused"
                },
                controls.speed
            ),
            Readout::Citizen => citizen_readout(&snapshot.0.universe, selection.0),
            Readout::Decision => decision_readout(&snapshot.0.universe, selection.0),
            Readout::ExportStatus => exporter.status.clone(),
            Readout::Error => controls.error.clone().unwrap_or_default(),
            Readout::RunButton => if controls.running { "Pause" } else { "Run" }.into(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (control, interaction, mut color) in &mut buttons {
        let disabled = !matches!(control, Control::Export)
            && (controls.restart_pending
                || controls.error.is_some() && !matches!(control, Control::Restart)
                || matches!(control, Control::Step) && controls.running);
        let selected = matches!(control, Control::Speed(speed) if *speed == controls.speed);
        let next = if disabled {
            Color::srgb(0.09, 0.10, 0.11)
        } else if *interaction == Interaction::Pressed {
            Color::srgb(0.18, 0.48, 0.39)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.20, 0.27, 0.29)
        } else if selected || matches!(control, Control::ToggleRunning) {
            SELECTED
        } else {
            PANEL
        };
        color.set_if_neq(BackgroundColor(next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use learning_lord_simulation::Citizen;

    #[test]
    fn typing_in_location_editor_suppresses_simulation_shortcuts() {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<Controls>()
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<debug_export::DebugExport>()
            .insert_resource(locations::State::focused_fixture())
            .add_systems(Update, handle_controls);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.update();
        assert!(!app.world().resource::<Controls>().running);
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .universe
                .current_time_ms(),
            0
        );
    }

    #[test]
    fn typing_in_caravan_editor_suppresses_simulation_shortcuts() {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<Controls>()
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<debug_export::DebugExport>()
            .insert_resource(caravan_controls::State::focused_fixture())
            .add_systems(Update, handle_controls);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.update();
        assert!(!app.world().resource::<Controls>().running);
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .universe
                .current_time_ms(),
            0
        );
    }

    #[test]
    fn decision_panel_shows_reachable_hunger_and_uses_saved_prices() {
        use learning_lord_simulation::marketplace::Prices;
        let prices = Prices::new(2.0).unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(prices)
                .with_citizen("Ada", Citizen::new(0.0).unwrap().with_berries(35).unwrap())
                .unwrap();
        assert!(decision_readout(&universe, None).contains("No planning decision yet"));
        let planned = universe.start_planning(id).unwrap();
        let text = decision_readout(&planned, None);
        assert!(text.contains("Hunger"));
        assert!(text.contains("Replenish reserves"));
        let hunger = text
            .split("\nHunger")
            .nth(1)
            .unwrap()
            .split("\nSleep")
            .next()
            .unwrap();
        assert!(!hunger.contains("Unavailable:"));
        assert!(text.contains("Sleep"));
        assert!(text.contains("Production"));
        assert_eq!(text.matches("[chosen first]").count(), 1);
        assert!(text.contains("Goal avg"));
        assert!(text.contains("Plan avg"));
        assert!(text.contains("berries 2.000"));
        assert_eq!(
            decision_readout(
                &planned
                    .with_prices(Prices::new(1.0).unwrap())
                    .advance(1)
                    .unwrap(),
                None
            ),
            text
        );
        assert_eq!(
            sequence_readout(
                &[
                    CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
                    CitizenAction::Produce(learning_lord_simulation::production::Recipe::Forage),
                    CitizenAction::Eat
                ],
                &Citizen::new(0.0).unwrap()
            ),
            "Forage x2 > Eat"
        );
    }

    #[test]
    fn production_readout_explains_saved_supplies_and_targets() {
        use learning_lord_simulation::{StartingRole, production::Recipe};
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_citizen(
                    "Baker",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_starting_role(StartingRole::Baker)
                        .with_berries(930)
                        .unwrap(),
                )
                .unwrap();
        let (universe, _) = universe
            .with_property(id, Recipe::BakeBread.location())
            .unwrap();
        let universe = universe.start_planning(id).unwrap();
        let text = decision_readout(&universe, Some(id));
        assert!(text.contains("Production recipes | last decision snapshot"));
        assert!(text.contains("not current inventory"));
        assert!(text.contains("Bake bread | Estimated profit"));
        assert!(text.contains("Target remaining"));
        assert!(text.contains("insufficient accessible input supplies"));
        assert!(text.contains("100 g missing"));
        let later = universe.advance(1).unwrap();
        assert_eq!(text, decision_readout(&later, Some(id)));
    }

    #[test]
    fn carried_readout_separates_reserved_and_town_owned_goods() {
        use learning_lord_simulation::{
            locations::Location,
            production::{Recipe, Skill},
            taxation::{TaxKind, TaxRate},
        };
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_skill(Skill::Tailoring, 1.0)
            .unwrap()
            .with_good(Good::Cloth, 25)
            .unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_citizen("Ada", citizen)
                .unwrap();
        let (universe, place) = universe.with_property(id, Location::Tailory).unwrap();
        let (universe, _) = universe
            .with_tax_rule(
                place,
                "Socage",
                TaxKind::Socage {
                    rates: [(Good::FlaxBlock, TaxRate::new(10000).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let universe = universe
            .start_action(id, CitizenAction::Produce(Recipe::MakeClothingBlock))
            .unwrap();
        let AgentKind::Citizen(citizen) = &universe.agents()[&id].kind;
        let universe = universe
            .advance(citizen.active_action().unwrap().remaining_ms())
            .unwrap();
        let readout = citizen_readout(&universe, Some(id));
        assert!(
            readout.contains(
                "Flax block: 0 blocks available | 1 block tax reserved | 0 blocks town-owned"
            ),
            "{readout}"
        );
        let universe = universe
            .advance(
                learning_lord_simulation::calendar::FIRST_WEEKLY_SETTLEMENT_MS
                    - universe.current_time_ms(),
            )
            .unwrap();
        let readout = citizen_readout(&universe, Some(id));
        assert!(
            readout.contains("0 blocks tax reserved | 1 block town-owned"),
            "{readout}"
        );
    }

    #[test]
    fn readout_shows_berries_and_goal_completion_replanning() {
        let citizen = Citizen::with_needs(60.0, 100.0)
            .unwrap()
            .with_berries(310)
            .unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(learning_lord_simulation::marketplace::Prices::new(1.0).unwrap())
                .with_citizen("Ada", citizen)
                .unwrap();
        let universe = universe
            .start_planning(id)
            .unwrap()
            .advance(50_000)
            .unwrap();
        let readout = citizen_readout(&universe, None);
        assert!(readout.contains("Eating | 00:01:45 remaining"));
        assert!(readout.contains("Location:"));
        assert!(readout.contains("Replan after Reduce Hunger completes"));
        assert!(readout.contains("Berries: 155 g"));
        assert!(readout.contains("Clothing need: 20.0 | Garment condition: None"));
        assert!(!readout.contains("Water: 0 g"));
        assert!(!readout.contains("Bread: 0 loaves"));
        assert!(readout.contains("Coins: 0 | Wealth: 0.260 coins"));
    }

    #[test]
    fn sleeping_readout_counts_down_to_goal_completion() {
        let citizen = Citizen::with_needs(-50.0, 50.0).unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(learning_lord_simulation::marketplace::Prices::default())
                .with_citizen("Ada", citizen)
                .unwrap();
        let universe = universe
            .start_planning(id)
            .unwrap()
            .advance(2 * 60 * 60 * 1000)
            .unwrap();
        let readout = citizen_readout(&universe, None);
        assert!(readout.contains("Sleeping | 06:00:00 remaining"));
        assert!(readout.contains("Location: Ada's home"));
        assert!(readout.contains("Replan after Reduce Sleep Need completes"));
    }

    #[test]
    fn right_arrow_steps_and_either_shift_key_advances_to_next_day() {
        use std::time::{Duration, Instant};

        for shift in [None, Some(KeyCode::ShiftLeft), Some(KeyCode::ShiftRight)] {
            let worker = SimulationWorker::spawn(
                Universe::with_map(learning_lord_simulation::locations::Map::default()),
                60.try_into().unwrap(),
            );
            let mut app = App::new();
            app.insert_resource(DisplaySnapshot(worker.snapshot()))
                .insert_resource(worker)
                .init_resource::<Controls>()
                .init_resource::<debug_export::DebugExport>()
                .init_resource::<InputFocus>()
                .init_resource::<ButtonInput<KeyCode>>()
                .add_systems(Update, handle_controls);
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            if let Some(shift) = shift {
                keyboard.press(shift);
            }
            keyboard.press(KeyCode::ArrowRight);
            app.update();
            let expected = if shift.is_some() {
                28 * 60 * 60 * 1000
            } else {
                simulation::STEP_MS
            };
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let snapshot = app.world().resource::<SimulationWorker>().snapshot();
                assert!(snapshot.error.is_none());
                if snapshot.revision > 0 {
                    assert_eq!(snapshot.universe.current_time_ms(), expected);
                    break;
                }
                assert!(Instant::now() < deadline, "worker did not process shortcut");
                std::thread::yield_now();
            }
        }
    }

    #[test]
    fn ui_buttons_select_speeds_step_and_toggle_running() {
        use std::time::{Duration, Instant};

        let worker = SimulationWorker::spawn(
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(learning_lord_simulation::marketplace::Prices::default()),
            60.try_into().unwrap(),
        );
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<Controls>()
            .init_resource::<citizens::Selection>()
            .init_resource::<citizens::RosterPage>()
            .init_resource::<citizens::PlanDisplay>()
            .init_resource::<citizen_history::Selection>()
            .init_resource::<market::Selection>()
            .init_resource::<locations::State>()
            .init_resource::<caravan_controls::State>()
            .init_resource::<market::OrderDisplay>()
            .init_resource::<market_history::ChartDisplay>()
            .init_resource::<debug_export::DebugExport>()
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    poll_worker,
                    locations::handle,
                    caravan_controls::handle,
                    handle_controls,
                    citizens::handle_selection,
                    citizen_history::handle_selection,
                    citizen_history::refresh,
                    market::handle_selection,
                    refresh_display,
                    citizens::refresh_cards,
                    citizens::refresh_page_label,
                    citizens::refresh_plan,
                    citizens::scroll_panels,
                    map_view::refresh,
                    market::refresh_choices,
                    market::refresh,
                    locations::refresh,
                    caravan_controls::refresh,
                    market_history::refresh,
                    market_history::hover,
                )
                    .chain(),
            );
        app.update();
        app.update();
        app.world_mut().clear_trackers();
        app.update();
        assert!(!app.world().resource_ref::<DisplaySnapshot>().is_changed());
        assert!(
            !app.world()
                .resource_ref::<citizens::Selection>()
                .is_changed()
        );
        let mut unchanged = app.world_mut().query::<(
            Option<Ref<Text>>,
            Option<Ref<Node>>,
            Option<Ref<BackgroundColor>>,
            Option<Ref<UiTransform>>,
            Option<Ref<Visibility>>,
        )>();
        for (text, node, color, transform, visibility) in unchanged.iter(app.world()) {
            assert!(text.is_none_or(|value| !value.is_changed()));
            assert!(node.is_none_or(|value| !value.is_changed()));
            assert!(color.is_none_or(|value| !value.is_changed()));
            assert!(transform.is_none_or(|value| !value.is_changed()));
            assert!(visibility.is_none_or(|value| !value.is_changed()));
        }

        app.world_mut().clear_trackers();
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .set_changed();
        app.update();
        for (text, node, color, transform, visibility) in unchanged.iter(app.world()) {
            assert!(text.is_none_or(|value| !value.is_changed()));
            assert!(node.is_none_or(|value| !value.is_changed()));
            assert!(color.is_none_or(|value| !value.is_changed()));
            assert!(transform.is_none_or(|value| !value.is_changed()));
            assert!(visibility.is_none_or(|value| !value.is_changed()));
        }

        let buttons: Vec<_> = app
            .world_mut()
            .query::<(Entity, &Control)>()
            .iter(app.world())
            .map(|(entity, control)| (entity, *control))
            .collect();
        let step_button = buttons
            .iter()
            .find(|(_, control)| matches!(control, Control::Step))
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(step_button)
            .insert(Interaction::Hovered);
        app.update();
        assert_eq!(
            app.world()
                .entity(step_button)
                .get::<BackgroundColor>()
                .unwrap()
                .0,
            Color::srgb(0.20, 0.27, 0.29)
        );
        app.world_mut()
            .entity_mut(step_button)
            .insert(Interaction::None);
        app.update();
        for (entity, control) in &buttons {
            if let Control::Speed(speed) = control {
                app.world_mut()
                    .entity_mut(*entity)
                    .insert(Interaction::Pressed);
                app.update();
                assert_eq!(app.world().resource::<Controls>().speed, *speed);
                assert!(!app.world().resource::<Controls>().running);
                app.world_mut()
                    .entity_mut(*entity)
                    .insert(Interaction::None);
                app.update();
            }
        }
        let step = buttons
            .iter()
            .find(|(_, control)| matches!(control, Control::Step))
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(step)
            .insert(Interaction::Pressed);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if app
                .world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .current_time_ms()
                == simulation::STEP_MS
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "manual step did not reach the UI"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let clock = app
            .world_mut()
            .query::<(&Readout, &Text)>()
            .iter(app.world())
            .find(|(readout, _)| matches!(readout, Readout::Clock))
            .unwrap()
            .1;
        assert_eq!(clock.0, "Day 0 | Monday | 00:30:00");

        let toggle = buttons
            .iter()
            .find(|(_, control)| matches!(control, Control::ToggleRunning))
            .unwrap()
            .0;
        for running in [true, false] {
            app.world_mut()
                .entity_mut(toggle)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(app.world().resource::<Controls>().running, running);
            app.world_mut().entity_mut(toggle).insert(Interaction::None);
            app.update();
        }
        let restart = buttons
            .iter()
            .find(|(_, control)| matches!(control, Control::Restart))
            .unwrap()
            .0;
        let previous_prices = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .prices();
        app.world_mut()
            .entity_mut(restart)
            .insert(Interaction::Pressed);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if app.world().resource::<DisplaySnapshot>().0.generation == 1 {
                break;
            }
            assert!(Instant::now() < deadline, "restart did not reach the UI");
            std::thread::sleep(Duration::from_millis(1));
        }
        let snapshot = &app.world().resource::<DisplaySnapshot>().0;
        assert_eq!(
            snapshot.universe.current_time_ms(),
            simulation::START_TIME_MS
        );
        assert_eq!(snapshot.universe.prices(), previous_prices);
        assert_eq!(snapshot.universe.agents().len(), 24);
        let controls = app.world().resource::<Controls>();
        assert_eq!(controls.speed, 1);
        assert!(!controls.running);
        assert!(!controls.restart_pending);
        let market_text = app
            .world_mut()
            .query::<(&Readout, &Text)>()
            .iter(app.world())
            .find(|(readout, _)| matches!(readout, Readout::Market))
            .unwrap()
            .1;
        assert!(market_text.0.contains("12 goods"));

        let directory = std::env::temp_dir().join(format!(
            "learning-lord-export-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.world_mut()
            .resource_mut::<debug_export::DebugExport>()
            .directory = directory.clone();
        app.world_mut().resource_mut::<Controls>().error = Some("Simulation error".into());
        let expected = app.world().resource::<DisplaySnapshot>().0.universe.clone();
        let export = buttons
            .iter()
            .find(|(_, control)| matches!(control, Control::Export))
            .unwrap()
            .0;
        app.world_mut()
            .entity_mut(export)
            .insert(Interaction::Pressed);
        app.update();
        let files: Vec<_> = std::fs::read_dir(&directory).unwrap().collect();
        assert_eq!(files.len(), 1);
        let saved = std::fs::read_to_string(files[0].as_ref().unwrap().path()).unwrap();
        assert!(saved.contains(&format!("{expected:#?}")));
        assert_eq!(
            app.world().resource::<DisplaySnapshot>().0.universe,
            expected
        );
        assert_eq!(
            app.world().resource::<Controls>().error.as_deref(),
            Some("Simulation error")
        );
        let status = app
            .world_mut()
            .query::<(&Readout, &Text)>()
            .iter(app.world())
            .find(|(readout, _)| matches!(readout, Readout::ExportStatus))
            .unwrap()
            .1;
        assert!(status.0.starts_with("Exported "));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn clock_formats_elapsed_days_and_wraps_hours_minutes_and_seconds() {
        for (time_ms, expected) in [
            (0, "Day 0 | Monday | 00:00:00"),
            (999, "Day 0 | Monday | 00:00:00"),
            (1_000, "Day 0 | Monday | 00:00:01"),
            (60_000, "Day 0 | Monday | 00:01:00"),
            (86_399_999, "Day 0 | Monday | 23:59:59"),
            (86_400_000, "Day 1 | Tuesday | 00:00:00"),
            (
                100 * 86_400_000 + 3_723_000,
                "Day 100 | Wednesday | 01:02:03",
            ),
        ] {
            assert_eq!(format_clock(time_ms), expected);
        }
    }
}
