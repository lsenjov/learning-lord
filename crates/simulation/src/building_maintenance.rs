use crate::{
    AgentKind, SimulationError, Universe,
    condition::MAX_CONDITION_UNITS,
    construction::{
        ConstructionClient, ConstructionItem, ConstructionOutput, ConstructionProject,
        ConstructionStage, ContractId, HOUR_MS, MILESTONE_BASE_MS, ProjectId, RepairTarget,
    },
    locations::{BuildingId, Location, PlaceId, RoomId},
};
use rand::RngExt;

type RepairCredits<T> = Vec<(T, u128)>;

impl Universe {
    pub fn repair_project(
        &self,
        place: PlaceId,
        target: RepairTarget,
    ) -> Option<&ConstructionProject> {
        self.construction.projects.values().find(|project| {
            project.place == place && project.repair == Some(target) && !project.completed
        })
    }

    pub(crate) fn maintenance_boundary(&self) -> u64 {
        HOUR_MS - self.current_time_ms % HOUR_MS
    }

    pub(crate) fn construction_repair_credits(
        &self,
        elapsed: u64,
    ) -> (
        RepairCredits<RoomId>,
        RepairCredits<crate::locations::ObjectId>,
    ) {
        let mut progress = std::collections::HashMap::<ProjectId, u64>::new();
        for agent in self.agents.values() {
            let AgentKind::Citizen(worker) = &agent.kind;
            let Some(active) = worker.active_action() else {
                continue;
            };
            let crate::CitizenAction::Construct {
                contract,
                base_ms,
                duration_ms,
                ..
            } = active.action()
            else {
                continue;
            };
            let part = &self.construction.contracts[&contract];
            if self.construction.projects[&part.project].repair.is_none() {
                continue;
            }
            let before = active.duration_ms() - active.remaining_ms();
            let after = before + elapsed.min(active.remaining_ms());
            let delta = (u128::from(base_ms) * u128::from(after) / u128::from(duration_ms)
                - u128::from(base_ms) * u128::from(before) / u128::from(duration_ms))
                as u64;
            *progress.entry(part.project).or_default() += delta;
        }
        let mut rooms = Vec::new();
        let mut objects = Vec::new();
        for (id, delta) in progress {
            let project = &self.construction.projects[&id];
            let credit = project.repair_credit_through(project.completed_base_ms + delta)
                - project.repair_credit_through(project.completed_base_ms);
            match project.repair.unwrap() {
                RepairTarget::Room(room) => rooms.push((room, credit)),
                RepairTarget::Object(object) => objects.push((object, credit)),
            }
        }
        rooms.sort_by_key(|(id, _)| id.0);
        objects.sort_by_key(|(id, _)| id.0);
        (rooms, objects)
    }

    fn repair_owner(
        &self,
        place: PlaceId,
        target: RepairTarget,
    ) -> Result<ConstructionClient, SimulationError> {
        let site = self.map.place(place)?;
        let owner = match target {
            RepairTarget::Room(id) => Some(
                site.buildings
                    .iter()
                    .find(|building| building.rooms.iter().any(|room| room.id == id))
                    .ok_or(SimulationError::MissingProperty)?
                    .owner,
            ),
            RepairTarget::Object(id) => {
                if !site.objects().any(|object| object.id == id) {
                    return Err(SimulationError::MissingProperty);
                }
                site.object_owner(id)
            }
        };
        match owner {
            Some(id) => Ok(ConstructionClient::Account(self.economic_id(id)?)),
            None if matches!(site.kind, Location::Market | Location::Warehouse) => {
                Ok(ConstructionClient::Town)
            }
            None => Err(SimulationError::MissingProperty),
        }
    }

