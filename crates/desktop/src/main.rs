mod simulation;

use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use learning_lord_simulation::{
    AgentKind, Citizen, CitizenAction, Universe, planning::COMMITMENT_MS,
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
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            running: false,
            speed: 1,
            error: None,
        }
    }
}

#[derive(Resource)]
struct DisplaySnapshot(Snapshot);

#[derive(Component, Clone, Copy)]
enum Control {
    ToggleRunning,
    Step,
    Speed(u32),
}

#[derive(Component)]
enum Readout {
    Clock,
    Status,
    Citizen,
    Error,
    RunButton,
}

fn main() -> Result<(), String> {
    let rate = simulation::update_rate()?;
    let citizen = Citizen::new(0.0).map_err(|error| error.to_string())?;
    let (universe, id) = Universe::default().with_citizen("Ada", citizen);
    let universe = universe
        .start_planning(id)
        .map_err(|error| error.to_string())?;
    let worker = SimulationWorker::spawn(universe, rate);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Learning Lord".into(),
                name: Some("learning-lord".into()),
                resolution: (1024, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(BACKGROUND))
        .insert_resource(DisplaySnapshot(worker.snapshot()))
        .insert_resource(worker)
        .init_resource::<Controls>()
        .init_resource::<InputFocus>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (poll_worker, handle_controls, refresh_display).chain(),
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
            height: px(48),
            padding: UiRect::horizontal(px(18)),
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
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::all(px(40)),
            flex_direction: FlexDirection::Column,
            row_gap: px(18),
            ..default()
        })
        .with_children(|root| {
            root.spawn(text("LEARNING LORD", 18.0, MUTED));
            root.spawn((text("Day 0 | 00:00:00", 44.0, TEXT), Readout::Clock));
            root.spawn((text("Paused | 1x", 20.0, MUTED), Readout::Status));
            root.spawn(Node {
                column_gap: px(12),
                row_gap: px(12),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                row.spawn(button(Control::ToggleRunning))
                    .with_children(|button| {
                        button.spawn((text("Run", 20.0, TEXT), Readout::RunButton));
                    });
                row.spawn(button(Control::Step)).with_children(|button| {
                    button.spawn(text("Advance 30 minutes", 20.0, TEXT));
                });
            });
            root.spawn(text("Simulation speed", 16.0, MUTED));
            root.spawn(Node {
                column_gap: px(10),
                row_gap: px(10),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                for speed in SPEEDS {
                    row.spawn(button(Control::Speed(speed)))
                        .with_children(|button| {
                            button.spawn(text(format!("{speed}x"), 20.0, TEXT));
                        });
                }
            });
            root.spawn(text(
                "1x = 1 simulation minute per real second",
                16.0,
                MUTED,
            ));
            root.spawn((
                Node {
                    padding: UiRect::all(px(24)),
                    margin: UiRect::top(px(8)),
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|panel| {
                panel.spawn((text("", 22.0, TEXT), Readout::Citizen));
            });
            root.spawn(text(
                "Space: run / pause     Right arrow: advance 30 minutes while paused",
                14.0,
                MUTED,
            ));
            root.spawn((text("", 16.0, Color::srgb(1.0, 0.55, 0.48)), Readout::Error));
        });
}

fn poll_worker(
    worker: Res<SimulationWorker>,
    mut snapshot: ResMut<DisplaySnapshot>,
    mut controls: ResMut<Controls>,
) {
    snapshot.0 = worker.snapshot();
    if let Some(error) = &snapshot.0.error {
        controls.running = false;
        controls.error = Some(error.clone());
    }
}

fn apply_control(control: Control, controls: &mut Controls, worker: &SimulationWorker) {
    if controls.error.is_some() {
        return;
    }
    let command = match control {
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
    }
}

fn handle_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    buttons: Query<(Entity, &Interaction, &Control), Changed<Interaction>>,
    mut focus: ResMut<InputFocus>,
    mut controls: ResMut<Controls>,
    worker: Res<SimulationWorker>,
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
            apply_control(*control, &mut controls, &worker);
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

fn citizen_readout(universe: &Universe) -> String {
    let Some(agent) = universe.agents().values().next() else {
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
            };
            let seconds = active.remaining_ms().div_ceil(1000);
            format!("{name} | {:02}:{:02} remaining", seconds / 60, seconds % 60)
        })
        .unwrap_or_else(|| "Idle".into());
    let plan = citizen
        .active_plan()
        .map(|active| {
            let upcoming = active
                .plan()
                .actions()
                .iter()
                .skip(active.action_index() + 1)
                .map(|action| match action {
                    CitizenAction::Eat => "Eat",
                    CitizenAction::Wait => "Wait",
                })
                .collect::<Vec<_>>()
                .join(" > ");
            let upcoming = if upcoming.is_empty() {
                "None"
            } else {
                &upcoming
            };
            let seconds = COMMITMENT_MS
                .saturating_sub(active.elapsed_ms())
                .div_ceil(1000);
            let replan = if seconds == 0 {
                "Replan after current action".into()
            } else {
                format!(
                    "Replan in {:02}:{:02}:{:02} (after action completes)",
                    seconds / 3600,
                    seconds / 60 % 60,
                    seconds % 60
                )
            };
            format!("Planned next: {upcoming}\n{replan}")
        })
        .unwrap_or_else(|| "No active plan".into());
    format!(
        "{}\nHunger: {:.1}     Wellbeing: {wellbeing}\n{action}\n{plan}",
        agent.name,
        citizen.hunger()
    )
}

fn refresh_display(
    controls: Res<Controls>,
    snapshot: Res<DisplaySnapshot>,
    mut readouts: Query<(&Readout, &mut Text)>,
    mut buttons: Query<(&Control, &Interaction, &mut BackgroundColor)>,
) {
    for (readout, mut text) in &mut readouts {
        let value = match readout {
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
            Readout::Citizen => citizen_readout(&snapshot.0.universe),
            Readout::Error => controls.error.clone().unwrap_or_default(),
            Readout::RunButton => if controls.running { "Pause" } else { "Run" }.into(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (control, interaction, mut color) in &mut buttons {
        let disabled =
            controls.error.is_some() || matches!(control, Control::Step) && controls.running;
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

    #[test]
    fn readout_shows_only_upcoming_actions_and_remaining_commitment() {
        let citizen = Citizen::with_hunger_rate(-500.0, 0.0).unwrap();
        let (universe, id) = Universe::default().with_citizen("Ada", citizen);
        let universe = universe
            .start_planning(id)
            .unwrap()
            .advance(30 * 60 * 1000)
            .unwrap();
        let readout = citizen_readout(&universe);
        assert!(readout.contains("Waiting | 30:00 remaining"));
        assert!(readout.contains("Planned next: Wait > Wait > Wait > Wait > Wait > Wait\n"));
        assert!(readout.contains("Replan in 01:30:00 (after action completes)"));
    }

    #[test]
    fn ui_buttons_select_speeds_step_and_toggle_running() {
        use std::time::{Duration, Instant};

        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<Controls>()
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (poll_worker, handle_controls, refresh_display).chain(),
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
