use crate::{
    DisplaySnapshot, MUTED, PANEL, TEXT, market,
    simulation::{Command, Mutation, SimulationWorker},
    text,
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use learning_lord_simulation::{
    Universe,
    marketplace::{ExportPolicy, Good},
};

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Reserve(Good),
    Import,
    Export,
}
#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    Toggle,
    Field(FieldKind),
    SaveGood,
    SaveTariffs,
    Reset,
}
#[derive(Clone, Copy)]
pub(crate) enum FieldKind {
    Reserve,
    Import,
    Export,
}
#[derive(Component)]
pub(crate) enum Readout {
    Toggle,
    Reserve,
    Import,
    Export,
    Status,
}
#[derive(Clone)]
struct Draft {
    allowed: bool,
    reserve: String,
}
#[derive(Clone, Copy)]
enum Saved {
    Good(Good),
    Tariffs,
}
#[derive(Resource, Default)]
pub struct State {
    drafts: Vec<Draft>,
    import: String,
    export: String,
    field: Option<Field>,
    pending: Option<(u64, Saved)>,
    generation: u64,
    error: Option<String>,
}
impl State {
    #[cfg(test)]
    pub fn focused_fixture() -> Self {
        let mut state = Self::default();
        state.load(&Universe::default(), 0);
        state.field = Some(Field::Import);
        state
    }

