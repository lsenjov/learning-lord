use bevy::prelude::Resource;
use learning_lord_simulation::{Citizen, PlanningRuntime, SimulationError, Universe};
use std::{
    collections::VecDeque,
    num::NonZeroU32,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const SPEEDS: [u32; 6] = [1, 2, 3, 5, 10, 20];
pub const START_TIME_MS: u64 = 6 * 60 * 60 * 1000;
pub const STEP_MS: u64 = 30 * 60 * 1000;
const NANOS_PER_MS: u128 = 1_000_000;

pub fn update_rate() -> Result<NonZeroU32, String> {
    match std::env::var("LEARNING_LORD_MAX_UPDATES_PER_SECOND") {
        Ok(value) => value
            .parse::<NonZeroU32>()
            .map_err(|_| "LEARNING_LORD_MAX_UPDATES_PER_SECOND must be a positive integer".into()),
        Err(std::env::VarError::NotPresent) => Ok(NonZeroU32::new(60).unwrap()),
        Err(error) => Err(error.to_string()),
    }
}

#[derive(Clone, Copy)]
pub enum Command {
    SetRunning(bool),
    SetSpeed(u32),
    Step,
    NextDay,
    Restart,
    Shutdown,
}

struct TimedCommand {
    command: Command,
    at: Instant,
}

#[derive(Clone)]
pub struct Snapshot {
    pub universe: Universe,
    pub planning_history: std::collections::HashMap<
        learning_lord_simulation::AgentId,
        learning_lord_simulation::history::PlanningHistory,
    >,
    pub error: Option<String>,
    pub generation: u64,
    pub revision: u64,
}

#[derive(Resource)]
pub struct SimulationWorker {
    commands: mpsc::Sender<TimedCommand>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}

impl SimulationWorker {
    #[cfg(test)]
    pub fn spawn(universe: Universe, updates_per_second: NonZeroU32) -> Self {
        Self::spawn_with_wakeup(universe, updates_per_second, || {})
    }

    pub fn spawn_with_wakeup(
        universe: Universe,
        updates_per_second: NonZeroU32,
        wakeup: impl Fn() + Send + 'static,
    ) -> Self {
        let snapshot = Arc::new(Mutex::new(Snapshot {
            universe: universe.clone(),
            planning_history: Default::default(),
            error: None,
            generation: 0,
            revision: 0,
        }));
        let published = Arc::clone(&snapshot);
        let (commands, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("simulation".into())
            .spawn(move || run_worker(universe, updates_per_second, receiver, published, wakeup))
            .expect("could not start the simulation worker");
        Self {
            commands,
            snapshot,
            thread: Some(thread),
        }
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.commands
            .send(TimedCommand {
                command,
                at: Instant::now(),
            })
            .map_err(|_| "The simulation worker has stopped.".into())
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .expect("snapshot lock poisoned")
            .clone()
    }

    pub fn snapshot_after(&self, revision: u64) -> Option<Snapshot> {
        let snapshot = self.snapshot.lock().expect("snapshot lock poisoned");
        (snapshot.revision != revision).then(|| snapshot.clone())
    }
}

impl Drop for SimulationWorker {
    fn drop(&mut self) {
        let _ = self.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Pacing {
    running: bool,
    speed: u32,
    interval: Duration,
    accounted_at: Duration,
    last_update: Duration,
    pending_simulated_ns: u128,
}

impl Pacing {
    fn new(updates_per_second: NonZeroU32) -> Self {
        Self {
            running: false,
            speed: 1,
            interval: Duration::from_nanos(
                1_000_000_000u64.div_ceil(updates_per_second.get().into()),
            ),
            accounted_at: Duration::ZERO,
            last_update: Duration::ZERO,
            pending_simulated_ns: 0,
        }
    }

    fn account_until(&mut self, now: Duration) {
        // A command can arrive just after an update starts; never count that interval twice.
        let now = now.max(self.accounted_at);
        if self.running {
            self.pending_simulated_ns +=
                (now - self.accounted_at).as_nanos() * 60 * self.speed as u128;
        }
        self.accounted_at = now;
    }

    fn set_running(&mut self, running: bool, now: Duration) {
        self.account_until(now);
        if running && !self.running {
            self.last_update = self.accounted_at;
        }
        self.running = running;
    }

    fn set_speed(&mut self, speed: u32, now: Duration) {
        self.account_until(now);
        self.speed = speed;
    }

    fn wait_duration(&self, now: Duration) -> Option<Duration> {
        self.running.then(|| {
            self.interval
                .saturating_sub(now.saturating_sub(self.last_update))
        })
    }

    fn take_update(&mut self, now: Duration) -> Result<Option<u64>, SimulationError> {
        if self.wait_duration(now) != Some(Duration::ZERO) {
            return Ok(None);
        }
        self.account_until(now);
        let elapsed_ms = u64::try_from(self.pending_simulated_ns / NANOS_PER_MS)
            .map_err(|_| SimulationError::TimeOverflow)?;
        self.pending_simulated_ns %= NANOS_PER_MS;
        self.last_update = now;
        Ok(Some(elapsed_ms))
    }
}

pub const STARTING_CITIZENS: [(&str, learning_lord_simulation::StartingRole); 6] = {
    use learning_lord_simulation::StartingRole;
    [
        ("Ada", StartingRole::Farmer),
        ("Bram", StartingRole::Miller),
        ("Cleo", StartingRole::Woodcutter),
        ("Dara", StartingRole::Baker),
        ("Eira", StartingRole::Weaver),
        ("Finn", StartingRole::Tailor),
    ]
};

pub fn new_universe() -> Result<Universe, SimulationError> {
    let mut universe = Universe::starting_at(START_TIME_MS);
    let mut ids = Vec::new();
    for (name, role) in STARTING_CITIZENS {
        let mut citizen = Citizen::with_needs(-50.0, -66.6)?
            .with_berries(310)?
            .with_garment_condition(Some(0.5))?
            .with_starting_role(role)
            .with_coins(learning_lord_simulation::production::starting_coins(role))?;
        for (good, units) in learning_lord_simulation::production::starting_inputs(role).items() {
            citizen = citizen.with_good(good, units)?;
        }
        let (next, id) = universe.with_citizen(name, citizen)?;
        universe = next;
        ids.push(id);
    }
    use learning_lord_simulation::locations::Location;
    for (owner, kind) in [
        (ids[0], Location::Field),
        (ids[1], Location::Mill),
        (ids[3], Location::Bakery),
        (ids[4], Location::Weavery),
        (ids[5], Location::Tailory),
    ] {
        universe = universe.with_property(owner, kind)?.0;
    }
    for id in ids {
        universe = universe.start_planning(id)?;
    }
    Ok(universe)
}

struct WorkerState {
    universe: Universe,
    planner: PlanningRuntime,
    pacing: Pacing,
    error: Option<String>,
    generation: u64,
}

impl WorkerState {
    fn advance_interruptibly(
        &mut self,
        elapsed_ms: u64,
        should_cancel: impl FnMut() -> bool,
    ) -> bool {
        match self
            .universe
            .advance_with_planner(&mut self.planner, elapsed_ms, should_cancel)
        {
            Ok(Some(universe)) => {
                self.universe = universe;
                true
            }
            Ok(None) => false,
            Err(error) => {
                self.fail(error);
                true
            }
        }
    }

    #[cfg(test)]
    fn advance(&mut self, elapsed_ms: u64) {
        self.advance_interruptibly(elapsed_ms, || false);
    }

    #[cfg(test)]
    fn apply(&mut self, command: Command, at: Duration) -> bool {
        self.apply_interruptibly(command, at, || false).is_some()
    }

    fn fail(&mut self, error: SimulationError) {
        self.error = Some(error.to_string());
        self.pacing.running = false;
    }

    fn apply_interruptibly(
        &mut self,
        command: Command,
        at: Duration,
        should_cancel: impl FnMut() -> bool,
    ) -> Option<bool> {
        if matches!(command, Command::Shutdown) {
            return None;
        }
        if matches!(command, Command::Restart) {
            self.planner.reset();
            self.generation += 1;
            match new_universe() {
                Ok(universe) => {
                    self.universe = universe;
                    self.error = None;
                    let at = at.max(self.pacing.accounted_at);
                    self.pacing.running = false;
                    self.pacing.speed = 1;
                    self.pacing.accounted_at = at;
                    self.pacing.last_update = at;
                    self.pacing.pending_simulated_ns = 0;
                }
                Err(error) => self.fail(error),
            }
            return Some(true);
        }
        if self.error.is_some() {
            return Some(true);
        }
        match command {
            Command::SetRunning(running) => self.pacing.set_running(running, at),
            Command::SetSpeed(speed) => self.pacing.set_speed(speed, at),
            Command::Step if !self.pacing.running => {
                return Some(self.advance_interruptibly(STEP_MS, should_cancel));
            }
            Command::NextDay if !self.pacing.running => {
                const DAY_MS: u64 = 24 * 60 * 60 * 1000;
                let time = self.universe.current_time_ms();
                let elapsed = DAY_MS - time % DAY_MS + 4 * 60 * 60 * 1000;
                return Some(self.advance_interruptibly(elapsed, should_cancel));
            }
            Command::Step | Command::NextDay | Command::Shutdown | Command::Restart => {}
        }
        Some(true)
    }

    fn publish(&self, published: &Mutex<Snapshot>) {
        let mut published = published.lock().expect("snapshot lock poisoned");
        *published = Snapshot {
            universe: self.universe.clone(),
            planning_history: self
                .universe
                .agents()
                .keys()
                .filter_map(|id| {
                    self.planner
                        .history(*id)
                        .cloned()
                        .map(|history| (*id, history))
                })
                .collect(),
            error: self.error.clone(),
            generation: self.generation,
            revision: published.revision + 1,
        };
    }
}

fn poll_cancellation(
    commands: &mpsc::Receiver<TimedCommand>,
    deferred: &mut VecDeque<TimedCommand>,
) -> bool {
    if deferred
        .iter()
        .any(|command| matches!(command.command, Command::Restart | Command::Shutdown))
    {
        return true;
    }
    loop {
        match commands.try_recv() {
            Ok(command) => {
                let interrupt = matches!(command.command, Command::Restart | Command::Shutdown);
                deferred.push_back(command);
                if interrupt {
                    return true;
                }
            }
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn run_worker(
    universe: Universe,
    updates_per_second: NonZeroU32,
    commands: mpsc::Receiver<TimedCommand>,
    published: Arc<Mutex<Snapshot>>,
    wakeup: impl Fn(),
) {
    let epoch = Instant::now();
    let mut state = WorkerState {
        universe,
        planner: PlanningRuntime::default(),
        pacing: Pacing::new(updates_per_second),
        error: None,
        generation: 0,
    };
    let mut deferred = VecDeque::new();
    loop {
        let command = match deferred.pop_front() {
            Some(command) => Some(command),
            None => match commands.try_recv() {
                Ok(command) => Some(command),
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => None,
            },
        };
        if let Some(command) = command {
            let Some(changed) = state.apply_interruptibly(
                command.command,
                command.at.saturating_duration_since(epoch),
                || poll_cancellation(&commands, &mut deferred),
            ) else {
                break;
            };
            if changed {
                state.publish(&published);
                wakeup();
            }
            continue;
        }

        let now = epoch.elapsed();
        if state.pacing.wait_duration(now) == Some(Duration::ZERO) {
            let changed = match state.pacing.take_update(now) {
                Ok(Some(elapsed_ms)) => state.advance_interruptibly(elapsed_ms, || {
                    poll_cancellation(&commands, &mut deferred)
                }),
                Err(error) => {
                    state.fail(error);
                    true
                }
                Ok(None) => false,
            };
            if changed {
                state.publish(&published);
                wakeup();
            }
            continue;
        }

        let received = match state.pacing.wait_duration(now) {
            Some(wait) => match commands.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => commands.recv().ok(),
        };
        let Some(command) = received else { break };
        deferred.push_back(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use learning_lord_simulation::Citizen;

    fn pacing(rate: u32) -> Pacing {
        let mut pacing = Pacing::new(NonZeroU32::new(rate).unwrap());
        pacing.set_running(true, Duration::ZERO);
        pacing
    }

    fn worker_state(universe: Universe) -> WorkerState {
        WorkerState {
            universe,
            planner: PlanningRuntime::default(),
            pacing: Pacing::new(NonZeroU32::new(60).unwrap()),
            error: None,
            generation: 0,
        }
    }

    #[test]
    fn restart_and_shutdown_interrupt_waits_without_publishing_stale_advances() {
        for interrupt in [Command::Restart, Command::Shutdown] {
            let (universe, _) =
                Universe::with_map(learning_lord_simulation::locations::Map::default())
                    .with_citizen("Ada", Citizen::new(0.0).unwrap())
                    .unwrap();
            let source = universe.clone();
            let (commands, receiver) = mpsc::channel();
            let (waiting, started) = mpsc::channel();
            let (release, gate) = mpsc::channel();
            let thread = thread::spawn(move || {
                let mut state = worker_state(universe);
                let mut deferred = VecDeque::new();
                let changed = state.advance_interruptibly(STEP_MS, || {
                    waiting.send(()).unwrap();
                    gate.recv().unwrap();
                    poll_cancellation(&receiver, &mut deferred)
                });
                assert!(!changed);
                let cancelled = state.universe.clone();
                let command = deferred.pop_front().unwrap();
                let outcome = state.apply_interruptibly(command.command, Duration::ZERO, || false);
                (cancelled, state, outcome)
            });
            started.recv_timeout(Duration::from_secs(5)).unwrap();
            commands
                .send(TimedCommand {
                    command: interrupt,
                    at: Instant::now(),
                })
                .unwrap();
            release.send(()).unwrap();
            let (cancelled, state, outcome) = thread.join().unwrap();
            assert_eq!(cancelled, source);
            assert!(state.error.is_none());
            match interrupt {
                Command::Restart => {
                    assert_eq!(outcome, Some(true));
                    assert_eq!(state.generation, 1);
                    assert_eq!(state.universe.current_time_ms(), START_TIME_MS);
                }
                Command::Shutdown => assert_eq!(outcome, None),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn cancellation_defers_commands_in_order_and_skips_obsolete_steps_before_restart() {
        let (commands, receiver) = mpsc::channel();
        for command in [
            Command::SetSpeed(20),
            Command::Step,
            Command::NextDay,
            Command::SetRunning(true),
            Command::Restart,
            Command::SetSpeed(3),
            Command::Step,
        ] {
            commands
                .send(TimedCommand {
                    command,
                    at: Instant::now(),
                })
                .unwrap();
        }
        let mut deferred = VecDeque::new();
        assert!(poll_cancellation(&receiver, &mut deferred));
        assert_eq!(deferred.len(), 5);
        let (universe, _) = Universe::with_map(learning_lord_simulation::locations::Map::default())
            .with_citizen("Ada", Citizen::new(0.0).unwrap())
            .unwrap();
        let mut state = worker_state(universe.clone());
        let command = deferred.pop_front().unwrap();
        assert!(matches!(command.command, Command::SetSpeed(20)));
        state.apply_interruptibly(command.command, Duration::ZERO, || false);
        assert_eq!(state.pacing.speed, 20);
        for expected_next_day in [false, true] {
            let command = deferred.pop_front().unwrap();
            assert_eq!(
                matches!(command.command, Command::NextDay),
                expected_next_day
            );
            assert_eq!(
                state.apply_interruptibly(command.command, Duration::ZERO, || poll_cancellation(
                    &receiver,
                    &mut deferred
                )),
                Some(false)
            );
            assert_eq!(state.universe, universe);
        }
        let command = deferred.pop_front().unwrap();
        assert!(matches!(command.command, Command::SetRunning(true)));
        state.apply_interruptibly(command.command, Duration::ZERO, || false);
        assert!(state.pacing.running);
        let command = deferred.pop_front().unwrap();
        assert!(matches!(command.command, Command::Restart));
        state.apply_interruptibly(command.command, Duration::ZERO, || false);
        assert!(!state.pacing.running);
        assert_eq!(state.pacing.speed, 1);
        assert!(!poll_cancellation(&receiver, &mut deferred));
        assert_eq!(deferred.len(), 2);
        let command = deferred.pop_front().unwrap();
        assert!(matches!(command.command, Command::SetSpeed(3)));
        state.apply_interruptibly(command.command, Duration::ZERO, || false);
        assert_eq!(state.pacing.speed, 3);
        let command = deferred.pop_front().unwrap();
        assert!(matches!(command.command, Command::Step));
        assert_eq!(
            state.apply_interruptibly(command.command, Duration::ZERO, || false),
            Some(true)
        );
        assert_eq!(state.universe.current_time_ms(), START_TIME_MS + STEP_MS);
        assert!(deferred.is_empty());
    }

    #[test]
    fn publications_wake_after_step_restart_and_running_updates_but_not_while_idle() {
        let (wakeups, received) = mpsc::channel();
        let worker = SimulationWorker::spawn_with_wakeup(
            Universe::default(),
            NonZeroU32::new(60).unwrap(),
            move || {
                wakeups.send(()).unwrap();
            },
        );
        assert!(worker.snapshot_after(0).is_none());
        assert_eq!(
            received.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );

        worker.send(Command::Step).unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        let stepped = worker.snapshot_after(0).unwrap();
        assert_eq!(stepped.universe.current_time_ms(), STEP_MS);
        assert_eq!(stepped.revision, 1);
        assert!(worker.snapshot_after(stepped.revision).is_none());

        worker.send(Command::Restart).unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        let restarted = worker.snapshot_after(stepped.revision).unwrap();
        assert_eq!(restarted.generation, 1);
        assert_eq!(restarted.universe.current_time_ms(), START_TIME_MS);
        assert_eq!(restarted.revision, 2);
        assert_eq!(
            received.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );

        worker.send(Command::SetRunning(true)).unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            worker
                .snapshot_after(restarted.revision)
                .unwrap()
                .universe
                .current_time_ms()
                > 0
        );
        drop(worker);
    }

    #[test]
    fn six_citizens_start_with_agreed_role_supplies_and_advance_without_changing_the_source() {
        let universe = new_universe().unwrap();
        let source = universe.clone();
        assert_eq!(universe.agents().len(), 6);
        assert_eq!(universe.current_time_ms(), START_TIME_MS);
        assert!(universe.market().previous_period().is_none());
        assert_eq!(
            universe.market().current_period().start_ms,
            learning_lord_simulation::marketplace::UPDATE_TIME_MS
        );
        assert_eq!(
            universe.market().current_period().end_ms,
            learning_lord_simulation::marketplace::UPDATE_TIME_MS
                + learning_lord_simulation::marketplace::DAY_MS
        );
        for agent in universe.agents().values() {
            let learning_lord_simulation::AgentKind::Citizen(citizen) = &agent.kind;
            assert_eq!(citizen.hunger(), -50.0);
            assert_eq!(citizen.tiredness(), -66.6);
            assert_eq!(citizen.coins(), 300);
            assert_eq!(citizen.berries_units(), 310);
            assert_eq!(citizen.garment_condition(), Some(0.5));
            assert_eq!(citizen.map(), universe.map());
            assert_eq!(citizen.prices(), universe.prices());
            assert_eq!(citizen.position(), universe.map().position(citizen.home()));
            assert!(citizen.active_plan().is_some());
        }
        let started = Instant::now();
        let next = universe.advance(STEP_MS).unwrap();
        println!("First six-citizen step: {:?}", started.elapsed());
        assert_eq!(universe, source);
        assert_eq!(next.current_time_ms(), START_TIME_MS + STEP_MS);
        for (id, agent) in next.agents() {
            let learning_lord_simulation::AgentKind::Citizen(citizen) = &agent.kind;
            assert!(citizen.active_plan().is_some());
            assert_ne!(agent, &source.agents()[id]);
            assert!(citizen.hunger().is_finite());
        }
    }

    #[test]
    fn initial_town_runs_production_and_real_market_transactions() {
        use learning_lord_simulation::{AgentKind, marketplace::Good};
        let started = std::time::Instant::now();
        let source = new_universe().unwrap();
        let mut universe = source.clone();
        for step in 0..144 {
            universe = universe
                .advance(STEP_MS)
                .unwrap_or_else(|error| panic!("Town step {step}: {error:?}"));
            for agent in universe.agents().values() {
                let AgentKind::Citizen(citizen) = &agent.kind;
                assert!(citizen.hunger().is_finite() && citizen.tiredness().is_finite());
                assert!(citizen.coins() >= 0);
            }
        }
        println!(
            "Three-day town: {:?}, {} trades, {} orders",
            started.elapsed(),
            universe.market().trades().len(),
            universe.market().orders().count()
        );
        for agent in universe.agents().values() {
            let AgentKind::Citizen(citizen) = &agent.kind;
            println!(
                "{} hunger {} coins {} inventory {:?} targets {:?} demand {:?}",
                agent.name,
                citizen.hunger(),
                citizen.coins(),
                Good::ALL.map(|good| (good, citizen.units(good))),
                citizen.production_targets(),
                universe.market().requested(citizen.id())
            );
        }
        for order in universe.market().orders() {
            println!(
                "Order {:?} {} {}",
                order.good, order.units, order.quoted_price
            );
        }
        assert!(
            universe
                .market()
                .orders()
                .any(|order| order.good != Good::Berries)
        );
        assert!(
            universe
                .market()
                .trades()
                .iter()
                .any(|trade| trade.good != Good::Berries)
        );
        assert!(
            universe
                .market()
                .trades()
                .iter()
                .any(|trade| matches!(trade.good, Good::Bread | Good::BerryPie))
        );
        assert!(source.market().orders().next().is_none());
        assert!(source.market().trades().is_empty());
    }

    #[test]
    fn initial_town_has_fourteen_separated_identified_places_and_agreed_starting_roles() {
        use learning_lord_simulation::{
            AgentKind, StartingRole, locations::Location, marketplace::Good,
        };
        for _ in 0..10 {
            let universe = new_universe().unwrap();
            let map = universe.map();
            let places: Vec<_> = map.places().values().collect();
            assert_eq!(places.len(), 14);
            assert_eq!(
                places
                    .iter()
                    .filter(|place| place.kind == Location::Home)
                    .count(),
                6
            );
            assert_eq!(
                places.iter().filter(|place| place.owner.is_none()).count(),
                3
            );
            for (index, place) in places.iter().enumerate() {
                assert_eq!(place.id.0.get_version_num(), 4);
                assert!(place.position.x.abs() <= 1000.0 && place.position.y.abs() <= 1000.0);
                for other in &places[index + 1..] {
                    assert!(place.position.distance(other.position) >= 200.0);
                }
                if let Some(owner) = place.owner {
                    assert!(universe.agents().contains_key(&owner));
                }
            }
            for (id, agent) in universe.agents() {
                let AgentKind::Citizen(citizen) = &agent.kind;
                assert_eq!(citizen.id(), *id);
                assert_eq!(citizen.id().0.get_version_num(), 4);
                assert_eq!(citizen.berries_units(), 310);
                assert_eq!(
                    citizen.coins(),
                    learning_lord_simulation::production::starting_coins(
                        citizen.starting_role().unwrap()
                    )
                );
                for good in Good::ALL.into_iter().filter(|good| *good != Good::Berries) {
                    let expected = learning_lord_simulation::production::starting_inputs(
                        citizen.starting_role().unwrap(),
                    )
                    .items()
                    .find(|(g, _)| *g == good)
                    .map_or(0, |(_, units)| units);
                    assert_eq!(citizen.units(good), expected);
                }
                assert_eq!(map.place(citizen.home()).unwrap().owner, Some(*id));
                assert_eq!(citizen.position(), map.position(citizen.home()));
                let (role, property) = match agent.name.as_str() {
                    "Ada" => (StartingRole::Farmer, Some(Location::Field)),
                    "Bram" => (StartingRole::Miller, Some(Location::Mill)),
                    "Cleo" => (StartingRole::Woodcutter, None),
                    "Dara" => (StartingRole::Baker, Some(Location::Bakery)),
                    "Eira" => (StartingRole::Weaver, Some(Location::Weavery)),
                    "Finn" => (StartingRole::Tailor, Some(Location::Tailory)),
                    _ => panic!("unexpected citizen"),
                };
                assert_eq!(citizen.starting_role(), Some(role));
                assert_eq!(
                    citizen.owned_properties().count(),
                    if property.is_some() { 2 } else { 1 }
                );
                if let Some(kind) = property {
                    assert!(citizen.owned_properties().any(|place| place.kind == kind));
                }
            }
            assert_eq!(universe.clone().map(), map);
        }
    }

    #[test]
    fn restart_replaces_universe_and_clears_error_speed_and_time_debt() {
        let universe = new_universe().unwrap().advance(STEP_MS).unwrap();
        let old_ids: Vec<_> = universe.agents().keys().copied().collect();
        let old_prices = universe.prices();
        let old_map = universe.map();
        let mut state = WorkerState {
            universe,
            planner: PlanningRuntime::default(),
            pacing: pacing(30),
            error: Some("old error".into()),
            generation: 0,
        };
        state.pacing.speed = 20;
        state.pacing.pending_simulated_ns = 123456789;
        assert!(state.apply(Command::Restart, Duration::from_secs(10)));
        assert_eq!(state.universe.current_time_ms(), START_TIME_MS);
        assert_eq!(state.universe.agents().len(), 6);
        assert!(!state.universe.agents().contains_key(&old_ids[0]));
        assert_eq!(state.universe.prices(), old_prices);
        assert_ne!(state.universe.map(), old_map);
        assert!(state.error.is_none());
        assert!(!state.pacing.running);
        assert_eq!(state.pacing.speed, 1);
        assert_eq!(state.pacing.pending_simulated_ns, 0);
        assert_eq!(state.generation, 1);
        state.apply(Command::SetRunning(true), Duration::from_secs(20));
        assert_eq!(
            state.pacing.take_update(Duration::from_secs(21)),
            Ok(Some(60_000))
        );
        let learning_lord_simulation::AgentKind::Citizen(citizen) =
            &state.universe.agents().values().next().unwrap().kind;
        assert_eq!(
            citizen.position(),
            state.universe.map().position(citizen.home())
        );
        assert_eq!(citizen.map(), state.universe.map());
        assert_eq!(citizen.hunger(), -50.0);
        assert_eq!(citizen.tiredness(), -66.6);
        assert_eq!(citizen.coins(), 300);
        assert!(citizen.active_plan().is_some());
    }

    #[test]
    fn update_cap_changes_frequency_without_changing_simulation_speed() {
        for (rate, interval_ns) in [(60, 16_666_667), (30, 33_333_334)] {
            let mut pacing = pacing(rate);
            assert_eq!(
                pacing.take_update(Duration::from_nanos(interval_ns - 1)),
                Ok(None)
            );
            let first = pacing
                .take_update(Duration::from_nanos(interval_ns))
                .unwrap()
                .unwrap();
            assert_eq!(first, 60_000 / rate as u64);
            assert_eq!(
                pacing.take_update(Duration::from_nanos(interval_ns)),
                Ok(None)
            );
            let later = pacing.take_update(Duration::from_secs(1)).unwrap().unwrap();
            assert_eq!(first + later, 60_000);
        }
    }

    #[test]
    fn slow_processing_produces_one_larger_jump() {
        let mut pacing = pacing(60);
        assert_eq!(
            pacing.take_update(Duration::from_millis(17)),
            Ok(Some(1_020))
        );
        assert_eq!(
            pacing.take_update(Duration::from_millis(117)),
            Ok(Some(6_000))
        );
        assert_eq!(pacing.take_update(Duration::from_millis(117)), Ok(None));
    }

    #[test]
    fn fractional_milliseconds_are_preserved() {
        let mut pacing = pacing(60);
        let mut total = 0;
        for index in 1..=1000 {
            total += pacing
                .take_update(Duration::from_nanos(index * 17_123_456))
                .unwrap()
                .unwrap();
        }
        assert_eq!(total, 1_027_407);
        assert_eq!(pacing.pending_simulated_ns, 360_000);
    }

    #[test]
    fn speeds_scale_elapsed_time_and_changes_preserve_the_previous_rate() {
        for speed in SPEEDS {
            let mut pacing = pacing(60);
            pacing.set_speed(speed, Duration::ZERO);
            assert_eq!(
                pacing.take_update(Duration::from_secs(1)),
                Ok(Some(60_000 * speed as u64))
            );
        }
        let mut pacing = pacing(60);
        pacing.set_speed(2, Duration::from_millis(250));
        assert_eq!(
            pacing.take_update(Duration::from_secs(1)),
            Ok(Some(105_000))
        );
    }

    #[test]
    fn paused_time_is_excluded_even_when_commands_wait_for_a_busy_worker() {
        let mut pacing = pacing(60);
        assert_eq!(
            pacing.take_update(Duration::from_millis(17)),
            Ok(Some(1_020))
        );
        pacing.set_running(false, Duration::from_millis(50));
        assert_eq!(pacing.take_update(Duration::from_secs(9)), Ok(None));
        pacing.set_running(true, Duration::from_secs(10));
        assert_eq!(
            pacing.take_update(Duration::from_millis(10_100)),
            Ok(Some(7_980))
        );
    }

    #[test]
    fn late_command_timestamps_do_not_recount_elapsed_time() {
        let mut pacing = pacing(60);
        assert_eq!(
            pacing.take_update(Duration::from_millis(20)),
            Ok(Some(1_200))
        );
        pacing.set_speed(2, Duration::from_millis(19));
        assert_eq!(
            pacing.take_update(Duration::from_millis(40)),
            Ok(Some(2_400))
        );
    }

    #[test]
    fn paused_steps_are_exact_and_ignore_speed_while_running_steps_are_rejected() {
        let (universe, id) =
            Universe::with_map(learning_lord_simulation::locations::Map::default())
                .with_citizen("Ada", Citizen::new(0.0).unwrap())
                .unwrap();
        let universe = universe.start_planning(id).unwrap();
        let original = universe.clone();
        let mut state = WorkerState {
            universe,
            planner: PlanningRuntime::default(),
            pacing: Pacing::new(NonZeroU32::new(60).unwrap()),
            error: None,
            generation: 0,
        };
        state.apply(Command::SetSpeed(20), Duration::ZERO);
        state.apply(Command::Step, Duration::from_secs(10));
        assert_eq!(state.universe.current_time_ms(), STEP_MS);
        assert_eq!(
            state.universe,
            original
                .advance_with_planner(&mut PlanningRuntime::default(), STEP_MS, || false)
                .unwrap()
                .unwrap()
        );
        state.apply(Command::SetRunning(true), Duration::from_secs(11));
        state.apply(Command::Step, Duration::from_secs(12));
        assert_eq!(state.universe.current_time_ms(), STEP_MS);
        assert_eq!(original.current_time_ms(), 0);
    }

    #[test]
    fn next_day_advances_normally_to_the_following_calendar_day_at_four() {
        const HOUR_MS: u64 = 60 * 60 * 1000;
        const DAY_MS: u64 = 24 * HOUR_MS;
        for start in [3 * HOUR_MS, 4 * HOUR_MS, 6 * HOUR_MS, DAY_MS - 60_000] {
            let (universe, id) =
                Universe::with_map(learning_lord_simulation::locations::Map::default())
                    .with_citizen("Ada", Citizen::new(0.0).unwrap())
                    .unwrap();
            let universe = universe.start_planning(id).unwrap().advance(start).unwrap();
            let expected = universe
                .advance_with_planner(
                    &mut PlanningRuntime::default(),
                    DAY_MS + 4 * HOUR_MS - start,
                    || false,
                )
                .unwrap()
                .unwrap();
            let mut state = WorkerState {
                universe,
                planner: PlanningRuntime::default(),
                pacing: Pacing::new(NonZeroU32::new(60).unwrap()),
                error: None,
                generation: 0,
            };
            state.apply(Command::SetSpeed(20), Duration::ZERO);
            state.apply(Command::NextDay, Duration::from_secs(10));
            assert!(state.error.is_none());
            assert_eq!(state.universe.current_time_ms(), DAY_MS + 4 * HOUR_MS);
            assert_eq!(state.universe, expected);
            assert_eq!(state.pacing.speed, 20);
            assert!(!state.pacing.running);
            state.apply(Command::SetRunning(true), Duration::from_secs(11));
            state.apply(Command::NextDay, Duration::from_secs(12));
            assert_eq!(state.universe, expected);
        }
    }

    #[test]
    fn next_day_uses_the_clock_after_preceding_commands() {
        const DAY_MS: u64 = 24 * 60 * 60 * 1000;
        let universe = Universe::with_map(learning_lord_simulation::locations::Map::default())
            .advance(DAY_MS - 60_000)
            .unwrap();
        let worker = SimulationWorker::spawn(universe, NonZeroU32::new(60).unwrap());
        worker.send(Command::Step).unwrap();
        worker.send(Command::NextDay).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = worker.snapshot();
            assert!(snapshot.error.is_none());
            if snapshot.universe.current_time_ms() == 2 * DAY_MS + 4 * 60 * 60 * 1000 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "worker did not advance to the next day"
            );
            thread::yield_now();
        }
    }

    #[test]
    fn worker_processes_ordered_commands_publishes_latest_state_and_shuts_down() {
        let original = Universe::with_map(learning_lord_simulation::locations::Map::default());
        let worker = SimulationWorker::spawn(original.clone(), NonZeroU32::new(30).unwrap());
        worker.send(Command::SetSpeed(20)).unwrap();
        worker.send(Command::Step).unwrap();
        worker.send(Command::Step).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = worker.snapshot();
            assert!(snapshot.error.is_none());
            if snapshot.universe.current_time_ms() == 2 * STEP_MS {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "worker did not process the manual steps"
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(original.current_time_ms(), 0);
        let commands = worker.commands.clone();
        drop(worker);
        assert!(
            commands
                .send(TimedCommand {
                    command: Command::Step,
                    at: Instant::now()
                })
                .is_err()
        );
    }

    #[test]
    fn overflow_pauses_the_worker_and_preserves_the_last_snapshot() {
        let universe = Universe::with_map(learning_lord_simulation::locations::Map::default())
            .advance(u64::MAX)
            .unwrap();
        let mut state = WorkerState {
            universe: universe.clone(),
            planner: PlanningRuntime::default(),
            pacing: pacing(60),
            error: None,
            generation: 0,
        };
        state.advance(1);
        assert_eq!(state.universe, universe);
        assert!(!state.pacing.running);
        assert!(state.error.is_some());
        assert!(state.apply(Command::SetRunning(true), Duration::from_secs(1)));
        assert!(!state.pacing.running);
        assert!(!state.apply(Command::Shutdown, Duration::from_secs(1)));
        let mut pacing = pacing(60);
        assert_eq!(
            pacing.take_update(Duration::from_secs(u64::MAX)),
            Err(SimulationError::TimeOverflow)
        );
    }
}