    fn repair_scope(
        &self,
        place: PlaceId,
        target: RepairTarget,
    ) -> Result<(ConstructionItem, Option<BuildingId>, Option<RoomId>, u128), SimulationError> {
        let site = self.map.place(place)?;
        match target {
            RepairTarget::Room(id) => {
                let building = site
                    .buildings
                    .iter()
                    .find(|building| building.rooms.iter().any(|room| room.id == id))
                    .ok_or(SimulationError::MissingProperty)?;
                let room = building.rooms.iter().find(|room| room.id == id).unwrap();
                Ok((
                    ConstructionItem::Room(room.size),
                    Some(building.id),
                    Some(id),
                    room.condition.damage_units(),
                ))
            }
            RepairTarget::Object(id) => {
                if matches!(site.kind, Location::Forest | Location::River) {
                    return Err(SimulationError::MissingProperty);
                }
                let object = site
                    .objects()
                    .find(|object| object.id == id)
                    .ok_or(SimulationError::MissingProperty)?;
                let item = ConstructionItem::from_object_kind(object.kind)
                    .ok_or(SimulationError::MissingProperty)?;
                let containing = site.buildings.iter().find_map(|building| {
                    building
                        .rooms
                        .iter()
                        .find(|room| room.objects.iter().any(|object| object.id == id))
                        .map(|room| (building.id, room.id))
                });
                Ok((
                    item,
                    containing.map(|pair| pair.0),
                    containing.map(|pair| pair.1),
                    object.condition.damage_units(),
                ))
            }
        }
    }

    fn reset_repair_scope(&mut self, id: ProjectId, damage: u128) {
        let item = self.construction.projects[&id].item;
        let total_base_ms =
            (damage * u128::from(item.base_hours() * HOUR_MS)).div_ceil(MAX_CONDITION_UNITS) as u64;
        let total_wood = (damage * u128::from(item.wood())).div_ceil(MAX_CONDITION_UNITS) as u64;
        let mut stages = Vec::new();
        let mut elapsed = 0;
        let wood_through = |base| {
            (u128::from(total_wood) * u128::from(base)).div_ceil(u128::from(total_base_ms)) as u64
        };
        while elapsed < total_base_ms {
            let end = (elapsed + MILESTONE_BASE_MS).min(total_base_ms);
            stages.push(ConstructionStage {
                next_contract: ContractId(
                    uuid::Builder::from_random_bytes(self.construction.rng.random()).into_uuid(),
                ),
                base_ms: end - elapsed,
                wood: wood_through(end) - wood_through(elapsed),
                completed_base_ms: 0,
                installed_wood: 0,
            });
            elapsed = end;
        }
        let project = self.construction.projects.get_mut(&id).unwrap();
        project.total_base_ms = total_base_ms;
        project.total_wood = total_wood;
        project.repair_damage_units = damage;
        project.stages = stages;
    }

