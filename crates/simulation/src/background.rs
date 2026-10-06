use crate::{
    AgentId, Citizen, SimulationError,
    planning::{self, ActivePlan, Plan, PlanningRequest},
};
use std::collections::HashMap;

pub(crate) const LEAD_MS: u64 = 30 * 60 * 1000;

struct Pending {
    plan: Plan,
    boundary_ms: u64,
    boundary_action_index: usize,
    request: PlanningRequest,
}

/// Owns speculative work for one authoritative universe lineage.
/// Reset before switching branches; cloned snapshots carry no task handles.
#[derive(Default)]
pub struct PlanningRuntime {
    pending: HashMap<AgentId, Pending>,
    histories: HashMap<AgentId, crate::history::PlanningHistory>,
    samples: Vec<(AgentId, u64, f64, f64, u64)>,
}

impl PlanningRuntime {
    pub fn reset(&mut self) {
        self.abort_advance();
        self.histories.clear();
    }

    pub(crate) fn abort_advance(&mut self) {
        for pending in self.pending.values() {
            pending.request.cancel();
        }
        self.pending.clear();
        self.samples.clear();
    }

    pub fn history(&self, id: AgentId) -> Option<&crate::history::PlanningHistory> {
        self.histories.get(&id)
    }

    pub(crate) fn finish_advance(&mut self, source: &crate::Universe, now: u64) {
        for id in source.agents().keys() {
            self.histories
                .entry(*id)
                .or_insert_with(|| crate::history::PlanningHistory::new(source.current_time_ms()));
        }
        for (id, time, request_ms, wait_ms, requests) in self.samples.drain(..) {
            let history = self.histories.get_mut(&id).unwrap();
            history.advance(time);
            history.current.request_ms += request_ms;
            history.current.wait_ms += wait_ms;
            history.current.requests += requests;
        }
        for history in self.histories.values_mut() {
            history.advance(now);
        }
    }

    pub(crate) fn planning_event(
        &mut self,
        citizen: &Citizen,
        now: u64,
    ) -> Result<Option<u64>, SimulationError> {
        let Some(execution) = citizen.active_plan() else {
            return Ok(None);
        };
        if self
            .pending
            .get(&citizen.id())
            .is_some_and(|pending| pending.plan != *execution.plan() || pending.boundary_ms < now)
        {
            self.cancel(citizen.id());
        }
        if self.pending.contains_key(&citizen.id()) {
            return Ok(None);
        }
        let Some(action) = citizen.active_action() else {
            return Ok(None);
        };
        let remaining = action.remaining_ms();
        if !execution.finishes_commitment(remaining) {
            return Ok(None);
        }
        if remaining > LEAD_MS {
            return Ok(Some(remaining - LEAD_MS));
        }
        let Ok(Some((boundary_ms, projected, boundary_action_index))) = project(citizen, now)
        else {
            return Ok(None);
        };
        let Ok(request) = planning::request(&projected) else {
            return Ok(None);
        };
        self.pending.insert(
            citizen.id(),
            Pending {
                plan: execution.plan().clone(),
                boundary_ms,
                boundary_action_index,
                request,
            },
        );
        Ok(None)
    }

    pub(crate) fn cancel(&mut self, id: AgentId) {
        if let Some(pending) = self.pending.remove(&id) {
            pending.request.cancel();
        }
    }

