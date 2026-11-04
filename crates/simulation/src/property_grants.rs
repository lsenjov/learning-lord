use crate::{
    AgentId, AgentKind, SimulationError, Universe,
    locations::{ObjectKind, PlaceId},
    marketplace::ShoppingList,
    storage::GoodsOwner,
};

impl Universe {
    pub fn validate_property_grant(
        &self,
        place: PlaceId,
        household: AgentId,
    ) -> Result<(), SimulationError> {
        let reject = |reason| Err(SimulationError::PropertyGrantRejected(reason));
        if !self.households.contains_key(&household) {
            return reject("Select a live household.");
        }
        if self.map.places().values().any(|site| {
            site.owner == Some(household)
                || site
                    .buildings
                    .iter()
                    .any(|building| building.owner == Some(household))
        }) {
            return reject("The household already owns real property.");
        }
        let site = self.map.place(place)?;
        if site.owner.is_some() || site.kind.is_public() || site.kind.is_town_owned() {
            return reject("Only former private town property can be granted.");
        }
        if site
            .buildings
            .iter()
            .any(|building| building.owner.is_some())
        {
            return reject("Foreign buildings prevent this grant.");
        }
        if self
            .storage
            .stock()
            .iter()
            .any(|(key, units)| key.place == place && *units > 0 && key.owner != GoodsOwner::Town)
            || self.market.orders().any(|order| order.place == place)
        {
            return reject("Foreign goods or market listings prevent this grant.");
        }
        if self
            .construction
            .projects
            .values()
            .any(|project| project.place == place && !project.completed)
        {
            return reject("Unfinished construction prevents this grant.");
        }
        if site.objects().any(|object| {
            !object.workers.is_empty()
                || matches!(
                    object.kind,
                    ObjectKind::Bed { resident: Some(_) }
                        | ObjectKind::FermentationVessel { batch: Some(_) }
                        | ObjectKind::CropPlot { crop: Some(_) }
                )
        }) {
            return reject("Active workers, residents, batches or crops prevent this grant.");
        }
        if self.construction.contracts.values().any(|contract| {
            contract.status.open()
                && contract
                    .carpenter
                    .is_some_and(|id| self.households[&household].members.contains(&id))
        }) {
            return reject("A household member has an open construction contract.");
        }
        if crate::marketplace::Good::ALL
            .into_iter()
            .any(|good| self.market.production_trial(household, good).is_some())
        {
            return reject("An active production trial prevents this grant.");
        }
        Ok(())
    }

    pub fn grant_property(
        &self,
        place: PlaceId,
        household: AgentId,
    ) -> Result<Self, SimulationError> {
        self.validate_property_grant(place, household)?;
        let mut world = self.clone();
        let members = world.households[&household].members.clone();
        let replanning: Vec<_> = members
            .iter()
            .copied()
            .filter(|id| {
                let AgentKind::Citizen(citizen) = &world.agents[id].kind;
                citizen.active_plan.is_some()
            })
            .collect();
        for id in &members {
            world.cancel_maintenance(*id)?;
            let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(id).unwrap().kind;
            citizen.map = world.map.clone();
            citizen.storage = world.storage.clone();
            citizen.abandon_action()?;
            citizen.market_request = ShoppingList::default();
            citizen.request_group = None;
            world.map = citizen.map.clone();
            world.storage = citizen.storage.clone();
        }
        let site = world.map.places_mut().get_mut(&place).unwrap();
        site.owner = Some(household);
        for building in &mut site.buildings {
            building.owner = Some(household);
        }
        let mut beds = site
            .buildings
            .iter_mut()
            .flat_map(|building| &mut building.rooms)
            .filter(|room| room.condition.condition_units() > 0)
            .flat_map(|room| &mut room.objects)
            .filter(|object| {
                object.condition.condition_units() > 0
                    && object.kind == ObjectKind::Bed { resident: None }
            });
        let mut home = false;
        for member in &members {
            if let Some(bed) = beds.next() {
                bed.kind = ObjectKind::Bed {
                    resident: Some(*member),
                };
                home = true;
            }
        }
        let home = home.then_some(place);
        world.households.get_mut(&household).unwrap().home = home;
        for id in &members {
            let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(id).unwrap().kind;
            citizen.home = home;
        }
        for (_, agent) in world.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.map = world.map.clone();
        }
        world.refresh_account_requests()?;
        world.refresh_tax_rates();
        world.refresh_construction_context()?;
        world.refresh_market();
        world.refresh_workplace_supplies()?;
        world.refresh_household_history(world.current_time_ms, false)?;
        for id in replanning {
            if world.agents.contains_key(&id) {
                world = world.start_planning(id)?;
            }
        }
        Ok(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Citizen, CitizenAction, StartingRole, locations::Location, marketplace::Good,
        production::Recipe,
    };