    pub(crate) fn commission_repairs(&mut self) -> Result<(), SimulationError> {
        if !self.current_time_ms.is_multiple_of(HOUR_MS) {
            return Ok(());
        }
        let mut targets = Vec::new();
        for (place, site) in self.map.places() {
            for building in &site.buildings {
                for room in &building.rooms {
                    let target = RepairTarget::Room(room.id);
                    if room.condition.needs_repair()
                        || self.repair_project(*place, target).is_some()
                    {
                        targets.push((room.condition.damage_units(), *place, target));
                    }
                }
            }
            if !matches!(site.kind, Location::Forest | Location::River) {
                for object in site
                    .objects()
                    .filter(|object| ConstructionItem::from_object_kind(object.kind).is_some())
                {
                    let target = RepairTarget::Object(object.id);
                    if object.condition.needs_repair()
                        || self.repair_project(*place, target).is_some()
                    {
                        targets.push((object.condition.damage_units(), *place, target));
                    }
                }
            }
        }
        let target_key = |target| match target {
            RepairTarget::Room(id) => (0, id.0),
            RepairTarget::Object(id) => (1, id.0),
        };
        targets.sort_by_key(|(damage, place, target)| {
            (std::cmp::Reverse(*damage), place.0, target_key(*target))
        });
        for (_, place, target) in targets {
            let existing = self.repair_project(place, target).map(|project| project.id);
            if existing.is_some_and(|id| {
                self.construction
                    .contracts
                    .values()
                    .any(|contract| contract.project == id && contract.status.open())
            }) {
                continue;
            }
            let Ok(client) = self.repair_owner(place, target) else {
                continue;
            };
            let available = self.construction_client_budget(client)?;
            if available <= 0 {
                continue;
            }
            let (item, building, room, damage) = self.repair_scope(place, target)?;
            if damage == 0 {
                continue;
            }
            let id = if let Some(id) = existing {
                id
            } else {
                let id = ProjectId(
                    uuid::Builder::from_random_bytes(self.construction.rng.random()).into_uuid(),
                );
                self.construction.projects.insert(
                    id,
                    ConstructionProject {
                        id,
                        client,
                        place,
                        building,
                        room,
                        item,
                        completed_base_ms: 0,
                        installed_wood: 0,
                        completed: false,
                        output: match target {
                            RepairTarget::Room(id) => ConstructionOutput::Room(id),
                            RepairTarget::Object(id) => ConstructionOutput::Object(id),
                        },
                        stages: Vec::new(),
                        total_base_ms: 0,
                        total_wood: 0,
                        repair: Some(target),
                        repair_damage_units: 0,
                        escrow: 0,
                        funding_base_ms: 0,
                        funding_offer: 0,
                        allocated_base_ms: 0,
                        allocated_offer: 0,
                        scheduling: false,
                    },
                );
                id
            };
            let awarded = self
                .construction
                .contracts
                .values()
                .any(|contract| contract.project == id && contract.awarded_ms.is_some());
            if !awarded {
                self.construction.projects.get_mut(&id).unwrap().client = client;
                self.reset_repair_scope(id, damage);
            }
            let project = &self.construction.projects[&id];
            let offer = self
                .construction_estimate_for_client(
                    client,
                    place,
                    project.remaining_base_ms(),
                    project.missing_wood(),
                )?
                .min(available)
                .max(1);
            self.post_construction(id, offer)?;
        }
        self.refresh_guild_history(self.current_time_ms, false);
        Ok(())
    }

