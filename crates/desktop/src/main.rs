use bevy::prelude::*;
use learning_lord_simulation::Universe;

#[derive(Default, Resource)]
struct Simulation(Universe);

fn main() {
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
        .insert_resource(ClearColor(Color::srgb(0.08, 0.10, 0.12)))
        .init_resource::<Simulation>()
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, simulation: Res<Simulation>) {
    commands.spawn(Camera2d);
    commands.spawn((
        Text2d::new(format!(
            "Learning Lord\n{} agents",
            simulation.0.agents().len()
        )),
        TextFont {
            font_size: FontSize::Px(32.0),
            ..default()
        },
        TextLayout::justify(Justify::Center),
    ));
}