    pub(crate) fn at_boundary(
        &mut self,
        citizen: &Citizen,
        execution: &mut ActivePlan,
        now: u64,
        should_cancel: &mut impl FnMut() -> bool,
    ) -> Result<Option<Citizen>, SimulationError> {
        let mut actual = citizen.clone();
        execution.prepare_replan(&mut actual)?;
        let pending = self.pending.remove(&citizen.id());
        let mut speculative = pending.as_ref().is_some_and(|pending| {
            pending.boundary_ms == now
                && pending.boundary_action_index == execution.action_index()
                && pending.plan == *execution.plan()
        });
        let mut request = match pending {
            Some(pending)
                if pending.boundary_ms == now
                    && pending.boundary_action_index == execution.action_index()
                    && pending.plan == *execution.plan() =>
            {
                pending.request
            }
            Some(pending) => {
                pending.request.cancel();
                planning::request(&actual)?
            }
            None => planning::request(&actual)?,
        };
        let mut wait_ms = 0.0;
        let mut request_ms = 0.0;
        let mut requests = 0;
        loop {
            if should_cancel() {
                request.cancel();
                self.abort_advance();
                return Ok(None);
            }
            if let Some(result) = request.try_result() {
                request_ms += request.elapsed().as_secs_f64() * 1000.0;
                requests += 1;
                if let Ok(plan) = result.as_ref()
                    && (!speculative || ActivePlan::first_action_valid(&actual, plan))
                    && let Ok(started) = execution.adopt(actual.clone(), plan.clone())
                {
                    self.samples
                        .push((citizen.id(), now, request_ms, wait_ms, requests));
                    return Ok(Some(started));
                }
                if !speculative {
                    let plan = result?;
                    let started = execution.adopt(actual, plan)?;
                    self.samples
                        .push((citizen.id(), now, request_ms, wait_ms, requests));
                    return Ok(Some(started));
                }
                request = planning::request(&actual)?;
                speculative = false;
            }
            let waiting = std::time::Instant::now();
            std::thread::park_timeout(std::time::Duration::from_millis(2));
            wait_ms += waiting.elapsed().as_secs_f64() * 1000.0;
        }
    }
}

impl Drop for PlanningRuntime {
    fn drop(&mut self) {
        self.reset();
    }
}