    pub(crate) fn validate_repair_project(
        &self,
        project: &ConstructionProject,
    ) -> Result<(), SimulationError> {
        let target = project.repair.ok_or(SimulationError::MissingProperty)?;
        let (item, building, room, _) = self.repair_scope(project.place, target)?;
        let client = match project.client {
            ConstructionClient::Account(id) => ConstructionClient::Account(self.economic_id(id)?),
            ConstructionClient::Town => ConstructionClient::Town,
        };
        if self.repair_owner(project.place, target)? != client
            || item != project.item
            || building != project.building
            || room != project.room
            || project.repair_damage_units == 0
            || project.repair_damage_units > MAX_CONDITION_UNITS
            || project.total_base_ms
                != (project.repair_damage_units * u128::from(item.base_hours() * HOUR_MS))
                    .div_ceil(MAX_CONDITION_UNITS) as u64
            || project.total_wood
                != (project.repair_damage_units * u128::from(item.wood()))
                    .div_ceil(MAX_CONDITION_UNITS) as u64
            || project.output
                != match target {
                    RepairTarget::Room(id) => ConstructionOutput::Room(id),
                    RepairTarget::Object(id) => ConstructionOutput::Object(id),
                }
        {
            return Err(SimulationError::MissingProperty);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Citizen, StartingRole,
        condition::Condition,
        construction::{BIDDING_MS, ContractStatus},
        locations::Map,
        marketplace::Good,
    };
    use uuid::Uuid;

    fn fixture(
        carpenter: bool,
    ) -> (
        Universe,
        crate::AgentId,
        Option<crate::AgentId>,
        PlaceId,
        RepairTarget,
    ) {
        let owner = Citizen::with_needs(-100.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_coins(1_000_000)
            .unwrap()
            .with_garment_condition(Some(1.0))
            .unwrap()
            .with_good(Good::BreadRoll, 10_000)
            .unwrap()
            .with_good(Good::Water, 100_000)
            .unwrap();
        let (mut world, owner) = Universe::with_map(Map::default())
            .with_citizen("Owner", owner)
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&owner].kind;
        let place = citizen.home;
        let target = RepairTarget::Room(world.map.place(place).unwrap().buildings[0].rooms[0].id);
        let worker = if carpenter {
            let (next, worker) = world
                .with_citizen(
                    "Carpenter",
                    Citizen::with_needs(-100.0, -100.0)
                        .unwrap()
                        .with_thirst(-100.0)
                        .unwrap()
                        .with_starting_role(StartingRole::Carpenter)
                        .with_coins(100_000)
                        .unwrap()
                        .with_garment_condition(Some(1.0))
                        .unwrap()
                        .with_good(Good::Wood, 10_000)
                        .unwrap()
                        .with_good(Good::BreadRoll, 10_000)
                        .unwrap()
                        .with_good(Good::Water, 100_000)
                        .unwrap(),
                )
                .unwrap();
            world = next;
            Some(worker)
        } else {
            None
        };
        (world, owner, worker, place, target)
    }

    fn damage(world: &mut Universe, place: PlaceId, target: RepairTarget, units: u128) {
        let condition = Condition::from_units(MAX_CONDITION_UNITS - units).unwrap();
        let site = world.map.places_mut().get_mut(&place).unwrap();
        match target {
            RepairTarget::Room(id) => {
                site.buildings
                    .iter_mut()
                    .flat_map(|building| &mut building.rooms)
                    .find(|room| room.id == id)
                    .unwrap()
                    .condition = condition
            }
            RepairTarget::Object(id) => {
                if let Some(object) = site
                    .outdoor_objects
                    .iter_mut()
                    .find(|object| object.id == id)
                {
                    object.condition = condition;
                } else {
                    site.buildings
                        .iter_mut()
                        .flat_map(|building| &mut building.rooms)
                        .flat_map(|room| &mut room.objects)
                        .find(|object| object.id == id)
                        .unwrap()
                        .condition = condition;
                }
            }
        }
        world.refresh_market();
    }

    fn start_repair(mut world: Universe, worker: crate::AgentId, place: PlaceId) -> Universe {
        world = world.advance(BIDDING_MS).unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
        let home = citizen.home;
        world = world
            .with_stored_good(
                home,
                crate::storage::GoodsOwner::Agent(worker),
                Good::BreadRoll,
                10_000,
            )
            .unwrap()
            .with_stored_good(
                home,
                crate::storage::GoodsOwner::Agent(worker),
                Good::Water,
                100_000,
            )
            .unwrap();
        world.cancel_maintenance(worker).unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&worker).unwrap().kind;
        citizen.abandon_action().unwrap();
        citizen.active_plan = None;
        citizen.hunger = -100.0;
        citizen.thirst = -100.0;
        citizen.tiredness = -100.0;
        citizen.position = world.map.position(place);
        citizen.set_inventory_units(Good::Wood, 10_000);
        citizen.set_inventory_units(Good::BreadRoll, 0);
        citizen.set_inventory_units(Good::Water, 0);
        world.map = citizen.map.clone();
        world.storage = citizen.storage.clone();
        world.refresh_market();
        assert!(world.start_construction_action(worker).unwrap());
        let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
        assert!(
            matches!(
                citizen.active_action().unwrap().action(),
                crate::CitizenAction::Construct { .. }
            ),
            "{:?}",
            citizen.active_action().unwrap().action()
        );
        world.refresh_market();
        world
    }

