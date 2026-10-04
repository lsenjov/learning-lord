mod citizens;
mod debug_export;
mod map_view;
mod simulation;

use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use learning_lord_simulation::{
    AgentKind, CitizenAction, Universe, marketplace::Good, planning::COMMITMENT_MS,
};
use simulation::{Command as SimulationCommand, SPEEDS, SimulationWorker, Snapshot};

const BACKGROUND: Color = Color::srgb(0.06, 0.08, 0.10);
const PANEL: Color = Color::srgb(0.10, 0.13, 0.16);
const TEXT: Color = Color::srgb(0.92, 0.95, 0.94);
const MUTED: Color = Color::srgb(0.58, 0.66, 0.69);
const SELECTED: Color = Color::srgb(0.12, 0.38, 0.30);

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
    let worker = SimulationWorker::spawn(universe, rate);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Learning Lord".into(),
                name: Some("learning-lord".into()),
                resolution: (1280, 800).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(BACKGROUND))
        .insert_resource(DisplaySnapshot(worker.snapshot()))
        .insert_resource(worker)
        .init_resource::<Controls>()
        .init_resource::<citizens::Selection>()
        .init_resource::<citizens::PlanDisplay>()
        .init_resource::<debug_export::DebugExport>()
        .init_resource::<InputFocus>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                poll_worker,
                handle_controls,
                citizens::handle_selection,
                refresh_display,
                citizens::refresh_cards,
                citizens::refresh_plan,
                citizens::scroll_panels,
                map_view::refresh,
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

fn setup(mut commands: Commands) {
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
        citizens::spawn_roster(root);
        root.spawn(Node { width: percent(100), flex_grow: 1.0, min_height: px(0), column_gap: px(14), ..default() })
            .with_children(|row| {
                row.spawn((Node { width: px(324), flex_shrink: 0.0, flex_direction: FlexDirection::Column,
                    row_gap: px(12), min_height: px(0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default(), bevy::ui::RelativeCursorPosition::default()))
                    .with_children(|left| {
                        map_view::spawn(left);
                    });
                citizens::spawn_details(row);
            });
        root.spawn(text("Space: run / pause   |   Right: advance 30 min   |   1-4: select citizen   |   1x = 1 minute / second   |   Scroll panels for more", 13.0, MUTED));
        root.spawn((text("", 14.0, Color::srgb(1.0, 0.55, 0.48)), Readout::Error));
    });
}

fn poll_worker(
    worker: Res<SimulationWorker>,
    mut snapshot: ResMut<DisplaySnapshot>,
    mut controls: ResMut<Controls>,
) {
    let next = worker.snapshot();
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
        Control::Step => return,
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
    keyboard: Res<ButtonInput<KeyCode>>,
    buttons: Query<(Entity, &Interaction, &Control), Changed<Interaction>>,
    mut focus: ResMut<InputFocus>,
    mut controls: ResMut<Controls>,
    worker: Res<SimulationWorker>,
    snapshot: Res<DisplaySnapshot>,
    mut exporter: ResMut<debug_export::DebugExport>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        apply_control(Control::ToggleRunning, &mut controls, &worker);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        apply_control(Control::Step, &mut controls, &worker);
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
        "Day {} | {:02}:{:02}:{:02}",
        seconds / 86_400,
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
                CitizenAction::Eat => "Eating",
                CitizenAction::Wait => "Waiting",
                CitizenAction::Sleep => "Sleeping",
                CitizenAction::Forage => "Foraging",
                CitizenAction::FindRocks => "Finding rocks",
                CitizenAction::BuyBerries => "Buying berries",
                CitizenAction::SellPebbles => "Selling pebbles",
                CitizenAction::Travel(location) => match location {
                    learning_lord_simulation::locations::Location::House => "Walking to house",
                    learning_lord_simulation::locations::Location::Forest => "Walking to forest",
                    learning_lord_simulation::locations::Location::River => "Walking to river",
                    learning_lord_simulation::locations::Location::Market => "Walking to market",
                },
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
    let commitment = citizen
        .active_plan()
        .map(|active| {
            let left = COMMITMENT_MS.saturating_sub(active.elapsed_ms());
            if left == 0 {
                "Replan after current action".into()
            } else {
                format!(
                    "Commitment left: {} (then finish action)",
                    citizens::duration(left)
                )
            }
        })
        .unwrap_or_else(|| "No active plan".into());
    format!(
        "{} | Berries: {:.1} g | Pebbles: {:.1} g\nCoins: {:.2} | Wealth: {} coins\nHunger: {:.1}     Tiredness: {:.1}     Wellbeing: {wellbeing}\nLocation: {}\n{action}\n{commitment}",
        agent.name,
        citizen.berries_grams(),
        citizen.pebbles_grams(),
        citizen.coins(),
        citizen
            .wealth()
            .map_or_else(|error| error.to_string(), |wealth| format!("{wealth:.3}")),
        citizen.hunger(),
        citizen.tiredness(),
        citizens::location_label(citizen)
    )
}

fn action_label(action: CitizenAction) -> &'static str {
    match action {
        CitizenAction::Eat => "Eat",
        CitizenAction::Wait => "Wait",
        CitizenAction::Sleep => "Sleep",
        CitizenAction::Forage => "Forage",
        CitizenAction::FindRocks => "Find rocks",
        CitizenAction::BuyBerries => "Buy berries",
        CitizenAction::SellPebbles => "Sell pebbles",
        CitizenAction::Travel(location) => match location {
            learning_lord_simulation::locations::Location::House => "Travel home",
            learning_lord_simulation::locations::Location::Forest => "Travel to forest",
            learning_lord_simulation::locations::Location::River => "Travel to river",
            learning_lord_simulation::locations::Location::Market => "Travel to market",
        },
    }
}