    pub fn editing(&self) -> bool {
        self.field.is_some()
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    fn load(&mut self, universe: &Universe, generation: u64) {
        let policy = universe.caravan_policy();
        *self = Self {
            drafts: policy
                .exports
                .iter()
                .map(|item| Draft {
                    allowed: item.allowed,
                    reserve: item.minimum_reserve.to_string(),
                })
                .collect(),
            import: percent(policy.import_tariff_basis_points),
            export: percent(policy.export_tariff_basis_points),
            generation,
            ..default()
        };
    }
    fn value_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Reserve(good) => &mut self.drafts[good as usize].reserve,
            Field::Import => &mut self.import,
            Field::Export => &mut self.export,
        }
    }
}
fn percent(points: u16) -> String {
    format!("{}.{:02}", points / 100, points % 100)
}
fn button(
    parent: &mut ChildSpawnerCommands,
    choice: Choice,
    label: &str,
    readout: Option<Readout>,
) {
    parent
        .spawn((
            Button,
            choice,
            BackgroundColor(PANEL),
            Node {
                padding: UiRect::axes(px(10), px(7)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
        ))
        .with_children(|button| {
            let mut item = button.spawn(text(label, 14.0, TEXT));
            if let Some(readout) = readout {
                item.insert(readout);
            }
        });
}
pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), flex_shrink: 0.0, ..default() }).with_children(|controls| {
        controls.spawn(text("CARAVAN POLICY", 14.0, MUTED));
        controls.spawn(Node { column_gap: px(8), row_gap: px(6), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|row| {
            button(row, Choice::Toggle, "", Some(Readout::Toggle));
            button(row, Choice::Field(FieldKind::Reserve), "", Some(Readout::Reserve));
            button(row, Choice::SaveGood, "Save good", None);
        });
        controls.spawn(Node { column_gap: px(8), row_gap: px(6), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|row| {
            button(row, Choice::Field(FieldKind::Import), "", Some(Readout::Import));
            button(row, Choice::Field(FieldKind::Export), "", Some(Readout::Export));
            button(row, Choice::SaveTariffs, "Save tariffs", None);
            button(row, Choice::Reset, "Reset drafts", None);
        });
        controls.spawn(text("Reserve uses whole grams or items. Town-wide tariffs: 0–100%, at most two decimal places.", 13.0, MUTED));
        controls.spawn((text("", 13.0, MUTED), Readout::Status));
    });
}
pub fn handle(
    mut state: ResMut<State>,
    (snapshot, worker): (Res<DisplaySnapshot>, Res<SimulationWorker>),
    selection: Res<market::Selection>,
    locations: Option<Res<crate::locations::State>>,
    controls: Option<Res<crate::Controls>>,
    buttons: Query<(&Interaction, &Choice), Changed<Interaction>>,
    mut keys: MessageReader<KeyboardInput>,
) {
    if state.drafts.is_empty() || state.generation != snapshot.0.generation {
        state.load(&snapshot.0.universe, snapshot.0.generation);
        keys.clear();
        return;
    }
    if controls.is_some_and(|controls| controls.restart_pending) || state.pending() {
        state.field = None;
        keys.clear();
        return;
    }
    if selection.view != market::View::Market {
        state.field = None;
        keys.clear();
        return;
    }
    let good = selection.good();
    if matches!(state.field, Some(Field::Reserve(previous)) if previous != good) {
        state.field = None;
    }
    for (interaction, choice) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if state.pending() {
            break;
        }
        state.error = None;
        if !matches!(choice, Choice::Field(_)) {
            state.field = None;
        }
        let mutation = match choice {
            Choice::Toggle => {
                state.drafts[good as usize].allowed ^= true;
                None
            }
            Choice::Field(kind) => {
                state.field = Some(match kind {
                    FieldKind::Reserve => Field::Reserve(good),
                    FieldKind::Import => Field::Import,
                    FieldKind::Export => Field::Export,
                });
                None
            }
            Choice::Reset => {
                state.load(&snapshot.0.universe, snapshot.0.generation);
                None
            }
            Choice::SaveGood => {
                let draft = &state.drafts[good as usize];
                match draft.reserve.parse::<u64>() {
                    Ok(minimum_reserve) => Some((
                        Mutation::SaveExportPolicy {
                            good,
                            policy: ExportPolicy {
                                allowed: draft.allowed,
                                minimum_reserve,
                            },
                        },
                        Saved::Good(good),
                    )),
                    Err(_) => {
                        state.error = Some("Reserve must be a whole non-negative quantity.".into());
                        None
                    }
                }
            }
            Choice::SaveTariffs => {
                match crate::locations::percentage(&state.import).and_then(|import| {
                    crate::locations::percentage(&state.export)
                        .map(|export| (import.basis_points(), export.basis_points()))
                }) {
                    Ok((import_basis_points, export_basis_points)) => Some((
                        Mutation::SaveTariffs {
                            import_basis_points,
                            export_basis_points,
                        },
                        Saved::Tariffs,
                    )),
                    Err(error) => {
                        state.error = Some(error);
                        None
                    }
                }
            }
        };
        if let Some((mutation, saved)) = mutation {
            if locations
                .as_ref()
                .is_some_and(|locations| locations.pending())
            {
                state.error = Some("Another change is saving. Try again when it finishes.".into());
                continue;
            }
            match worker.send(Command::Mutate {
                generation: snapshot.0.generation,
                mutation,
            }) {
                Ok(()) => state.pending = Some((snapshot.0.mutation_revision, saved)),
                Err(error) => state.error = Some(error),
            }
        }
    }
    for key in keys.read() {
        if key.state != ButtonState::Pressed || state.pending() {
            continue;
        }
        let Some(field) = state.field else {
            continue;
        };
        match &key.logical_key {
            Key::Escape | Key::Enter | Key::Tab => state.field = None,
            Key::Backspace => {
                state.value_mut(field).pop();
            }
            Key::Character(value)
                if value
                    .chars()
                    .all(|c| c.is_ascii_digit() || (field != Field::Reserve(good) && c == '.')) =>
            {
                let target = state.value_mut(field);
                if target.len() + value.len() <= 30 {
                    target.push_str(value);
                }
            }
            _ => {}
        }
    }
}
pub fn refresh(
    mut state: ResMut<State>,
    snapshot: Res<DisplaySnapshot>,
    selection: Res<market::Selection>,
    mut readouts: Query<(&Readout, &mut Text)>,
) {
    if state.drafts.is_empty() || state.generation != snapshot.0.generation {
        state.load(&snapshot.0.universe, snapshot.0.generation);
    }
    if let Some((revision, saved)) = state.pending
        && snapshot.0.mutation_revision > revision
    {
        state.pending = None;
        state.error = snapshot.0.mutation_error.clone();
        if state.error.is_none() {
            let policy = snapshot.0.universe.caravan_policy();
            match saved {
                Saved::Good(good) => {
                    let saved = policy.exports[good as usize];
                    state.drafts[good as usize] = Draft {
                        allowed: saved.allowed,
                        reserve: saved.minimum_reserve.to_string(),
                    };
                }
                Saved::Tariffs => {
                    state.import = percent(policy.import_tariff_basis_points);
                    state.export = percent(policy.export_tariff_basis_points);
                }
            }
        }
    }
    let good = selection.good();
    let draft = &state.drafts[good as usize];
    for (readout, mut label) in &mut readouts {
        let cursor = |field| if state.field == Some(field) { " |" } else { "" };
        let next = match readout {
            Readout::Toggle => format!(
                "Exports: {}",
                if draft.allowed { "enabled" } else { "disabled" }
            ),
            Readout::Reserve => format!(
                "Minimum reserve: {} {}{}",
                draft.reserve,
                if good.units_per_price_unit() == 1000 {
                    "g"
                } else {
                    "items"
                },
                cursor(Field::Reserve(good))
            ),
            Readout::Import => format!("Import tariff: {}%{}", state.import, cursor(Field::Import)),
            Readout::Export => format!("Export tariff: {}%{}", state.export, cursor(Field::Export)),
            Readout::Status => state.error.clone().unwrap_or_else(|| {
                if state.pending() {
                    "Saving…".into()
                } else {
                    "Changes apply when saved.".into()
                }
            }),
        };
        label.set_if_neq(Text(next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn app() -> App {
        let worker = SimulationWorker::spawn(Universe::default(), 60.try_into().unwrap());
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<State>()
            .init_resource::<market::Selection>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, (handle, refresh).chain());
        app.world_mut().resource_mut::<market::Selection>().view = market::View::Market;
        app.update();
        app
    }
    fn press(app: &mut App, choice: Choice) {
        let entity = app.world_mut().spawn((choice, Interaction::Pressed)).id();
        app.update();
        app.world_mut().despawn(entity);
    }
    fn await_save(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.world().resource::<State>().pending() {
            assert!(Instant::now() < deadline);
            let snapshot = app.world().resource::<SimulationWorker>().snapshot();
            app.world_mut().resource_mut::<DisplaySnapshot>().0 = snapshot;
            app.update();
            std::thread::yield_now();
        }
    }
    #[test]
    fn good_and_tariff_saves_preserve_other_unsaved_drafts() {
        let mut app = app();
        {
            let mut state = app.world_mut().resource_mut::<State>();
            state.drafts[Good::Berries as usize].allowed = false;
            state.drafts[Good::Berries as usize].reserve = "1234".into();
            state.drafts[Good::Bread as usize].reserve = "7".into();
            state.import = "50".into();
            state.export = "12.34".into();
        }
        press(&mut app, Choice::SaveGood);
        await_save(&mut app);
        let policy = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .caravan_policy();
        assert!(!policy.exports[Good::Berries as usize].allowed);
        assert_eq!(policy.exports[Good::Berries as usize].minimum_reserve, 1234);
        assert_eq!(policy.import_tariff_basis_points, 0);
        assert_eq!(app.world().resource::<State>().import, "50");
        assert_eq!(
            app.world().resource::<State>().drafts[Good::Bread as usize].reserve,
            "7"
        );
        press(&mut app, Choice::SaveTariffs);
        await_save(&mut app);
        let policy = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .caravan_policy();
        assert_eq!(policy.import_tariff_basis_points, 5000);
        assert_eq!(policy.export_tariff_basis_points, 1234);
        assert_eq!(policy.exports[Good::Berries as usize].minimum_reserve, 1234);
    }
    #[test]
    fn invalid_and_failed_saves_keep_drafts_and_restart_discards_stale_clicks() {
        let mut app = app();
        app.world_mut().resource_mut::<State>().drafts[0].reserve = "1.5".into();
        press(&mut app, Choice::SaveGood);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(app.world().resource::<State>().drafts[0].reserve, "1.5");
        app.world_mut().resource_mut::<State>().import = "100.01".into();
        press(&mut app, Choice::SaveTariffs);
        assert!(app.world().resource::<State>().error.is_some());
        assert_eq!(app.world().resource::<State>().import, "100.01");
        app.world_mut().resource_mut::<State>().pending = Some((0, Saved::Tariffs));
        {
            let mut snapshot = app.world_mut().resource_mut::<DisplaySnapshot>();
            snapshot.0.mutation_revision = 1;
            snapshot.0.mutation_error = Some("Worker rejected change".into());
        }
        app.update();
        assert_eq!(app.world().resource::<State>().import, "100.01");
        assert_eq!(
            app.world().resource::<State>().error.as_deref(),
            Some("Worker rejected change")
        );
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        press(&mut app, Choice::SaveTariffs);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(app.world().resource::<State>().import, "0.00");
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .mutation_revision,
            0
        );
    }
    #[test]
    fn another_editor_pending_keeps_drafts_without_submitting() {
        let mut app = app();
        app.insert_resource(crate::locations::State::pending_fixture());
        app.world_mut().resource_mut::<State>().import = "50".into();
        app.world_mut().resource_mut::<State>().drafts[0].reserve = "123".into();
        press(&mut app, Choice::SaveTariffs);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(app.world().resource::<State>().import, "50");
        assert!(
            app.world()
                .resource::<State>()
                .error
                .as_ref()
                .unwrap()
                .contains("Another change")
        );
        press(&mut app, Choice::SaveGood);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(app.world().resource::<State>().drafts[0].reserve, "123");
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .mutation_revision,
            0
        );
    }

    #[test]
    fn pending_guard_and_restart_request_prevent_dispatch() {
        let mut app = app();
        app.world_mut().resource_mut::<State>().pending = Some((0, Saved::Good(Good::Berries)));
        app.world_mut().resource_mut::<State>().import = "50".into();
        press(&mut app, Choice::SaveTariffs);
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .mutation_revision,
            0
        );
        app.world_mut().resource_mut::<State>().pending = None;
        app.insert_resource(crate::Controls {
            restart_pending: true,
            ..default()
        });
        press(&mut app, Choice::SaveTariffs);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(app.world().resource::<State>().import, "50");
        assert_eq!(
            app.world()
                .resource::<SimulationWorker>()
                .snapshot()
                .mutation_revision,
            0
        );
    }
}