    #[test]
    fn partial_repair_survives_failure_and_resumes_fixed_scope_without_more_wood() {
        let (mut world, _, worker, place, target) = fixture(true);
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        let world = world.advance(0).unwrap();
        let id = world.repair_project(place, target).unwrap().id;
        let snapshot = world.construction.projects[&id].repair_damage_units;
        let mut world = start_repair(world, worker.unwrap(), place)
            .advance(HOUR_MS / 4)
            .unwrap();
        let project = world.construction.projects[&id].clone();
        assert!(project.completed_base_ms > 0 && project.remaining_base_ms() > 0);
        assert_eq!(project.missing_wood(), 0);
        let contract = world
            .construction
            .contracts
            .values()
            .find(|contract| contract.project == id && contract.status == ContractStatus::Active)
            .unwrap()
            .id;
        assert_eq!(world.construction.contracts[&contract].paid, 0);
        let before = world.map.repair_target_condition(place, target).unwrap();
        world
            .construction
            .contracts
            .get_mut(&contract)
            .unwrap()
            .deadline_ms = Some(world.current_time_ms + 1);
        world = world.advance(1).unwrap();
        assert_eq!(
            world.construction.contracts[&contract].status,
            ContractStatus::Failed
        );
        assert!(
            world
                .map
                .repair_target_condition(place, target)
                .unwrap()
                .condition_units()
                .abs_diff(before.condition_units())
                < u128::from(crate::condition::CONDITION_LIFETIME_MS) * 1_000
        );
        world = world
            .advance(HOUR_MS - world.current_time_ms % HOUR_MS)
            .unwrap();
        let retained = &world.construction.projects[&id];
        assert_eq!(retained.repair_damage_units, snapshot);
        assert_eq!(retained.installed_wood, project.installed_wood);
        assert!(retained.completed_base_ms >= project.completed_base_ms);
        assert_eq!(retained.missing_wood(), 0);
        assert_eq!(
            world
                .construction
                .contracts
                .values()
                .filter(
                    |contract| contract.project == id && contract.status == ContractStatus::Bidding
                )
                .count(),
            1
        );
    }

