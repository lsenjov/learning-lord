use crate::{
    DisplaySnapshot, PANEL, SELECTED, TEXT,
    floating_ui::Windows,
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
use learning_lord_simulation::StartingRole;

const ROLES: [StartingRole; 10] = [
    StartingRole::Farmer,
    StartingRole::Miller,
    StartingRole::Woodcutter,
    StartingRole::Baker,
    StartingRole::Weaver,
    StartingRole::Tailor,
    StartingRole::Fisher,
    StartingRole::Brewer,
    StartingRole::Carpenter,
    StartingRole::Innkeeper,
];

#[derive(Resource)]
pub(crate) struct State {
    enabled: bool,
    role: usize,
    funds: String,
    editing: bool,
    replace: bool,
    pending: Option<u64>,
    generation: u64,
    status: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            role: 0,
            funds: "0".into(),
            editing: false,
            replace: false,
            pending: None,
            generation: 0,
            status: String::new(),
        }
    }
}
impl State {
    pub(crate) fn stop_editing(&mut self) {
        self.editing = false;
    }
    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn editing(&self) -> bool {
        self.editing
    }
}
#[derive(Component)]
pub(crate) struct Content;
#[derive(Component, Clone, Copy)]
pub(crate) enum Choice {
    Toggle,
    Role,
    Funds,
    Member,
    Household,
    RemoveCitizen,
    RemoveHousehold,
    LowerLivingTier,
    RaiseLivingTier,
}