pub(crate) fn project(
    source: &Citizen,
    now: u64,
) -> Result<Option<(u64, Citizen, usize)>, SimulationError> {
    let Some(mut execution) = source.active_plan().cloned() else {
        return Ok(None);
    };
    let Some(action) = source.active_action() else {
        return Ok(None);
    };
    let remaining = action.remaining_ms();
    if !execution.finishes_commitment(remaining) {
        return Ok(None);
    }
    let mut citizen = source.clone();
    citizen.active_plan = None;
    citizen = citizen.advance_predicted(remaining)?;
    let boundary = now
        .checked_add(remaining)
        .ok_or(SimulationError::TimeOverflow)?;
    execution.record_elapsed(remaining)?;
    execution.finish_action();
    citizen.refresh_production_targets(false)?;
    execution.prepare_replan(&mut citizen)?;
    Ok(Some((boundary, citizen, execution.action_index())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ACTION_DURATION_MS, AgentKind, CitizenAction, FORAGE_AVERAGE_GRAMS, SLEEP_DURATION_MS,
        Universe, locations::Map,
    };
    use planning::executing_fixture;
    const HOUR: u64 = 3_600_000;

    fn universe(citizen: Citizen) -> (Universe, AgentId) {
        Universe::with_map(citizen.map.clone())
            .with_citizen("Ada", citizen)
            .unwrap()
    }

    #[test]
    fn sleep_window_is_before_waking_instead_of_commitment_expiry() {
        let source = executing_fixture(
            &Citizen::with_needs(-50.0, 100.0).unwrap(),
            vec![CitizenAction::Sleep],
        );
        let (boundary, projected, _) = project(&source, 0).unwrap().unwrap();
        assert_eq!(boundary, SLEEP_DURATION_MS);
        assert!(projected.active_action().is_none() && projected.active_plan().is_none());
        let mut runtime = PlanningRuntime::default();
        assert_eq!(
            runtime.planning_event(&source, 0).unwrap(),
            Some(SLEEP_DURATION_MS - LEAD_MS)
        );
        assert!(runtime.pending.is_empty());
        let source = source.advance(7 * HOUR).unwrap();
        assert_eq!(
            runtime.planning_event(&source, 7 * HOUR).unwrap(),
            Some(LEAD_MS)
        );
        assert!(runtime.pending.is_empty());
    }

    #[test]
    fn short_exhaustion_submits_immediately_once_and_expected_forage_preserves_rng() {
        let citizen = Citizen::with_needs(-50.0, -50.0).unwrap();
        let source = executing_fixture(
            &citizen,
            vec![CitizenAction::Produce(crate::production::Recipe::Forage)],
        );
        let saved = source.clone();
        let (boundary, projected, _) = project(&source, 0).unwrap().unwrap();
        assert_eq!(boundary, ACTION_DURATION_MS);
        assert_eq!(
            projected.berries_units(),
            citizen.berries_units() + FORAGE_AVERAGE_GRAMS
        );
        assert_eq!(projected.forage_rng, citizen.forage_rng);
        let mut runtime = PlanningRuntime::default();
        assert_eq!(runtime.planning_event(&source, 0).unwrap(), None);
        assert_eq!(runtime.pending.len(), 1);
        runtime.planning_event(&source, 1).unwrap();
        assert_eq!(runtime.pending.len(), 1);
        assert_eq!(source, saved);
    }

    #[test]
    fn only_current_known_final_action_can_submit_before_a_boundary() {
        let citizen = Citizen::with_needs(-50.0, -50.0).unwrap();
        let source = executing_fixture(
            &citizen,
            vec![CitizenAction::Produce(crate::production::Recipe::Forage); 10],
        );
        let mut runtime = PlanningRuntime::default();
        assert!(project(&source, 0).unwrap().is_none());
        assert_eq!(runtime.planning_event(&source, 0).unwrap(), None);
        assert!(runtime.pending.is_empty());
        let final_action = source.advance(3 * ACTION_DURATION_MS).unwrap();
        assert_eq!(final_action.active_plan().unwrap().action_index(), 3);
        runtime
            .planning_event(&final_action, 3 * ACTION_DURATION_MS)
            .unwrap();
        assert_eq!(runtime.pending.len(), 1);
        assert_eq!(
            runtime.pending[&citizen.id()].boundary_ms,
            planning::COMMITMENT_MS
        );
    }

    #[test]
    fn five_minute_final_action_submits_at_its_start_without_advancing_time() {
        let citizen = Citizen::with_needs(-50.0, -50.0)
            .unwrap()
            .with_good(crate::marketplace::Good::FlaxGarment, 1)
            .unwrap();
        let source = executing_fixture(&citizen, vec![CitizenAction::EquipClothing]);
        let (world, id) = universe(source);
        let mut runtime = PlanningRuntime::default();
        let result = world
            .advance_with_planner(&mut runtime, 0, || false)
            .unwrap()
            .unwrap();
        assert_eq!(result, world);
        assert_eq!(runtime.pending.len(), 1);
        assert_eq!(
            runtime.pending[&id].boundary_ms,
            crate::EQUIP_CLOTHING_DURATION_MS
        );
    }

    #[test]
    fn uncertain_forage_shortage_does_not_predict_a_final_action() {
        let citizen = Citizen::with_needs(-50.0, -50.0)
            .unwrap()
            .with_berries(50)
            .unwrap();
        let source = executing_fixture(
            &citizen,
            vec![
                CitizenAction::Produce(crate::production::Recipe::Forage),
                CitizenAction::Eat,
                CitizenAction::Sleep,
            ],
        );
        let mut runtime = PlanningRuntime::default();
        assert!(project(&source, 0).unwrap().is_none());
        runtime.planning_event(&source, 0).unwrap();
        assert!(runtime.pending.is_empty());
    }

    #[test]
    fn day_jump_crosses_window_and_adopts_without_overwriting_actual_state() {
        let citizen = Citizen::with_needs(-50.0, 100.0)
            .unwrap()
            .with_berries(310)
            .unwrap();
        let source = executing_fixture(&citizen, vec![CitizenAction::Sleep]);
        let (world, id) = universe(source);
        let saved = world.clone();
        let mut runtime = PlanningRuntime::default();
        let at_window = world
            .advance_with_planner(&mut runtime, SLEEP_DURATION_MS - LEAD_MS, || false)
            .unwrap()
            .unwrap();
        assert_eq!(runtime.pending.len(), 1);
        let next = at_window
            .advance_with_planner(&mut runtime, LEAD_MS, || false)
            .unwrap()
            .unwrap();
        assert_eq!(next.current_time_ms(), SLEEP_DURATION_MS);
        let AgentKind::Citizen(actual) = &next.agents()[&id].kind;
        assert_eq!(actual.active_plan().unwrap().elapsed_ms(), 0);
        assert_eq!(actual.forage_rng, citizen.forage_rng);
        assert_eq!(actual.coins(), citizen.coins());
        assert_eq!(world, saved);
        let day = world
            .advance_with_planner(&mut PlanningRuntime::default(), 24 * HOUR, || false)
            .unwrap()
            .unwrap();
        assert_eq!(day.current_time_ms(), 24 * HOUR);
    }

    #[test]
    fn stale_market_result_adopts_only_plan_and_preserves_actual_resources() {
        let citizen = Citizen::with_needs(-50.0, 100.0)
            .unwrap()
            .with_berries(310)
            .unwrap();
        let source = executing_fixture(&citizen, vec![CitizenAction::Sleep]);
        let mut execution = source.active_plan().unwrap().clone();
        let (_, projected, _) = project(&source, 0).unwrap().unwrap();
        let mut actual = projected.clone();
        actual.coins += 17;
        actual.prices = actual
            .prices
            .with_price(crate::marketplace::Good::Berries, 2.0)
            .unwrap();
        actual.market.prices = actual.prices;
        let mut forecast = projected;
        forecast.coins += 999;
        let request = planning::request(&forecast).unwrap();
        let mut runtime = PlanningRuntime::default();
        runtime.pending.insert(
            citizen.id(),
            Pending {
                plan: execution.plan().clone(),
                boundary_ms: SLEEP_DURATION_MS,
                boundary_action_index: 1,
                request,
            },
        );
        execution.finish_action();
        let result = runtime
            .at_boundary(&actual, &mut execution, SLEEP_DURATION_MS, &mut || false)
            .unwrap()
            .unwrap();
        let expected = actual.start_action(execution.plan().actions()[0]).unwrap();
        assert_eq!(result, expected);
        assert_eq!(result.coins(), actual.coins());
        assert_eq!(result.forage_rng, actual.forage_rng);
        assert_ne!(execution.plan().decision().unwrap().prices, actual.prices);
    }

    #[test]
    fn missing_first_meal_uses_fresh_actual_state_and_reset_discards_old_request() {
        let citizen = Citizen::with_needs(80.0, -50.0).unwrap();
        let source = executing_fixture(
            &citizen,
            vec![CitizenAction::Produce(crate::production::Recipe::Forage)],
        );
        let mut execution = source.active_plan().unwrap().clone();
        let forecast = citizen.with_berries(310).unwrap();
        let expected_forecast = planning::plan(&forecast).unwrap();
        assert_eq!(expected_forecast.actions()[0], CitizenAction::Eat);
        let mut runtime = PlanningRuntime::default();
        runtime.pending.insert(
            citizen.id(),
            Pending {
                plan: execution.plan().clone(),
                boundary_ms: ACTION_DURATION_MS,
                boundary_action_index: 1,
                request: planning::request(&forecast).unwrap(),
            },
        );
        execution.finish_action();
        let fresh = planning::plan(&citizen).unwrap();
        let result = runtime
            .at_boundary(&citizen, &mut execution, ACTION_DURATION_MS, &mut || false)
            .unwrap()
            .unwrap();
        assert_eq!(execution.plan(), &fresh);
        assert_eq!(result, citizen.start_action(fresh.actions()[0]).unwrap());
        runtime.planning_event(&source, 0).unwrap();
        assert_eq!(runtime.pending.len(), 1);
        runtime.reset();
        assert!(runtime.pending.is_empty());
    }

    #[test]
    fn interruption_and_overflow_keep_source_snapshot_and_cancel_pending_work() {
        let citizen = Citizen::with_needs(-50.0, -50.0).unwrap();
        let (world, _) = universe(executing_fixture(
            &citizen,
            vec![CitizenAction::Produce(crate::production::Recipe::Forage)],
        ));
        let saved = world.clone();
        let mut runtime = PlanningRuntime::default();
        runtime
            .planning_event(
                match &world.agents().values().next().unwrap().kind {
                    AgentKind::Citizen(c) => c,
                },
                0,
            )
            .unwrap();
        assert_eq!(runtime.pending.len(), 1);
        assert!(
            world
                .advance_with_planner(&mut runtime, HOUR, || true)
                .unwrap()
                .is_none()
        );
        assert!(runtime.pending.is_empty());
        assert_eq!(world, saved);
        let world = Universe::with_map(Map::default())
            .advance(u64::MAX)
            .unwrap();
        assert_eq!(
            world.advance_with_planner(&mut runtime, 1, || false),
            Err(SimulationError::TimeOverflow)
        );
    }
    #[test]
    fn not_ready_boundary_wait_can_cancel_without_advancing_the_source_snapshot() {
        let citizen = Citizen::with_needs(-50.0, 100.0).unwrap();
        let source = executing_fixture(&citizen, vec![CitizenAction::Sleep]);
        let (world, id) = universe(source.clone());
        let saved = world.clone();
        let (_, projected, _) = project(&source, 0).unwrap().unwrap();
        let (started, received) = std::sync::mpsc::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let request = planning::gated_request_fixture(&projected, started, gate);
        received
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let mut runtime = PlanningRuntime::default();
        runtime.pending.insert(
            id,
            Pending {
                plan: source.active_plan().unwrap().plan().clone(),
                boundary_ms: SLEEP_DURATION_MS,
                boundary_action_index: 1,
                request,
            },
        );
        let mut polls = 0;
        let result = world
            .advance_with_planner(&mut runtime, SLEEP_DURATION_MS, || {
                polls += 1;
                polls == 5
            })
            .unwrap();
        assert!(result.is_none());
        assert_eq!(polls, 5);
        assert_eq!(world, saved);
        assert!(runtime.pending.is_empty());
        release.send(()).unwrap();
    }

    #[test]
    fn cancelled_and_failed_advances_preserve_published_telemetry() {
        let (world, id) = universe(Citizen::new(0.0).unwrap());
        let mut runtime = PlanningRuntime::default();
        runtime.samples.push((id, 0, 12.0, 3.0, 1));
        runtime.finish_advance(&world, 0);
        let saved = runtime.history(id).unwrap().clone();
        runtime.samples.push((id, 0, 99.0, 99.0, 1));
        assert!(
            world
                .advance_with_planner(&mut runtime, 1, || true)
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.history(id), Some(&saved));
        assert!(runtime.samples.is_empty());
        let empty = Universe::with_map(Map::default())
            .advance(u64::MAX)
            .unwrap();
        assert_eq!(
            empty.advance_with_planner(&mut runtime, 1, || false),
            Err(SimulationError::TimeOverflow)
        );
        assert_eq!(runtime.history(id), Some(&saved));
        runtime.reset();
        assert!(runtime.history(id).is_none());
    }

    #[test]
    fn telemetry_closes_daily_and_attributes_requests_to_adoption_day() {
        let (world, id) = universe(Citizen::new(0.0).unwrap());
        let mut runtime = PlanningRuntime::default();
        let boundary = crate::marketplace::UPDATE_TIME_MS;
        runtime.samples.push((id, boundary, 12.0, 3.0, 2));
        runtime.finish_advance(&world, boundary);
        let history = runtime.history(id).unwrap();
        assert_eq!(history.completed.len(), 1);
        assert_eq!(history.completed[0].requests, 0);
        assert_eq!(history.current.start_ms, boundary);
        assert_eq!(history.current.requests, 2);
        assert_eq!(history.current.request_ms, 12.0);
        assert_eq!(history.current.wait_ms, 3.0);
        runtime.finish_advance(&world, boundary + 31 * crate::marketplace::DAY_MS);
        assert_eq!(runtime.history(id).unwrap().completed.len(), 30);
    }

    #[test]
    #[ignore = "manual spatial town wall-clock benchmark"]
    fn larger_spatial_town_benchmark() {
        use crate::{
            StartingRole,
            locations::{Location, Position},
        };
        use rand::SeedableRng;
        use std::time::{Duration, Instant};
        let count: usize = std::env::var("TOWN_BENCH_CITIZENS")
            .unwrap_or("24".into())
            .parse()
            .unwrap();
        assert!(matches!(count, 6 | 24));
        let days: u64 = std::env::var("TOWN_BENCH_DAYS")
            .unwrap_or("3".into())
            .parse()
            .unwrap();
        let cap = Duration::from_secs(
            std::env::var("TOWN_BENCH_CAP_SECONDS")
                .unwrap_or("120".into())
                .parse()
                .unwrap(),
        );
        let started = Instant::now();
        let position = |slot: usize| Position {
            x: -750.0 + (slot % 7) as f64 * 250.0,
            y: -750.0 + (slot / 7) as f64 * 250.0,
        };
        let mut map = Map::new(position(0), position(1), position(2)).unwrap();
        let roles = [
            StartingRole::Farmer,
            StartingRole::Miller,
            StartingRole::Woodcutter,
            StartingRole::Baker,
            StartingRole::Weaver,
            StartingRole::Tailor,
        ];
        let properties = [
            Some(Location::Field),
            Some(Location::Mill),
            None,
            Some(Location::Bakery),
            Some(Location::Weavery),
            Some(Location::Tailory),
        ];
        let mut citizens = Vec::new();
        let mut slot = 3;
        for index in 0..count {
            let role = roles[index % 6];
            let mut citizen = Citizen::with_needs(-50.0, -66.6)
                .unwrap()
                .with_berries(310)
                .unwrap()
                .with_garment_condition(Some(0.5))
                .unwrap()
                .with_starting_role(role)
                .with_coins(crate::production::starting_coins(role))
                .unwrap();
            citizen.id = AgentId(uuid::Uuid::from_u128(index as u128 + 1));
            citizen.forage_rng = rand::rngs::SmallRng::seed_from_u64(42 + index as u64);
            for (good, units) in crate::production::starting_inputs(role).items() {
                citizen = citizen.with_good(good, units).unwrap();
            }
            let (next, home) = map
                .with_place(
                    Location::Home,
                    position(slot),
                    Some(citizen.id),
                    format!("Citizen {}'s home", index + 1),
                )
                .unwrap();
            map = next;
            citizen.home = home;
            citizen.position = position(slot);
            slot += 1;
            if let Some(kind) = properties[index % 6] {
                map = map
                    .with_place(
                        kind,
                        position(slot),
                        Some(citizen.id),
                        format!("Citizen {index}'s {}", kind.name()),
                    )
                    .unwrap()
                    .0;
                slot += 1;
            }
            citizens.push(citizen);
        }
        let mut world = Universe::with_map(map.clone()).advance(6 * HOUR).unwrap();
        for mut citizen in citizens {
            citizen.map = map.clone();
            world = world
                .with_citizen(format!("Citizen {}", citizen.id.0.as_u128()), citizen)
                .unwrap()
                .0;
        }
        let ids: Vec<_> = (0..count)
            .map(|index| AgentId(uuid::Uuid::from_u128(index as u128 + 1)))
            .collect();
        for id in ids {
            world = world.start_planning(id).unwrap();
        }
        assert_eq!(world.map().places().len(), 3 + count + count / 6 * 5);
        let mut home_slot = 3;
        for index in 0..count {
            let id = AgentId(uuid::Uuid::from_u128(index as u128 + 1));
            let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
            assert_eq!(world.map().position(citizen.home()), position(home_slot));
            assert_eq!(citizen.position(), position(home_slot));
            home_slot += 1 + usize::from(properties[index % 6].is_some());
        }
        let available = std::thread::available_parallelism().unwrap().get();
        println!(
            "citizens={count} hardware_threads={available} planning_workers={} startup_ms={:.3}",
            available.saturating_sub(4).max(2),
            started.elapsed().as_secs_f64() * 1000.0
        );
        let mut runtime = PlanningRuntime::default();
        for day in 1..=days {
            let day_start = Instant::now();
            let next = world
                .advance_with_planner(&mut runtime, 24 * HOUR, || day_start.elapsed() >= cap)
                .unwrap();
            let Some(next) = next else {
                println!(
                    "citizens={count} day={day} cancelled_after_ms={:.3} source_hour={}",
                    day_start.elapsed().as_secs_f64() * 1000.0,
                    world.current_time_ms() / HOUR
                );
                break;
            };
            world = next;
            let mut hunger_max = f64::NEG_INFINITY;
            let mut wealth = 0;
            for agent in world.agents().values() {
                let AgentKind::Citizen(citizen) = &agent.kind;
                assert!(citizen.hunger().is_finite() && citizen.tiredness().is_finite());
                assert!(citizen.coins() >= 0);
                hunger_max = hunger_max.max(citizen.hunger());
                wealth += citizen.coins();
            }
            let private_trades = world
                .market()
                .trades()
                .iter()
                .filter(|trade| {
                    let AgentKind::Citizen(seller) = &world.agents()[&trade.seller].kind;
                    world
                        .map()
                        .place(seller.selling_place())
                        .unwrap()
                        .owner
                        .is_some()
                })
                .count();
            println!(
                "citizens={count} day={day} elapsed_ms={:.3} trades={} orders={} private_trades={private_trades} max_hunger={hunger_max:.3} coins={wealth}",
                day_start.elapsed().as_secs_f64() * 1000.0,
                world.market().trades().len(),
                world.market().orders().count()
            );
        }
    }
    #[test]
    #[ignore = "manual wall-clock benchmark"]
    fn fixed_town_day_benchmark() {
        use crate::{StartingRole, locations::Location};
        use rand::SeedableRng;
        use std::time::Instant;
        let roles = [
            ("Ada", StartingRole::Farmer, Some(Location::Field)),
            ("Bram", StartingRole::Miller, Some(Location::Mill)),
            ("Cleo", StartingRole::Woodcutter, None),
            ("Dara", StartingRole::Baker, Some(Location::Bakery)),
            ("Eira", StartingRole::Weaver, Some(Location::Weavery)),
            ("Finn", StartingRole::Tailor, Some(Location::Tailory)),
        ];
        let mut world = Universe::with_map(Map::default())
            .advance(6 * HOUR)
            .unwrap();
        let mut ids = Vec::new();
        for (index, (name, role, property)) in roles.into_iter().enumerate() {
            let mut citizen = Citizen::with_needs(-50.0, -66.6)
                .unwrap()
                .with_berries(310)
                .unwrap()
                .with_garment_condition(Some(0.5))
                .unwrap()
                .with_starting_role(role)
                .with_coins(crate::production::starting_coins(role))
                .unwrap();
            citizen.id = AgentId(uuid::Uuid::from_u128(index as u128 + 1));
            citizen.forage_rng = rand::rngs::SmallRng::seed_from_u64(42 + index as u64);
            for (good, units) in crate::production::starting_inputs(role).items() {
                citizen = citizen.with_good(good, units).unwrap();
            }
            let (next, id) = world.with_citizen(name, citizen).unwrap();
            world = next;
            if let Some(property) = property {
                world = world.with_property(id, property).unwrap().0;
            }
            ids.push(id);
        }
        for id in ids {
            world = world.start_planning(id).unwrap();
        }
        let saved = world.clone();
        for round in 0..6 {
            for background in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let started = Instant::now();
                let result = if background {
                    world
                        .advance_with_planner(&mut PlanningRuntime::default(), 24 * HOUR, || false)
                        .unwrap()
                        .unwrap()
                } else {
                    world.advance(24 * HOUR).unwrap()
                };
                let elapsed = started.elapsed();
                assert_eq!(result.current_time_ms(), 30 * HOUR);
                for agent in result.agents().values() {
                    let AgentKind::Citizen(citizen) = &agent.kind;
                    assert!(citizen.hunger().is_finite() && citizen.tiredness().is_finite());
                    assert!(citizen.coins() >= 0);
                }
                println!(
                    "round={round} mode={} elapsed_ms={:.3}",
                    if background {
                        "background"
                    } else {
                        "synchronous"
                    },
                    elapsed.as_secs_f64() * 1000.0
                );
            }
        }
        assert_eq!(world, saved);
    }
}