fn sequence_readout(actions: &[CitizenAction]) -> String {
    let mut labels = Vec::new();
    let mut index = 0;
    while index < actions.len() {
        let action = actions[index];
        let count = actions[index..]
            .iter()
            .take_while(|&&next| next == action)
            .count();
        let label = action_label(action);
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
            "Prices used: berries {:.3}, pebbles {:.3} coins/kg",
            decision.prices.coins_per_kg(Good::Berries),
            decision.prices.coins_per_kg(Good::Pebbles)
        ),
    ];
    for candidate in &decision.candidates {
        let name = match candidate.goal {
            Effect::ReduceHunger => "Hunger",
            Effect::ReduceTiredness => "Sleep",
            Effect::IncreaseWealth => "Wealth",
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
            lines.push(sequence_readout(&forecast.actions));
        } else {
            lines.push(format!(
                "Unavailable: {}",
                candidate.unavailable_reason().unwrap()
            ));
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
            Readout::Market => {
                format!(
                    "Market: berries {:.3} | pebbles {:.3} coins/kg | updates at 04:00",
                    snapshot.0.universe.prices().coins_per_kg(Good::Berries),
                    snapshot.0.universe.prices().coins_per_kg(Good::Pebbles)
                )
            }
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
        color.0 = if disabled {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use learning_lord_simulation::Citizen;

    #[test]
    fn decision_panel_shows_reachable_hunger_and_uses_saved_prices() {
        use learning_lord_simulation::marketplace::Prices;
        let prices = Prices::new(2.0, 1.0).unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(prices)
                .with_citizen("Ada", Citizen::new(0.0).unwrap());
        assert!(decision_readout(&universe, None).contains("No planning decision yet"));
        let planned = universe.start_planning(id).unwrap();
        let text = decision_readout(&planned, None);
        assert!(text.contains("Hunger"));
        assert!(!text.contains("Unavailable:"));
        assert!(text.contains("Sleep"));
        assert!(text.contains("Wealth"));
        assert_eq!(text.matches("[chosen first]").count(), 1);
        assert!(text.contains("Goal avg"));
        assert!(text.contains("Plan avg"));
        assert!(text.contains("berries 2.000, pebbles 1.000"));
        assert_eq!(
            decision_readout(
                &planned
                    .with_prices(Prices::new(1.0, 4.0).unwrap())
                    .advance(1)
                    .unwrap(),
                None
            ),
            text
        );
        assert_eq!(
            sequence_readout(&[
                CitizenAction::Forage,
                CitizenAction::Forage,
                CitizenAction::Eat
            ]),
            "Forage x2 > Eat"
        );
    }

    #[test]
    fn readout_shows_berries_upcoming_actions_and_commitment() {
        let citizen = Citizen::with_needs(60.0, 100.0)
            .unwrap()
            .with_berries(200.0)
            .unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(learning_lord_simulation::marketplace::Prices::default())
                .with_citizen("Ada", citizen);
        let universe = universe
            .start_planning(id)
            .unwrap()
            .advance(50_000)
            .unwrap();
        let readout = citizen_readout(&universe, None);
        assert!(readout.contains("Eating | 00:00:50 remaining"));
        assert!(readout.contains("Location:"));
        assert!(readout.contains("Commitment left: 01:59:10"));
        assert!(readout.contains("Berries: 150.0 g"));
        assert!(readout.contains("Pebbles: 0.0 g"));
        assert!(readout.contains("Coins: 0.00 | Wealth: 0.150 coins"));
    }

    #[test]
    fn sleeping_readout_counts_down_to_completion_even_past_commitment() {
        let citizen = Citizen::with_needs(-50.0, 50.0).unwrap();
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_prices(learning_lord_simulation::marketplace::Prices::default())
                .with_citizen("Ada", citizen);
        let universe = universe
            .start_planning(id)
            .unwrap()
            .advance(COMMITMENT_MS)
            .unwrap();
        let readout = citizen_readout(&universe, None);
        assert!(readout.contains("Sleeping | 06:00:00 remaining"));
        assert!(readout.contains("Location: House"));
        assert!(readout.contains("Replan after current action"));
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
            .init_resource::<citizens::PlanDisplay>()
            .init_resource::<debug_export::DebugExport>()
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    poll_worker,
                    handle_controls,
                    citizens::handle_selection,
                    refresh_display,
                    citizens::refresh_cards,
                    citizens::refresh_plan,
                    citizens::scroll_panels,
                    map_view::refresh,
                )
                    .chain(),
            );
        app.update();

        let buttons: Vec<_> = app
            .world_mut()
            .query::<(Entity, &Control)>()
            .iter(app.world())
            .map(|(entity, control)| (entity, *control))
            .collect();
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
        assert_eq!(clock.0, "Day 0 | 00:30:00");

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
        assert_eq!(snapshot.universe.current_time_ms(), 0);
        assert_ne!(snapshot.universe.prices(), previous_prices);
        assert_eq!(snapshot.universe.agents().len(), 4);
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
        assert!(market_text.0.contains("coins/kg"));

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
            (0, "Day 0 | 00:00:00"),
            (999, "Day 0 | 00:00:00"),
            (1_000, "Day 0 | 00:00:01"),
            (60_000, "Day 0 | 00:01:00"),
            (86_399_999, "Day 0 | 23:59:59"),
            (86_400_000, "Day 1 | 00:00:00"),
            (100 * 86_400_000 + 3_723_000, "Day 100 | 01:02:03"),
        ] {
            assert_eq!(format_clock(time_ms), expected);
        }
    }
}