    fn fixture() -> (Universe, PlaceId, AgentId, AgentId) {
        let (world, old) = Universe::default()
            .with_citizen(
                "Old",
                Citizen::new(0.0).unwrap().with_furnished_home().unwrap(),
            )
            .unwrap();
        let account = world.economic_id(old).unwrap();
        let home = world.households[&account].home.unwrap();
        let world = world
            .with_household_workplace(old, Location::Weavery)
            .unwrap()
            .0
            .remove_household(account)
            .unwrap();
        let (world, member) = world
            .with_citizen(
                "New",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Weaver)
                    .with_coins(50)
                    .unwrap(),
            )
            .unwrap();
        let household = world.economic_id(member).unwrap();
        (world, home, household, member)
    }

    fn round_trip(world: &Universe) {
        assert_eq!(
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
            *world
        );
    }

    #[test]
    fn grant_departed_property_preserves_assets_and_town_goods_and_enables_sleep_and_work() {
        let (world, site, household, member) = fixture();
        let world = world
            .with_stored_good(site, GoodsOwner::Town, Good::Flax, 500)
            .unwrap()
            .with_stored_good(site, GoodsOwner::Town, Good::Berries, 500)
            .unwrap();
        let contents = world.storage.contents().clone();
        let old_site = world.map.place(site).unwrap().clone();
        let AgentKind::Citizen(before) = &world.agents[&member].kind;
        let position = before.position();
        let world = world
            .start_action(member, CitizenAction::Wait)
            .unwrap()
            .advance(1000)
            .unwrap();
        let mut expected = world.map.place(site).unwrap().clone();
        expected.owner = Some(household);
        for building in &mut expected.buildings {
            building.owner = Some(household);
        }
        for object in expected.objects_mut() {
            if object.kind == (ObjectKind::Bed { resident: None }) {
                object.kind = ObjectKind::Bed {
                    resident: Some(member),
                };
                break;
            }
        }
        let world = world.grant_property(site, household).unwrap();
        assert_eq!(world.map.place(site).unwrap(), &expected);
        assert_eq!(world.households[&household].coins, 50);
        let AgentKind::Citizen(after) = &world.agents[&member].kind;
        assert_eq!(after.position(), position);
        assert_eq!(world.storage.contents(), &contents);
        let new_site = world.map.place(site).unwrap();
        assert_eq!(new_site.id, old_site.id);
        for (before, after) in old_site.buildings.iter().zip(&new_site.buildings) {
            assert_eq!(before.id, after.id);
            assert_eq!(after.owner, Some(household));
            assert_eq!(before.rooms.len(), after.rooms.len());
            for (before, after) in before.rooms.iter().zip(&after.rooms) {
                assert_eq!(before.id, after.id);
                assert_eq!(
                    before
                        .objects
                        .iter()
                        .map(|object| object.id)
                        .collect::<Vec<_>>(),
                    after
                        .objects
                        .iter()
                        .map(|object| object.id)
                        .collect::<Vec<_>>()
                );
            }
        }
        let AgentKind::Citizen(citizen) = &world.agents[&member].kind;
        assert_eq!(citizen.sleep_place(), Some(site));
        assert_eq!(citizen.production_place(Recipe::SpinThread).unwrap(), site);
        assert_eq!(citizen.stored_available_units(site, Good::Flax), 0);
        assert_eq!(citizen.stored_available_units(site, Good::Berries), 0);
        assert_eq!(world.account_owned_units(household, Good::Flax).unwrap(), 0);
        round_trip(&world);
        let world = world
            .with_stored_good(site, GoodsOwner::Household(household), Good::Flax, 500)
            .unwrap();
        let world = world
            .start_action(member, CitizenAction::Travel(site))
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&member].kind;
        let duration = citizen.active_action().unwrap().remaining_ms();
        let world = world.advance(duration).unwrap();
        let eating = world.start_action(member, CitizenAction::Eat).unwrap();
        assert_eq!(eating.storage, world.storage);
        let AgentKind::Citizen(eater) = &eating.agents[&member].kind;
        assert!(eater.active_action().is_none());
        let AgentKind::Citizen(citizen) = &world.agents[&member].kind;
        assert_eq!(citizen.withdrawable_units(Good::Berries), 0);
        let sleeping = world
            .start_action(member, CitizenAction::Sleep)
            .unwrap()
            .advance(1000)
            .unwrap();
        round_trip(&sleeping);
        let working = world
            .start_action(member, CitizenAction::Produce(Recipe::SpinThread))
            .unwrap()
            .advance(1000)
            .unwrap();
        round_trip(&working);
        assert_eq!(
            working.storage.units(site, GoodsOwner::Town, Good::Flax),
            500
        );
    }

    #[test]
    fn grant_reuses_one_bed_without_building_for_excess_members_and_land_is_not_home() {
        let (world, site, household, member) = fixture();
        let (world, second) = world
            .with_household_member(household, "Second", Citizen::new(0.0).unwrap())
            .unwrap();
        let before = world.map.place(site).unwrap().objects().count();
        let world = world.grant_property(site, household).unwrap();
        assert_eq!(world.map.place(site).unwrap().objects().count(), before);
        for (id, expected) in [(member, Some(site)), (second, None)] {
            let AgentKind::Citizen(citizen) = &world.agents[&id].kind;
            assert_eq!(citizen.home, Some(site));
            assert_eq!(citizen.sleep_place(), expected);
        }
        round_trip(&world);
        let (world, site, household, _) = fixture();
        let mut world = world;
        for object in world.map.places_mut().get_mut(&site).unwrap().objects_mut() {
            if matches!(object.kind, ObjectKind::Bed { .. }) {
                object.kind = ObjectKind::PantryShelving;
            }
        }
        let world = world.grant_property(site, household).unwrap();
        assert_eq!(world.households[&household].home, None);
        round_trip(&world);
    }

    #[test]
    fn grant_rejects_retained_unfinished_construction_after_departure() {
        use crate::construction::ConstructionItem;
        use crate::locations::RoomSize;
        let (world, old) = Universe::default()
            .with_citizen(
                "Old",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(10_000)
                    .unwrap(),
            )
            .unwrap();
        let account = world.economic_id(old).unwrap();
        let site = world.households[&account].home.unwrap();
        let building = world.map.place(site).unwrap().buildings[0].id;
        let (world, _) = world
            .request_construction(
                old,
                site,
                building,
                None,
                ConstructionItem::Room(RoomSize::Small),
                1000,
            )
            .unwrap();
        let world = world.remove_household(account).unwrap();
        assert!(
            world
                .construction
                .projects
                .values()
                .any(|project| project.place == site && !project.completed)
        );
        let (world, member) = world
            .with_citizen("New", Citizen::new(0.0).unwrap())
            .unwrap();
        assert!(
            world
                .grant_property(site, world.economic_id(member).unwrap())
                .unwrap_err()
                .to_string()
                .contains("Unfinished construction")
        );
        round_trip(&world);
    }

    #[test]
    fn grant_rejects_foreign_assets_owned_households_public_and_stale_targets_atomically() {
        let (world, site, household, member) = fixture();
        let (world, other) = world
            .with_citizen("Other", Citizen::new(0.0).unwrap())
            .unwrap();
        let foreign = world.economic_id(other).unwrap();
        let foreign_stock = world
            .with_stored_good(site, GoodsOwner::Household(foreign), Good::Flax, 20)
            .unwrap();
        assert!(
            foreign_stock
                .grant_property(site, household)
                .unwrap_err()
                .to_string()
                .contains("Foreign goods")
        );
        let mut foreign_building = world.clone();
        foreign_building
            .map
            .places_mut()
            .get_mut(&site)
            .unwrap()
            .buildings[0]
            .owner = Some(foreign);
        assert!(
            foreign_building
                .grant_property(site, household)
                .unwrap_err()
                .to_string()
                .contains("Foreign buildings")
        );
        let (owned, _) = world.with_property(member, Location::Field).unwrap();
        assert!(
            owned
                .grant_property(site, household)
                .unwrap_err()
                .to_string()
                .contains("already owns")
        );
        assert!(
            world
                .grant_property(world.map.public_place(Location::Market), household)
                .is_err()
        );
        assert!(
            world
                .grant_property(site, AgentId(uuid::Uuid::new_v4()))
                .is_err()
        );
        assert!(
            world
                .grant_property(PlaceId(uuid::Uuid::new_v4()), household)
                .is_err()
        );
        let mut busy = world.clone();
        busy.map
            .places_mut()
            .get_mut(&site)
            .unwrap()
            .objects_mut()
            .next()
            .unwrap()
            .workers
            .push(other);
        assert!(
            busy.grant_property(site, household)
                .unwrap_err()
                .to_string()
                .contains("Active workers")
        );
        let granted = world.grant_property(site, household).unwrap();
        assert!(granted.grant_property(site, foreign).is_err());
        round_trip(&world);
    }
}