    #[test]
    fn active_room_repair_has_exact_work_and_cash_across_split_advancement() {
        let (mut world, owner, worker, place, target) = fixture(true);
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        let world = world.advance(0).unwrap();
        let world = start_repair(world, worker.unwrap(), place);
        let large = world.advance(HOUR_MS / 2).unwrap();
        let mut split = world;
        for _ in 0..30 {
            split = split.advance(HOUR_MS / 60).unwrap();
        }
        assert_eq!(large.construction, split.construction);
        assert_eq!(
            large.account_coins(owner).unwrap(),
            split.account_coins(owner).unwrap()
        );
        assert!(
            large
                .map
                .repair_target_condition(place, target)
                .unwrap()
                .condition_units()
                .abs_diff(
                    split
                        .map
                        .repair_target_condition(place, target)
                        .unwrap()
                        .condition_units()
                )
                <= 1
        );
        for (a, b) in large
            .map
            .place(place)
            .unwrap()
            .objects()
            .zip(split.map.place(place).unwrap().objects())
        {
            assert_eq!(a.id, b.id);
            assert!(
                a.condition
                    .condition_units()
                    .abs_diff(b.condition.condition_units())
                    <= u128::from(crate::condition::CONDITION_LIFETIME_MS),
                "object condition difference {}",
                a.condition
                    .condition_units()
                    .abs_diff(b.condition.condition_units())
            );
        }
        assert_eq!(
            large,
            Universe::from_save_str(&large.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn threshold_posts_on_anchored_hour_and_protects_household_cash() {
        let (mut world, owner, _, place, target) = fixture(false);
        damage(
            &mut world,
            place,
            target,
            MAX_CONDITION_UNITS * 499 / 10_000,
        );
        world = world.advance(HOUR_MS / 2).unwrap();
        assert!(world.repair_project(place, target).is_none());
        world = world.advance(HOUR_MS / 2).unwrap();
        let project = world.repair_project(place, target).unwrap();
        assert!(project.repair_damage_units >= MAX_CONDITION_UNITS / 20);
        assert_eq!(project.client, ConstructionClient::Account(owner));
        let budget = world.construction_budget(owner).unwrap();
        assert!(budget.cash >= budget.essential_reserve);
        assert!(project.funding_offer > 0);
        let saved = world.to_save_string().unwrap();
        assert_eq!(world, Universe::from_save_str(&saved).unwrap());
    }

    #[test]
    fn no_bid_reposts_refresh_unawarded_scope_and_are_split_invariant() {
        let (mut world, _, _, place, target) = fixture(false);
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        let world = world.advance(0).unwrap();
        let id = world.repair_project(place, target).unwrap().id;
        let initial = world.construction.projects[&id].repair_damage_units;
        let large = world.advance(BIDDING_MS + HOUR_MS).unwrap();
        let mut split = world;
        for _ in 0..50 {
            split = split.advance(HOUR_MS / 2).unwrap();
        }
        assert_eq!(large.construction, split.construction);
        assert_eq!(large.map, split.map);
        let project = &large.construction.projects[&id];
        assert!(project.repair_damage_units > initial);
        assert_eq!(
            large
                .construction
                .contracts
                .values()
                .filter(
                    |contract| contract.project == id && contract.status == ContractStatus::Bidding
                )
                .count(),
            1
        );
        assert!(large.construction.contracts.values().any(|contract| contract.project == id && contract.status == ContractStatus::NoBids));
        assert_eq!(
            large,
            Universe::from_save_str(&large.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn lowest_condition_gets_the_last_spare_coin() {
        let (mut world, owner, _, place, target) = fixture(false);
        let bed = RepairTarget::Object(
            world.map.place(place).unwrap().buildings[0].rooms[0].objects[0].id,
        );
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 10);
        damage(&mut world, place, bed, MAX_CONDITION_UNITS / 5);
        let reserve = world.construction_budget(owner).unwrap().essential_reserve;
        world.set_account_coins(owner, reserve + 1).unwrap();
        world.refresh_market();
        world.commission_repairs().unwrap();
        assert!(world.repair_project(place, target).is_none());
        let project = world.repair_project(place, bed).unwrap();
        assert_eq!(project.funding_offer, 1);
        assert_eq!(world.account_coins(owner).unwrap(), reserve);
    }

    #[test]
    fn repair_scope_is_fixed_after_award_and_restores_only_snapshot_damage() {
        let (mut world, _, _, place, target) = fixture(true);
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        let world = world.advance(0).unwrap();
        let id = world.repair_project(place, target).unwrap().id;
        let snapshot = world.construction.projects[&id].repair_damage_units;
        let world = world.advance(3 * BIDDING_MS).unwrap();
        let project = &world.construction.projects[&id];
        assert!(project.completed);
        assert_eq!(project.repair_damage_units, snapshot);
        assert!(
            world
                .map
                .repair_target_condition(place, target)
                .unwrap()
                .damage_units()
                > 0
        );
        assert!(
            world
                .map
                .repair_target_condition(place, target)
                .unwrap()
                .damage_units()
                < snapshot
        );
        assert_eq!(
            world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn guild_and_town_fund_repairs_and_forest_objects_are_exempt() {
        let (world, owner, _, _, _) = fixture(true);
        let (world, workshop) = world.with_property(owner, Location::Mill).unwrap();
        let (mut world, guild) = world.with_guild("Guild", &[owner], &[workshop], 0).unwrap();
        world.set_account_coins(guild, 100).unwrap();
        world
            .guild_histories
            .insert(guild, crate::guild_history::GuildHistory::new(0, 100));
        world.town_treasury = 100;
        let guild_target =
            RepairTarget::Room(world.map.place(workshop).unwrap().buildings[0].rooms[0].id);
        let warehouse = world.map.public_place(Location::Warehouse);
        let town_target = RepairTarget::Object(
            world
                .map
                .place(warehouse)
                .unwrap()
                .objects()
                .next()
                .unwrap()
                .id,
        );
        let forest = world.map.public_place(Location::Forest);
        let forest_target = RepairTarget::Object(
            world
                .map
                .place(forest)
                .unwrap()
                .objects()
                .find(|object| ConstructionItem::from_object_kind(object.kind).is_some())
                .unwrap()
                .id,
        );
        for (place, target) in [
            (workshop, guild_target),
            (warehouse, town_target),
            (forest, forest_target),
        ] {
            damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        }
        world.commission_repairs().unwrap();
        let guild_project = world.repair_project(workshop, guild_target).unwrap();
        assert_eq!(guild_project.client, ConstructionClient::Account(guild));
        assert_eq!(
            world.account_coins(guild).unwrap()
                + guild_project.escrow
                + world
                    .construction
                    .contracts
                    .values()
                    .filter(|contract| contract.project == guild_project.id)
                    .map(|contract| contract.escrow)
                    .sum::<i64>(),
            100
        );
        let town_project = world.repair_project(warehouse, town_target).unwrap();
        assert_eq!(town_project.client, ConstructionClient::Town);
        assert_eq!(
            world.town_treasury
                + town_project.escrow
                + world
                    .construction
                    .contracts
                    .values()
                    .filter(|contract| contract.project == town_project.id)
                    .map(|contract| contract.escrow)
                    .sum::<i64>(),
            100
        );
        assert!(world.repair_project(forest, forest_target).is_none());
        assert_eq!(
            world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
        let guild_id = guild_project.id;
        let town_id = town_project.id;
        let world = world.advance(3 * BIDDING_MS).unwrap();
        assert!(world.construction.projects[&guild_id].completed);
        assert!(world.construction.projects[&town_id].completed);
        let town_paid: i64 = world
            .construction
            .payments
            .iter()
            .filter(|payment| payment.client == ConstructionClient::Town)
            .map(|payment| payment.amount)
            .sum();
        let guild_paid: i64 = world
            .construction
            .payments
            .iter()
            .filter(|payment| payment.client == ConstructionClient::Account(guild))
            .map(|payment| payment.amount)
            .sum();
        assert!(town_paid > 0 && guild_paid > 0);
        assert_eq!(world.town_treasury + town_paid, 100);
        assert_eq!(world.account_coins(guild).unwrap() + guild_paid, 100);
        assert_eq!(
            world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn unawarded_repair_rebinds_to_household_and_records_household_escrow() {
        let (mut world, owner, _, place, target) = fixture(false);
        let (next, partner) = world
            .with_citizen(
                "Partner",
                Citizen::new(-100.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Baker)
                    .with_coins(1_000)
                    .unwrap(),
            )
            .unwrap();
        world = next;
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        let world = world.advance(0).unwrap();
        let project_id = world.repair_project(place, target).unwrap().id;
        let contract = world
            .construction
            .contracts
            .values()
            .find(|contract| contract.project == project_id)
            .unwrap()
            .id;
        let world = world.cancel_construction(owner, contract).unwrap();
        let personal_before = world.citizen_histories[&owner].current.construction;
        let (mut world, household) = world
            .with_household("Family", place, &[owner, partner])
            .unwrap();
        world.commission_repairs().unwrap();
        let project = &world.construction.projects[&project_id];
        assert_eq!(project.client, ConstructionClient::Account(household));
        assert_eq!(
            world.citizen_histories[&owner].current.construction,
            personal_before
        );
        assert_eq!(
            world.household_histories[&household]
                .current
                .construction
                .escrow_deposited,
            project.funding_offer
        );
        assert_eq!(
            world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn saved_repairs_reject_free_materials_and_duplicate_targets() {
        let (mut world, _, _, place, target) = fixture(false);
        damage(&mut world, place, target, MAX_CONDITION_UNITS / 20);
        let mut world = world.advance(0).unwrap();
        let id = world.repair_project(place, target).unwrap().id;
        let mut forged = world.clone();
        forged.construction.projects.get_mut(&id).unwrap().stages[0].wood = 0;
        assert!(forged.validate_construction().is_err());
        let mut duplicate = world.construction.projects[&id].clone();
        duplicate.id = ProjectId(Uuid::new_v4());
        world.construction.projects.insert(duplicate.id, duplicate);
        assert!(world.validate_construction().is_err());
    }
}
