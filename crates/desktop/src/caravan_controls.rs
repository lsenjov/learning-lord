use crate::{
    DisplaySnapshot, MUTED, PANEL, TEXT,
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
    GoodImport(Good),
    GoodExport(Good),
    Import,
    Export,
}
#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    Toggle,
    ToggleImports,
    Field(FieldKind),
    SaveGood,
    SaveTariffs,
    Reset,
}
#[derive(Clone, Copy)]
pub(crate) enum FieldKind {
    Reserve,
    GoodImport,
    GoodExport,
    Import,
    Export,
}
#[derive(Component)]
pub(crate) enum Readout {
    Toggle,
    ToggleImports,
    GoodImport,
    GoodExport,
    Reserve,
    Import,
    Export,
    Status,
}
#[derive(Clone)]
struct Draft {
    allowed: bool,
    imports_allowed: bool,
    import: String,
    export: String,
    reserve: String,
}
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    replace_on_input: bool,
    pending: Option<(u64, Saved)>,
    generation: u64,
    errors: std::collections::HashMap<Saved, String>,
    field_owner: Option<Entity>,
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
                .copied()
                .map(Draft::from_policy)
                .collect(),
            import: percent(policy.import_tariff_basis_points),
            export: percent(policy.export_tariff_basis_points),
            generation,
            ..default()
        };
    }
    fn value_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::GoodImport(good) => &mut self.drafts[good as usize].import,
            Field::GoodExport(good) => &mut self.drafts[good as usize].export,
            Field::Reserve(good) => &mut self.drafts[good as usize].reserve,
            Field::Import => &mut self.import,
            Field::Export => &mut self.export,
        }
    }
}
fn percent(points: i32) -> String {
    let magnitude = i64::from(points).abs();
    format!(
        "{}{}.{:02}",
        if points < 0 { "-" } else { "" },
        magnitude / 100,
        magnitude % 100
    )
}
fn tariff(value: &str) -> Result<i32, String> {
    let error =
        || "Tariff must be a signed percentage with at most two decimal places.".to_string();
    let (negative, digits) = value
        .strip_prefix('-')
        .map_or((false, value), |s| (true, s));
    let mut parts = digits.split('.');
    let whole = parts.next().ok_or_else(error)?;
    let fraction = parts.next().unwrap_or("");
    if whole.is_empty()
        || !whole.chars().all(|c| c.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.chars().all(|c| c.is_ascii_digit())
        || parts.next().is_some()
    {
        return Err(error());
    }
    let points = whole
        .parse::<i64>()
        .ok()
        .and_then(|n| n.checked_mul(100))
        .and_then(|n| {
            n.checked_add(
                fraction.parse::<i64>().unwrap_or(0) * if fraction.len() == 1 { 10 } else { 1 },
            )
        })
        .ok_or_else(error)?;
    i32::try_from(if negative { -points } else { points }).map_err(|_| error())
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
pub(crate) fn spawn_global(parent: &mut ChildSpawnerCommands) {
    spawn_editor(parent, Saved::Tariffs);
}

pub(crate) fn spawn_good(parent: &mut ChildSpawnerCommands, good: Good) {
    spawn_editor(parent, Saved::Good(good));
}

fn spawn_editor(parent: &mut ChildSpawnerCommands, scope: Saved) {
    parent
        .spawn((
            scope,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|controls| {
            controls.spawn(text("CARAVAN POLICY", 14.0, MUTED));
            controls
                .spawn(Node {
                    column_gap: px(8),
                    row_gap: px(6),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    match scope {
                        Saved::Good(_) => {
                            button(row, Choice::Toggle, "", Some(Readout::Toggle));
                            button(row, Choice::ToggleImports, "", Some(Readout::ToggleImports));
                            for (field, readout) in [
                                (FieldKind::Reserve, Readout::Reserve),
                                (FieldKind::GoodImport, Readout::GoodImport),
                                (FieldKind::GoodExport, Readout::GoodExport),
                            ] {
                                button(row, Choice::Field(field), "", Some(readout));
                            }
                            button(row, Choice::SaveGood, "Save good", None);
                        }
                        Saved::Tariffs => {
                            button(
                                row,
                                Choice::Field(FieldKind::Import),
                                "",
                                Some(Readout::Import),
                            );
                            button(
                                row,
                                Choice::Field(FieldKind::Export),
                                "",
                                Some(Readout::Export),
                            );
                            button(row, Choice::SaveTariffs, "Save tariffs", None);
                        }
                    }
                    button(row, Choice::Reset, "Reset drafts", None);
                });
            controls.spawn((text("", 13.0, MUTED), Readout::Status));
        });
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Editors<'w, 's> {
    parents: Query<'w, 's, &'static ChildOf>,
    roots: Query<'w, 's, (&'static Saved, Option<&'static InheritedVisibility>)>,
}

impl Editors<'_, '_> {
    fn owner(&self, entity: Entity) -> Option<(Entity, Saved)> {
        self.parents
            .iter_ancestors(entity)
            .find_map(|id| self.roots.get(id).ok().map(|(scope, _)| (id, *scope)))
    }
}

impl Draft {
    fn from_policy(policy: ExportPolicy) -> Self {
        Self {
            allowed: policy.allowed,
            imports_allowed: policy.imports_allowed,
            import: percent(policy.import_tariff_basis_points),
            export: percent(policy.export_tariff_basis_points),
            reserve: policy.minimum_reserve.to_string(),
        }
    }
}
impl State {
    fn reload_scope(&mut self, universe: &Universe, scope: Saved) {
        let policy = universe.caravan_policy();
        match scope {
            Saved::Good(good) => {
                self.drafts[good as usize] = Draft::from_policy(policy.exports[good as usize])
            }
            Saved::Tariffs => {
                self.import = percent(policy.import_tariff_basis_points);
                self.export = percent(policy.export_tariff_basis_points);
            }
        }
    }
}

pub fn handle(
    mut state: ResMut<State>,
    (snapshot, worker): (Res<DisplaySnapshot>, Res<SimulationWorker>),
    editors: Editors,
    (locations, construction): (
        Option<Res<crate::locations::State>>,
        Option<Res<crate::construction::State>>,
    ),
    controls: Option<Res<crate::Controls>>,
    dev: Option<Res<crate::dev_controls::State>>,
    buttons: Query<(Entity, &Interaction, &Choice), Changed<Interaction>>,
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
    if let Some(owner) = state.field_owner
        && !editors
            .roots
            .get(owner)
            .is_ok_and(|(_, visibility)| visibility.is_none_or(|v| v.get()))
    {
        stop_editing(&mut state);
    }
    for (entity, interaction, choice) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some((owner, scope)) = editors.owner(entity) else {
            continue;
        };
        if state.pending() {
            break;
        }
        if matches!(scope, Saved::Good(good) if !good.caravan_eligible())
            && !matches!(choice, Choice::Reset)
        {
            continue;
        }
        state.errors.remove(&scope);
        if !matches!(choice, Choice::Field(_)) {
            stop_editing(&mut state);
        }
        let mutation = match (scope, choice) {
            (Saved::Good(good), Choice::ToggleImports) => {
                state.drafts[good as usize].imports_allowed ^= true;
                None
            }
            (Saved::Good(good), Choice::Toggle) => {
                state.drafts[good as usize].allowed ^= true;
                None
            }
            (_, Choice::Field(kind)) => {
                state.field_owner = Some(owner);
                let field = match (scope, kind) {
                    (Saved::Good(good), FieldKind::GoodImport) => Field::GoodImport(good),
                    (Saved::Good(good), FieldKind::GoodExport) => Field::GoodExport(good),
                    (Saved::Good(good), FieldKind::Reserve) => Field::Reserve(good),
                    (Saved::Tariffs, FieldKind::Import) => Field::Import,
                    (Saved::Tariffs, FieldKind::Export) => Field::Export,
                    _ => continue,
                };
                if state.field != Some(field) {
                    state.replace_on_input = true;
                    state.field = Some(field);
                }
                None
            }
            (_, Choice::Reset) => {
                state.reload_scope(&snapshot.0.universe, scope);
                None
            }
            (Saved::Good(good), Choice::SaveGood) => {
                let draft = &state.drafts[good as usize];
                match draft
                    .reserve
                    .parse::<u64>()
                    .map_err(|_| "Reserve must be a whole non-negative quantity.".to_string())
                    .and_then(|reserve| {
                        Ok((reserve, tariff(&draft.import)?, tariff(&draft.export)?))
                    }) {
                    Ok((
                        minimum_reserve,
                        import_tariff_basis_points,
                        export_tariff_basis_points,
                    )) => Some((
                        Mutation::SaveExportPolicy {
                            good,
                            policy: ExportPolicy {
                                allowed: draft.allowed,
                                imports_allowed: draft.imports_allowed,
                                import_tariff_basis_points,
                                export_tariff_basis_points,
                                minimum_reserve,
                            },
                        },
                        Saved::Good(good),
                    )),
                    Err(error) => {
                        state.errors.insert(scope, error);
                        None
                    }
                }
            }
            (Saved::Tariffs, Choice::SaveTariffs) => {
                match tariff(&state.import)
                    .and_then(|import| tariff(&state.export).map(|export| (import, export)))
                {
                    Ok((import_basis_points, export_basis_points)) => Some((
                        Mutation::SaveTariffs {
                            import_basis_points,
                            export_basis_points,
                        },
                        Saved::Tariffs,
                    )),
                    Err(error) => {
                        state.errors.insert(scope, error);
                        None
                    }
                }
            }
            _ => continue,
        };
        if let Some((mutation, saved)) = mutation {
            if dev.as_ref().is_some_and(|editor| editor.pending())
                || locations
                    .as_ref()
                    .is_some_and(|locations| locations.pending())
                || construction.as_ref().is_some_and(|editor| editor.pending())
            {
                state.errors.insert(
                    scope,
                    "Another change is saving. Try again when it finishes.".into(),
                );
                continue;
            }
            match worker.send(Command::Mutate {
                generation: snapshot.0.generation,
                mutation,
            }) {
                Ok(()) => state.pending = Some((snapshot.0.mutation_revision, saved)),
                Err(error) => {
                    state.errors.insert(scope, error);
                }
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
            Key::Escape | Key::Enter | Key::Tab => stop_editing(&mut state),
            Key::Backspace => {
                if state.replace_on_input {
                    state.value_mut(field).clear();
                    state.replace_on_input = false;
                } else {
                    state.value_mut(field).pop();
                }
            }
            Key::Character(value)
                if value.chars().all(|c| {
                    c.is_ascii_digit()
                        || (!matches!(field, Field::Reserve(_)) && matches!(c, '.' | '-'))
                }) =>
            {
                let replace = state.replace_on_input;
                let target = state.value_mut(field);
                if !value.is_empty() && (if replace { 0 } else { target.len() }) + value.len() <= 30
                {
                    if replace {
                        target.clear();
                    }
                    target.push_str(value);
                    state.replace_on_input = false;
                }
            }
            _ => {}
        }
    }
}
pub fn refresh(
    mut state: ResMut<State>,
    snapshot: Res<DisplaySnapshot>,
    editors: Editors,
    mut readouts: Query<(Entity, &Readout, &mut Text)>,
    mut buttons: Query<(Entity, &Choice, &mut BackgroundColor)>,
) {
    if state.drafts.is_empty() || state.generation != snapshot.0.generation {
        state.load(&snapshot.0.universe, snapshot.0.generation);
    }
    if let Some((revision, saved)) = state.pending
        && snapshot.0.mutation_revision > revision
    {
        state.pending = None;
        if let Some(error) = &snapshot.0.mutation_error {
            state.errors.insert(saved, error.clone());
        } else {
            state.reload_scope(&snapshot.0.universe, saved);
        }
    }
    for (entity, choice, mut color) in &mut buttons {
        let disabled = matches!(editors.owner(entity),Some((_,Saved::Good(good))) if !good.caravan_eligible())
            && !matches!(choice, Choice::Reset);
        color.set_if_neq(BackgroundColor(if disabled {
            Color::srgb(0.04, 0.04, 0.04)
        } else {
            PANEL
        }));
    }
    for (entity, readout, mut label) in &mut readouts {
        let Some((_, scope)) = editors.owner(entity) else {
            continue;
        };
        let cursor = |field| if state.field == Some(field) { " |" } else { "" };
        let next = match (scope, readout) {
            (_, Readout::Status) => state.errors.get(&scope).cloned().unwrap_or_else(|| {
                if state.pending.is_some_and(|(_, saved)| saved == scope) {
                    "Saving…".into()
                } else {
                    String::new()
                }
            }),
            (Saved::Tariffs, Readout::Import) => {
                format!("Global import: {}%{}", state.import, cursor(Field::Import))
            }
            (Saved::Tariffs, Readout::Export) => {
                format!("Global export: {}%{}", state.export, cursor(Field::Export))
            }
            (Saved::Good(good), _) => {
                let draft = &state.drafts[good as usize];
                match readout {
                    Readout::Toggle | Readout::ToggleImports if !good.caravan_eligible() => {
                        "Caravan trade unavailable".into()
                    }
                    Readout::Reserve if !good.caravan_eligible() => {
                        "Minimum reserve: unavailable".into()
                    }
                    Readout::ToggleImports => format!(
                        "Imports: {}",
                        if draft.imports_allowed {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    ),
                    Readout::GoodImport => format!(
                        "Good import: {}%{}",
                        draft.import,
                        cursor(Field::GoodImport(good))
                    ),
                    Readout::GoodExport => format!(
                        "Good export: {}%{}",
                        draft.export,
                        cursor(Field::GoodExport(good))
                    ),
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
                    _ => continue,
                }
            }

            _ => continue,
        };
        label.set_if_neq(Text(next));
    }
}

pub(crate) fn focus_pane(state: &mut State, kind: crate::floating_ui::WindowKind) {
    use crate::floating_ui::WindowKind;
    let Some(field) = state.field else { return };
    let owner = match field {
        Field::GoodImport(good) | Field::GoodExport(good) | Field::Reserve(good) => {
            WindowKind::Good(good)
        }
        Field::Import | Field::Export => WindowKind::Market,
    };
    if owner != kind {
        stop_editing(state);
    }
}

pub(crate) fn stop_editing(state: &mut State) {
    state.field_owner = None;
    state.field = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market;
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
        app.world_mut().resource_mut::<market::Selection>().tab = market::Tab::CaravanPolicy;
        app.world_mut().resource_mut::<market::Selection>().good = Some(Good::Flour);
        app.update();
        app
    }
    fn press(app: &mut App, choice: Choice) {
        let good = app.world().resource::<market::Selection>().good();
        let scope = match choice {
            Choice::Field(FieldKind::Import | FieldKind::Export)
            | Choice::SaveTariffs
            | Choice::Reset => Saved::Tariffs,
            _ => Saved::Good(good),
        };
        let owner = app.world_mut().spawn(scope).id();
        let entity = app
            .world_mut()
            .spawn((ChildOf(owner), choice, Interaction::Pressed))
            .id();
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
    fn type_key(app: &mut App, logical_key: Key) {
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                key_code: KeyCode::KeyH,
                logical_key,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        app.update();
    }

    fn type_text(app: &mut App, value: &str) {
        type_key(app, Key::Character(value.into()));
    }

    #[test]
    fn numeric_fields_replace_on_first_input_only() {
        let mut app = app();
        for (kind, field, original) in [
            (FieldKind::Import, Field::Import, "0.00"),
            (FieldKind::Export, Field::Export, "20.00"),
            (
                FieldKind::GoodImport,
                Field::GoodImport(Good::Flour),
                "20.00",
            ),
            (
                FieldKind::GoodExport,
                Field::GoodExport(Good::Flour),
                "0.00",
            ),
            (FieldKind::Reserve, Field::Reserve(Good::Flour), "100"),
        ] {
            *app.world_mut().resource_mut::<State>().value_mut(field) = original.into();
            press(&mut app, Choice::Field(kind));
            assert_eq!(
                app.world_mut().resource_mut::<State>().value_mut(field),
                original
            );
            type_key(&mut app, Key::Tab);
            assert_eq!(
                app.world_mut().resource_mut::<State>().value_mut(field),
                original
            );
            press(&mut app, Choice::Field(kind));
            type_text(&mut app, "x");
            type_text(&mut app, "");
            assert_eq!(
                app.world_mut().resource_mut::<State>().value_mut(field),
                original
            );
            type_text(&mut app, "2");
            press(&mut app, Choice::Field(kind));
            type_text(&mut app, "0");
            assert_eq!(
                app.world_mut().resource_mut::<State>().value_mut(field),
                "20"
            );
            type_key(&mut app, Key::Backspace);
            assert_eq!(
                app.world_mut().resource_mut::<State>().value_mut(field),
                "2"
            );
            type_key(&mut app, Key::Enter);
            press(&mut app, Choice::Field(kind));
            type_key(&mut app, Key::Backspace);
            assert_eq!(app.world_mut().resource_mut::<State>().value_mut(field), "");
            type_text(&mut app, "20");
            type_key(&mut app, Key::Backspace);
            assert_eq!(
                app.world_mut().resource_mut::<State>().value_mut(field),
                "2"
            );
            type_key(&mut app, Key::Escape);
        }
        press(&mut app, Choice::Field(FieldKind::Import));
        type_text(&mut app, "-");
        type_text(&mut app, "2");
        type_text(&mut app, ".");
        type_text(&mut app, "5");
        assert_eq!(app.world().resource::<State>().import, "-2.5");
        press(&mut app, Choice::Field(FieldKind::Reserve));
        type_text(&mut app, "-");
        type_text(&mut app, ".");
        type_text(&mut app, "7");
        assert_eq!(
            app.world().resource::<State>().drafts[Good::Flour as usize].reserve,
            "7"
        );
        press(&mut app, Choice::Field(FieldKind::Import));
        type_text(&mut app, "20");
        press(&mut app, Choice::SaveTariffs);
        await_save(&mut app);
        assert_eq!(
            app.world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .caravan_policy()
                .import_tariff_basis_points,
            2000
        );
        press(&mut app, Choice::Field(FieldKind::Import));
        type_text(&mut app, "4");
        assert_eq!(app.world().resource::<State>().import, "4");
        press(&mut app, Choice::Reset);
        assert_eq!(app.world().resource::<State>().import, "20.00");
        press(&mut app, Choice::Field(FieldKind::Import));
        type_text(&mut app, "3");
        assert_eq!(app.world().resource::<State>().import, "3");
    }

    #[test]
    fn two_visible_policy_forms_edit_and_reset_only_their_own_good() {
        let mut app = app();
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|p| {
                spawn_good(p, Good::Wheat);
                spawn_good(p, Good::Flour);
            });
        let world = app.world_mut();
        world.flush();
        let forms: Vec<_> = world
            .query::<(Entity, &Saved)>()
            .iter(world)
            .map(|(e, s)| (e, *s))
            .collect();
        let button = |world: &mut World, good, reset| {
            let owner = forms
                .iter()
                .find(|(_, s)| *s == Saved::Good(good))
                .unwrap()
                .0;
            let candidates: Vec<_> = world
                .query::<(Entity, &Choice)>()
                .iter(world)
                .filter_map(|(id, c)| {
                    (if reset {
                        matches!(c, Choice::Reset)
                    } else {
                        matches!(c, Choice::Field(FieldKind::Reserve))
                    })
                    .then_some(id)
                })
                .collect();
            candidates
                .into_iter()
                .find(|&id| {
                    let mut parent = id;
                    while let Some(p) = world.get::<ChildOf>(parent) {
                        parent = p.parent();
                        if parent == owner {
                            return true;
                        }
                    }
                    false
                })
                .unwrap()
        };
        for (good, value) in [(Good::Wheat, "77"), (Good::Flour, "88")] {
            let field = button(app.world_mut(), good, false);
            let owner = forms
                .iter()
                .find(|(_, s)| *s == Saved::Good(good))
                .unwrap()
                .0;
            app.world_mut()
                .entity_mut(owner)
                .insert(InheritedVisibility::VISIBLE);
            app.world_mut()
                .entity_mut(field)
                .insert(Interaction::Pressed);
            app.update();
            type_text(&mut app, value);
        }
        let reset = button(app.world_mut(), Good::Flour, true);
        app.world_mut()
            .entity_mut(reset)
            .insert(Interaction::Pressed);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.drafts[Good::Wheat as usize].reserve, "77");
        assert_ne!(state.drafts[Good::Flour as usize].reserve, "88");
    }

    #[test]
    fn signed_percentages_accept_offsets_above_one_hundred() {
        for (input, expected) in [("-25", -2500), ("125.01", 12501), ("-0.01", -1), ("0", 0)] {
            assert_eq!(tariff(input), Ok(expected));
            assert_eq!(tariff(&percent(expected)), Ok(expected));
        }
        for input in ["--1", "1.234", "1-", "", "21474836.48", "1.2.3"] {
            assert!(tariff(input).is_err(), "{input}");
        }
        assert_eq!(tariff("-21474836.48"), Ok(i32::MIN));
    }

    #[test]
    fn switching_goods_preserves_each_reserve_and_clears_focus() {
        let mut app = app();
        press(&mut app, Choice::Field(FieldKind::Reserve));
        app.world_mut().resource_mut::<State>().drafts[Good::Flour as usize].reserve = "123".into();
        app.world_mut().resource_mut::<market::Selection>().good = Some(Good::BreadRoll);
        app.update();
        focus_pane(
            &mut app.world_mut().resource_mut::<State>(),
            crate::floating_ui::WindowKind::Good(Good::BreadRoll),
        );
        assert!(!app.world().resource::<State>().editing());
        app.world_mut().resource_mut::<State>().drafts[Good::BreadRoll as usize].reserve =
            "7".into();
        app.world_mut().resource_mut::<market::Selection>().good = Some(Good::Flour);
        app.update();
        let state = app.world().resource::<State>();
        assert_eq!(state.drafts[Good::Flour as usize].reserve, "123");
        assert_eq!(state.drafts[Good::BreadRoll as usize].reserve, "7");
    }

    #[test]
    fn unavailable_good_controls_ignore_clicks_and_keep_tariffs_editable() {
        let mut app = app();
        app.world_mut().resource_mut::<market::Selection>().good = Some(Good::Berries);
        press(&mut app, Choice::Toggle);
        press(&mut app, Choice::Field(FieldKind::Reserve));
        press(&mut app, Choice::SaveGood);
        assert!(!app.world().resource::<State>().drafts[Good::Berries as usize].allowed);
        assert!(!app.world().resource::<State>().editing());
        assert!(!app.world().resource::<State>().pending());
        press(&mut app, Choice::Field(FieldKind::Import));
        assert!(app.world().resource::<State>().editing());
        app.world_mut().resource_mut::<State>().import = "50".into();
        press(&mut app, Choice::SaveTariffs);
        await_save(&mut app);
        assert_eq!(
            app.world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .caravan_policy()
                .import_tariff_basis_points,
            5000
        );
    }

    #[test]
    fn hiding_policy_clears_focus_and_preserves_drafts() {
        let mut app = app();
        press(&mut app, Choice::Field(FieldKind::Reserve));
        app.world_mut().resource_mut::<State>().drafts[Good::Flour as usize].reserve = "123".into();
        assert!(app.world().resource::<State>().editing());
        let owner = app.world().resource::<State>().field_owner.unwrap();
        app.world_mut()
            .entity_mut(owner)
            .insert(InheritedVisibility::HIDDEN);
        app.update();
        assert!(!app.world().resource::<State>().editing());
        assert_eq!(
            app.world().resource::<State>().drafts[Good::Flour as usize].reserve,
            "123"
        );
    }

    #[test]
    fn good_and_tariff_saves_preserve_other_unsaved_drafts() {
        let mut app = app();
        {
            let mut state = app.world_mut().resource_mut::<State>();
            state.drafts[Good::Flour as usize].allowed = false;
            state.drafts[Good::Flour as usize].imports_allowed = false;
            state.drafts[Good::Flour as usize].import = "-25".into();
            state.drafts[Good::Flour as usize].export = "150".into();
            state.drafts[Good::Flour as usize].reserve = "1234".into();
            state.drafts[Good::BreadRoll as usize].reserve = "7".into();
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
        assert!(!policy.exports[Good::Flour as usize].allowed);
        assert!(!policy.exports[Good::Flour as usize].imports_allowed);
        assert_eq!(
            policy.exports[Good::Flour as usize].import_tariff_basis_points,
            -2500
        );
        assert_eq!(
            policy.exports[Good::Flour as usize].export_tariff_basis_points,
            15000
        );
        assert_eq!(policy.exports[Good::Flour as usize].minimum_reserve, 1234);
        assert_eq!(policy.import_tariff_basis_points, 0);
        assert_eq!(app.world().resource::<State>().import, "50");
        assert_eq!(
            app.world().resource::<State>().drafts[Good::BreadRoll as usize].reserve,
            "7"
        );
        app.world_mut().resource_mut::<State>().drafts[Good::Flour as usize].import = "-50".into();
        press(&mut app, Choice::SaveTariffs);
        await_save(&mut app);
        assert_eq!(
            app.world().resource::<State>().drafts[Good::Flour as usize].import,
            "-50"
        );
        let policy = app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .caravan_policy();
        assert_eq!(policy.import_tariff_basis_points, 5000);
        assert_eq!(policy.export_tariff_basis_points, 1234);
        assert_eq!(policy.exports[Good::Flour as usize].minimum_reserve, 1234);
    }
    #[test]
    fn invalid_and_failed_saves_keep_drafts_and_restart_discards_stale_clicks() {
        let mut app = app();
        app.world_mut().resource_mut::<State>().drafts[Good::Flour as usize].reserve = "1.5".into();
        press(&mut app, Choice::SaveGood);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(
            app.world().resource::<State>().drafts[Good::Flour as usize].reserve,
            "1.5"
        );
        app.world_mut().resource_mut::<State>().import = "100.001".into();
        press(&mut app, Choice::SaveTariffs);
        assert!(
            app.world()
                .resource::<State>()
                .errors
                .contains_key(&Saved::Tariffs)
        );
        assert_eq!(app.world().resource::<State>().import, "100.001");
        app.world_mut().resource_mut::<State>().pending = Some((0, Saved::Tariffs));
        {
            let mut snapshot = app.world_mut().resource_mut::<DisplaySnapshot>();
            snapshot.0.mutation_revision = 1;
            snapshot.0.mutation_error = Some("Worker rejected change".into());
        }
        app.update();
        assert_eq!(app.world().resource::<State>().import, "100.001");
        assert_eq!(
            app.world()
                .resource::<State>()
                .errors
                .get(&Saved::Tariffs)
                .map(String::as_str),
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
        app.world_mut().resource_mut::<State>().drafts[Good::Flour as usize].reserve = "123".into();
        press(&mut app, Choice::SaveTariffs);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(app.world().resource::<State>().import, "50");
        assert!(
            app.world()
                .resource::<State>()
                .errors
                .get(&Saved::Tariffs)
                .unwrap()
                .contains("Another change")
        );
        press(&mut app, Choice::SaveGood);
        assert!(!app.world().resource::<State>().pending());
        assert_eq!(
            app.world().resource::<State>().drafts[Good::Flour as usize].reserve,
            "123"
        );
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
        app.world_mut().resource_mut::<State>().pending = Some((0, Saved::Good(Good::Flour)));
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