pub(crate) fn spawn(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Content,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            ..default()
        },
    ));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    choice: Choice,
    selected: bool,
    tip: &str,
) {
    parent
        .spawn((
            Button,
            choice,
            Node {
                padding: UiRect::axes(px(7), px(3)),
                width: if matches!(choice, Choice::Funds) {
                    px(220)
                } else {
                    Val::Auto
                },
                max_width: if matches!(choice, Choice::Funds) {
                    px(220)
                } else {
                    Val::Auto
                },
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(if selected { SELECTED } else { PANEL }),
            crate::tooltip::Tooltip(tip.into()),
        ))
        .with_children(|p| {
            p.spawn(text(label, 13.0, TEXT));
        });
}

pub(crate) fn handle(
    mut state: ResMut<State>,
    snapshot: Res<DisplaySnapshot>,
    worker: Res<SimulationWorker>,
    windows: Res<Windows>,
    citizen: Res<crate::citizens::Selection>,
    controls: Res<crate::Controls>,
    mut locations: ResMut<crate::locations::State>,
    construction: Res<crate::construction::State>,
    mut caravan: ResMut<crate::caravan_controls::State>,
    choices: Query<(&Interaction, &Choice), Changed<Interaction>>,
    mut keys: MessageReader<KeyboardInput>,
) {
    if state.generation != snapshot.0.generation {
        state.generation = snapshot.0.generation;
        state.pending = None;
        state.editing = false;
        state.status.clear();
        keys.clear();
        return;
    }
    if state
        .pending
        .is_some_and(|revision| revision != snapshot.0.mutation_revision)
    {
        state.pending = None;
        state.status = snapshot
            .0
            .mutation_error
            .clone()
            .unwrap_or_else(|| "Change saved".into());
    }
    if locations.editing() || caravan.editing() || controls.restart_pending {
        state.editing = false;
    }
    let selected_household = windows
        .selected_household()
        .filter(|id| snapshot.0.universe.households().contains_key(id));
    for (interaction, choice) in &choices {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(choice, Choice::Toggle) {
            state.enabled = !state.enabled;
            state.editing = false;
            continue;
        }
        if !state.enabled || state.pending.is_some() || controls.restart_pending {
            continue;
        }
        match choice {
            Choice::Toggle => unreachable!(),
            Choice::Role => {
                state.role = (state.role + 1) % ROLES.len();
                state.editing = false;
            }
            Choice::Funds => {
                crate::locations::stop_editing(&mut locations);
                crate::caravan_controls::stop_editing(&mut caravan);
                state.editing = true;
                state.replace = true;
            }
            Choice::LowerLivingTier | Choice::RaiseLivingTier => {
                state.editing = false;
                if locations.pending() || construction.pending() || caravan.pending() {
                    state.status = "Another change is saving. Try again when it finishes.".into();
                    continue;
                }
                let Some(household) = selected_household else {
                    state.status = "Select a household in Collectives > Households".into();
                    continue;
                };
                let current = snapshot.0.universe.households()[&household]
                    .living_standard
                    .tier;
                let tier = if matches!(choice, Choice::RaiseLivingTier) {
                    current.saturating_add(1).min(5)
                } else {
                    current.saturating_sub(1).max(1)
                };
                if tier == current {
                    continue;
                }
                match worker.send(Command::Mutate {
                    generation: snapshot.0.generation,
                    mutation: Mutation::SetHouseholdLivingTier { household, tier },
                }) {
                    Ok(()) => {
                        state.pending = Some(snapshot.0.mutation_revision);
                        state.status = "Saving...".into();
                    }
                    Err(error) => state.status = error,
                }
            }
            Choice::RemoveCitizen | Choice::RemoveHousehold => {
                state.editing = false;
                if locations.pending() || construction.pending() || caravan.pending() {
                    state.status = "Another change is saving. Try again when it finishes.".into();
                    continue;
                }
                let mutation = if matches!(choice, Choice::RemoveCitizen) {
                    let Some(id) = citizen
                        .0
                        .filter(|id| snapshot.0.universe.agents().contains_key(id))
                    else {
                        state.status = "Select a citizen".into();
                        continue;
                    };
                    Mutation::RemoveCitizen(id)
                } else {
                    let Some(id) = selected_household else {
                        state.status = "Select a household in Collectives > Households".into();
                        continue;
                    };
                    Mutation::RemoveHousehold(id)
                };
                match worker.send(Command::Mutate {
                    generation: snapshot.0.generation,
                    mutation,
                }) {
                    Ok(()) => {
                        state.pending = Some(snapshot.0.mutation_revision);
                        state.status = "Removing...".into();
                    }
                    Err(error) => state.status = error,
                }
            }
            Choice::Member | Choice::Household => {
                state.editing = false;
                let household = if matches!(choice, Choice::Member) {
                    match selected_household {
                        Some(id) => Some(id),
                        _ => {
                            state.status = "Select a household in Collectives first".into();
                            continue;
                        }
                    }
                } else {
                    None
                };
                let funds = if household.is_some() {
                    0
                } else {
                    match state.funds.parse::<i64>() {
                        Ok(funds) if funds >= 0 => funds,
                        _ => {
                            state.status = "Funds must be a whole non-negative coin amount".into();
                            continue;
                        }
                    }
                };
                if locations.pending() || construction.pending() || caravan.pending() {
                    state.status = "Another change is saving. Try again when it finishes.".into();
                    continue;
                }
                match worker.send(Command::Mutate {
                    generation: snapshot.0.generation,
                    mutation: Mutation::AddCitizen {
                        household,
                        role: ROLES[state.role],
                        funds,
                    },
                }) {
                    Ok(()) => {
                        state.pending = Some(snapshot.0.mutation_revision);
                        state.status = "Adding...".into();
                    }
                    Err(error) => state.status = error,
                }
            }
        }
    }
    for key in keys.read() {
        if !state.editing || !state.enabled || key.state != ButtonState::Pressed {
            continue;
        }
        match &key.logical_key {
            Key::Escape | Key::Enter | Key::Tab => state.editing = false,
            Key::Backspace => {
                if state.replace {
                    state.funds.clear();
                    state.replace = false;
                } else {
                    state.funds.pop();
                }
            }
            Key::Character(value) if value.chars().all(|c| c.is_ascii_digit() || c == '-') => {
                if state.replace {
                    state.funds.clear();
                    state.replace = false;
                }
                let remaining = 19_usize.saturating_sub(state.funds.len());
                state.funds.extend(value.chars().take(remaining));
            }
            _ => {}
        }
    }
}

pub(crate) fn refresh(
    mut commands: Commands,
    state: Res<State>,
    roots: Query<Entity, With<Content>>,
) {
    if !state.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn_children().with_children(|p| {
            button(p, if state.enabled { "Dev: on" } else { "Dev: off" }, Choice::Toggle, state.enabled, "Local UI preference. Created citizens and funds remain in simulation saves.");
            if !state.enabled { return; }
            p.spawn(Node { column_gap: px(4), ..default() }).with_children(|p| {
                button(p, ROLES[state.role].name(), Choice::Role, false, "Cycle profession. Grants skill metadata only, without property, tools or guild membership.");
                button(p, &format!("Funds: {}", state.funds), Choice::Funds, state.editing, "Starting coins for a new household. Click and type a whole non-negative amount. Adding a member adds no coins.");
            });
            p.spawn(Node { column_gap: px(4), ..default() }).with_children(|p| {
                button(p, "Add citizen", Choice::Member, false, "Add to the household selected in Collectives > Households. Shares its home, without adding a bed.");
                button(p, "New household", Choice::Household, false, "Create a homeless household with one citizen and the chosen funds.");
            });
            p.spawn(Node { column_gap: px(4), ..default() }).with_children(|p| {
                button(p, "Remove citizen", Choice::RemoveCitizen, false, "Remove the selected citizen. The final member removes the household.");
                button(p, "Remove household", Choice::RemoveHousehold, false, "Remove the household selected in Collectives > Households, settling debts and transferring buildings to town.");
            });
            p.spawn(Node { column_gap: px(4), ..default() }).with_children(|p| {
                button(p, "Tier -", Choice::LowerLivingTier, false, "Lower the selected household's living tier. Resets assessment eligibility and delays promotion for 14 days.");
                button(p, "Tier +", Choice::RaiseLivingTier, false, "Raise the selected household's living tier. Resets the seven-day assessment period.");
            });
            if !state.status.is_empty() { p.spawn(text(&state.status, 13.0, TEXT)); }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let worker = SimulationWorker::spawn(
            crate::simulation::new_universe().unwrap(),
            std::num::NonZeroU32::new(60).unwrap(),
        );
        let mut app = App::new();
        app.insert_resource(DisplaySnapshot(worker.snapshot()))
            .insert_resource(worker)
            .init_resource::<State>()
            .init_resource::<Windows>()
            .init_resource::<crate::citizens::Selection>()
            .init_resource::<crate::Controls>()
            .init_resource::<crate::locations::State>()
            .init_resource::<crate::construction::State>()
            .init_resource::<crate::caravan_controls::State>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, (handle, refresh).chain());
        app.world_mut().spawn((Content, Node::default()));
        app.update();
        app
    }
    fn press(app: &mut App, choice: Choice) {
        app.world_mut().resource_mut::<Windows>().take_focus();
        let entity = app.world_mut().spawn((choice, Interaction::Pressed)).id();
        app.update();
        app.world_mut().despawn(entity);
    }
    #[test]
    fn toggle_gates_actions_and_menu_and_survives_world_changes() {
        let mut app = app();
        let choice_count =
            |app: &mut App| app.world_mut().query::<&Choice>().iter(app.world()).count();
        assert_eq!(choice_count(&mut app), 1);
        press(&mut app, Choice::Household);
        assert!(!app.world().resource::<State>().pending());
        press(&mut app, Choice::Toggle);
        assert_eq!(choice_count(&mut app), 9);
        press(&mut app, Choice::Funds);
        assert!(app.world().resource::<State>().editing());
        press(&mut app, Choice::Toggle);
        assert!(!app.world().resource::<State>().editing());
        assert_eq!(choice_count(&mut app), 1);
        press(&mut app, Choice::Toggle);
        app.world_mut()
            .resource_mut::<DisplaySnapshot>()
            .0
            .generation += 1;
        app.update();
        assert!(app.world().resource::<State>().enabled);
    }
    #[test]
    fn invalid_funds_and_missing_selection_are_clear_and_do_not_send() {
        let mut app = app();
        press(&mut app, Choice::Toggle);
        press(&mut app, Choice::Member);
        assert!(
            app.world()
                .resource::<State>()
                .status
                .contains("Select a household")
        );
        for funds in ["-1", "", "99999999999999999999999"] {
            app.world_mut().resource_mut::<State>().funds = funds.into();
            press(&mut app, Choice::Household);
            let state = app.world().resource::<State>();
            assert!(state.status.contains("whole non-negative"));
            assert!(!state.pending());
        }
    }
    #[test]
    fn living_tier_controls_require_selection_and_respect_bounds() {
        let mut app = app();
        press(&mut app, Choice::Toggle);
        press(&mut app, Choice::RaiseLivingTier);
        assert!(
            app.world()
                .resource::<State>()
                .status
                .contains("Select a household")
        );
        assert!(!app.world().resource::<State>().pending());
        let household = *app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .households()
            .keys()
            .next()
            .unwrap();
        app.world_mut()
            .resource_mut::<Windows>()
            .open(crate::floating_ui::WindowKind::Household(household));
        press(&mut app, Choice::LowerLivingTier);
        assert!(!app.world().resource::<State>().pending());
        press(&mut app, Choice::RaiseLivingTier);
        assert!(app.world().resource::<State>().pending());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let snapshot = app.world().resource::<SimulationWorker>().snapshot();
            if snapshot.mutation_revision > 0 {
                assert!(snapshot.mutation_error.is_none());
                assert_eq!(
                    snapshot.universe.households()[&household]
                        .living_standard
                        .tier,
                    2
                );
                let universe = snapshot
                    .universe
                    .set_household_living_tier(household, 5)
                    .unwrap();
                let mut snapshot = snapshot;
                snapshot.universe = universe;
                app.world_mut().resource_mut::<DisplaySnapshot>().0 = snapshot;
                app.update();
                press(&mut app, Choice::RaiseLivingTier);
                assert!(!app.world().resource::<State>().pending());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn funds_keyboard_replaces_then_appends_and_blocks_shortcuts() {
        let mut app = app();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, |shortcuts: crate::KeyboardShortcuts| {
            assert!(shortcuts.editing());
        });
        app.world_mut().resource_mut::<State>().enabled = true;
        app.world_mut().resource_mut::<State>().editing = true;
        app.world_mut().resource_mut::<State>().replace = true;
        for value in ["12", "3", "456789012345678901234567890"] {
            app.world_mut()
                .resource_mut::<Messages<KeyboardInput>>()
                .write(KeyboardInput {
                    key_code: KeyCode::Digit1,
                    logical_key: Key::Character(value.into()),
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                });
            app.update();
        }
        assert_eq!(app.world().resource::<State>().funds, "1234567890123456789");
    }

    #[test]
    fn member_uses_selected_household_and_profession_without_adding_funds() {
        let mut app = app();
        let household = *app
            .world()
            .resource::<DisplaySnapshot>()
            .0
            .universe
            .households()
            .keys()
            .next()
            .unwrap();
        app.world_mut()
            .resource_mut::<Windows>()
            .open(crate::floating_ui::WindowKind::Household(household));
        press(&mut app, Choice::Toggle);
        press(&mut app, Choice::Role);
        app.world_mut().resource_mut::<State>().funds = "invalid".into();
        press(&mut app, Choice::Member);
        assert!(app.world().resource::<State>().pending());
        let before = &app.world().resource::<DisplaySnapshot>().0.universe;
        let count = before.households()[&household].members.len();
        let coins = before.households()[&household].coins;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let snapshot = app.world().resource::<SimulationWorker>().snapshot();
            if snapshot.mutation_revision > 0 {
                assert!(snapshot.mutation_error.is_none());
                let family = &snapshot.universe.households()[&household];
                assert_eq!(family.members.len(), count + 1);
                assert_eq!(family.coins, coins);
                let learning_lord_simulation::AgentKind::Citizen(citizen) =
                    &snapshot.universe.agents()[family.members.last().unwrap()].kind;
                assert_eq!(citizen.starting_role(), Some(StartingRole::Miller));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn innkeeper_selector_creates_role_without_free_property_or_equipment() {
        let mut app = app();
        press(&mut app, Choice::Toggle);
        for _ in 0..ROLES.len() - 1 {
            press(&mut app, Choice::Role);
        }
        assert_eq!(
            ROLES[app.world().resource::<State>().role],
            StartingRole::Innkeeper
        );
        let before = app.world().resource::<DisplaySnapshot>().0.universe.clone();
        press(&mut app, Choice::Household);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let snapshot = app.world().resource::<SimulationWorker>().snapshot();
            if snapshot.mutation_revision > 0 {
                assert!(snapshot.mutation_error.is_none());
                assert_eq!(snapshot.universe.map(), before.map());
                assert!(snapshot.universe.inns().is_empty());
                let added = snapshot
                    .universe
                    .agents()
                    .iter()
                    .find(|(id, _)| !before.agents().contains_key(*id))
                    .map(|(_, agent)| agent)
                    .unwrap();
                let learning_lord_simulation::AgentKind::Citizen(citizen) = &added.kind;
                assert_eq!(citizen.starting_role(), Some(StartingRole::Innkeeper));
                assert!(citizen.home().is_none());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn toggle_off_does_not_undo_creation() {
        let mut app = app();
        press(&mut app, Choice::Toggle);
        app.world_mut().resource_mut::<State>().funds = "42".into();
        press(&mut app, Choice::Household);
        assert!(app.world().resource::<State>().pending());
        press(&mut app, Choice::Toggle);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let snapshot = app.world().resource::<SimulationWorker>().snapshot();
            if snapshot.mutation_revision > 0 {
                app.world_mut().resource_mut::<DisplaySnapshot>().0 = snapshot;
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        app.update();
        assert!(!app.world().resource::<State>().enabled);
        let world = &app.world().resource::<DisplaySnapshot>().0.universe;
        assert_eq!(world.households().len(), 28);
        let loaded =
            learning_lord_simulation::Universe::from_save_str(&world.to_save_string().unwrap())
                .unwrap();
        assert!(
            loaded
                .households()
                .values()
                .any(|h| h.home.is_none() && h.coins == 42)
        );
    }
    #[test]
    fn remove_controls_use_selected_targets_and_refresh_status() {
        for household_removal in [false, true] {
            let mut app = app();
            let family = app
                .world()
                .resource::<DisplaySnapshot>()
                .0
                .universe
                .households()
                .values()
                .find(|h| h.members.len() > 1)
                .unwrap()
                .clone();
            let member = family.members[0];
            app.world_mut()
                .resource_mut::<crate::citizens::Selection>()
                .0 = Some(member);
            app.world_mut()
                .resource_mut::<Windows>()
                .open(crate::floating_ui::WindowKind::Household(family.id));
            press(&mut app, Choice::Toggle);
            press(
                &mut app,
                if household_removal {
                    Choice::RemoveHousehold
                } else {
                    Choice::RemoveCitizen
                },
            );
            assert!(app.world().resource::<State>().pending());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let snapshot = app.world().resource::<SimulationWorker>().snapshot();
                if snapshot.mutation_revision > 0 {
                    assert!(
                        snapshot.mutation_error.is_none(),
                        "{:?}",
                        snapshot.mutation_error
                    );
                    assert!(!snapshot.universe.agents().contains_key(&member));
                    assert_eq!(
                        !snapshot.universe.households().contains_key(&family.id),
                        household_removal
                    );
                    app.world_mut().insert_resource(DisplaySnapshot(snapshot));
                    app.update();
                    assert!(!app.world().resource::<State>().pending());
                    assert_eq!(app.world().resource::<State>().status, "Change saved");
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }

    #[test]
    fn remove_controls_require_valid_selection_and_dev_mode() {
        let mut app = app();
        press(&mut app, Choice::RemoveCitizen);
        assert!(!app.world().resource::<State>().pending());
        press(&mut app, Choice::Toggle);
        press(&mut app, Choice::RemoveCitizen);
        assert!(
            app.world()
                .resource::<State>()
                .status
                .contains("Select a citizen")
        );
        press(&mut app, Choice::RemoveHousehold);
        assert!(
            app.world()
                .resource::<State>()
                .status
                .contains("Select a household")
        );
        assert!(!app.world().resource::<State>().pending());
    }
}
