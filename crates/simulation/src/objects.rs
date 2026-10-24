use crate::{
    Citizen, CitizenAction, SimulationError,
    locations::{Object, ObjectId, ObjectKind, Place, RoomId},
};

impl Citizen {
    pub(crate) fn abandon_action(&mut self) -> Result<(), SimulationError> {
        if let Some(active) = self.active_action {
            let site = active.action().required_place(self).unwrap_or(self.home);
            for (good, units) in active.stored_inputs.items() {
                self.storage.release_work_for(
                    self.id,
                    crate::storage::StockKey {
                        place: site,
                        owner: self.goods_owner(),
                        good,
                    },
                    units,
                    false,
                )?;
            }
            if active.action().production_recipe() == Some(crate::production::Recipe::BrewBeer) {
                let vessel = active.claimed_objects[1].ok_or(SimulationError::MissingProperty)?;
                let object = self
                    .map
                    .places_mut()
                    .get_mut(&site)
                    .ok_or(SimulationError::PlaceNotFound)?
                    .objects_mut()
                    .find(|object| object.id == vessel)
                    .ok_or(SimulationError::MissingProperty)?;
                object.kind = ObjectKind::FermentationVessel { batch: None };
            }
            self.map
                .release_objects(site, active.claimed_objects, self.id);
        }
        self.active_action = None;
        self.active_plan = None;
        Ok(())
    }

    pub(crate) fn may_use_object(&self, place: &Place, object: &Object) -> bool {
        if place.kind.is_town_owned() {
            return false;
        }
        let owner = if place.buildings.is_empty() {
            place.owner
        } else {
            place.object_owner(object.id)
        };
        owner.is_none_or(|owner| {
            self.may_use_owner(owner)
                || self
                    .guilds
                    .get(&owner)
                    .is_some_and(|access| access.workshops.contains(&place.id))
        })
    }

    pub(crate) fn accessible_batches<'a>(
        &'a self,
        place: &'a Place,
    ) -> impl Iterator<Item = &'a Option<crate::brewing::Batch>> + 'a {
        place
            .objects()
            .filter(|object| self.may_use_object(place, object))
            .filter_map(|object| match &object.kind {
                ObjectKind::FermentationVessel { batch } => Some(batch),
                _ => None,
            })
    }
    pub(crate) fn accessible_crops<'a>(
        &'a self,
        place: &'a Place,
    ) -> impl Iterator<Item = &'a Option<crate::farming::Crop>> + 'a {
        place
            .objects()
            .filter(|object| self.may_use_object(place, object))
            .filter_map(|object| match &object.kind {
                ObjectKind::CropPlot { crop } => Some(crop),
                _ => None,
            })
    }

    pub fn current_room(&self) -> Option<RoomId> {
        let active = self.active_action()?;
        let site = self.map.place(active.action().required_place(self)?).ok()?;
        active.claimed_objects().find_map(|id| site.object_room(id))
    }

    pub(crate) fn action_objects(
        &self,
        action: CitizenAction,
    ) -> Result<[Option<ObjectId>; 2], SimulationError> {
        let capability = if let Some(recipe) = action.production_recipe() {
            if crate::production::crop_kind(recipe).is_some() {
                return Err(SimulationError::MissingInputs);
            }
            Some(ObjectKind::for_recipe(recipe))
        } else if action == CitizenAction::Sleep {
            Some(ObjectKind::Bed {
                resident: Some(self.id),
            })
        } else {
            None
        };
        if capability.is_none()
            && action.crop_plot().is_none()
            && !matches!(action, CitizenAction::CollectBeer { .. })
        {
            return Ok([None; 2]);
        }
        let site = self.map.place(
            action
                .required_place(self)
                .ok_or(SimulationError::MissingProperty)?,
        )?;
        let specific = if let Some((_, index)) = action.crop_plot() {
            Some(
                site.crop_object(index)
                    .ok_or(SimulationError::MissingProperty)?,
            )
        } else if let CitizenAction::CollectBeer { slot, .. } = action {
            Some(
                site.batch_object(slot)
                    .ok_or(SimulationError::MissingProperty)?,
            )
        } else {
            None
        };
        let object = if let Some(object) = specific {
            if !self.may_use_object(site, object) {
                return Err(SimulationError::PrivateProperty);
            }
            if !object.available(self.id) {
                return Err(SimulationError::MissingInputs);
            }
            object
        } else {
            let kind = capability.unwrap();
            let eligible = || {
                site.objects().filter(|object| object.kind.same_capability(kind))
                .filter(|object| !matches!(object.kind, ObjectKind::Bed { resident } if resident != Some(self.id)))
                .filter(|object| self.may_use_object(site, object))
            };
            if eligible().next().is_none() {
                return Err(SimulationError::MissingProperty);
            }
            eligible()
                .find(|object| object.available(self.id))
                .ok_or(SimulationError::MissingInputs)?
        };
        let vessel = if action.production_recipe() == Some(crate::production::Recipe::BrewBeer) {
            Some(
                site.objects()
                    .find(|object| {
                        matches!(object.kind, ObjectKind::FermentationVessel { batch: None })
                            && self.may_use_object(site, object)
                            && object.available(self.id)
                    })
                    .ok_or(SimulationError::MissingInputs)?
                    .id,
            )
        } else {
            None
        };
        Ok([Some(object.id), vessel])
    }
}

impl crate::Universe {
    pub(crate) fn validate_objects(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::{AgentKind, persistence::SaveError};
        let invalid = || SaveError::InvalidState("object ownership or action claim mismatch");
        let valid_owner = |owner| {
            self.agents.contains_key(&owner)
                || self.guilds.contains_key(&owner)
                || self.households.contains_key(&owner)
        };
        for agent in self.agents.values() {
            let AgentKind::Citizen(worker) = &agent.kind;
            if worker.map != self.map {
                return Err(SaveError::InvalidState("citizen map snapshot mismatch"));
            }
        }
        for place in self.map.places().values() {
            if place.owner.is_some_and(|owner| !valid_owner(owner))
                || place
                    .buildings
                    .iter()
                    .any(|building| !valid_owner(building.owner))
            {
                return Err(invalid());
            }
            for object in place.objects() {
                if let ObjectKind::Bed {
                    resident: Some(resident),
                } = object.kind
                    && !self.agents.contains_key(&resident)
                {
                    return Err(invalid());
                }
                if let ObjectKind::CropPlot { crop: Some(crop) } = object.kind
                    && crop.planted_at_ms > self.current_time_ms
                {
                    return Err(invalid());
                }
                if matches!(
                    object.kind,
                    ObjectKind::FermentationVessel {
                        batch: Some(crate::brewing::Batch::Preparing)
                    }
                ) {
                    if object.workers.len() != 1 {
                        return Err(invalid());
                    }
                    let AgentKind::Citizen(worker) = &self
                        .agents
                        .get(&object.workers[0])
                        .ok_or_else(invalid)?
                        .kind;
                    if !worker.active_action().is_some_and(|active| {
                        active.action().production_recipe()
                            == Some(crate::production::Recipe::BrewBeer)
                            && active.claimed_objects[1] == Some(object.id)
                    }) {
                        return Err(invalid());
                    }
                }
                for id in &object.workers {
                    let AgentKind::Citizen(worker) = &self.agents.get(id).ok_or_else(invalid)?.kind;
                    if !worker.active_action().is_some_and(|active| {
                        active.claimed_objects().any(|id| id == object.id)
                            && active.action().required_place(worker) == Some(place.id)
                    }) {
                        return Err(invalid());
                    }
                }
            }
        }
        for agent in self.agents.values() {
            let AgentKind::Citizen(worker) = &agent.kind;
            let home = self.map.place(worker.home()).map_err(|_| invalid())?;
            if home
                .objects()
                .filter(|object| {
                    object.kind
                        == ObjectKind::Bed {
                            resident: Some(worker.id()),
                        }
                })
                .count()
                != 1
            {
                return Err(SaveError::InvalidState("missing designated bed"));
            }
            let Some(active) = worker.active_action() else {
                continue;
            };
            let action = active.action();
            let required = usize::from(
                action.production_recipe().is_some()
                    || action == CitizenAction::Sleep
                    || action.crop_plot().is_some()
                    || matches!(action, CitizenAction::CollectBeer { .. }),
            ) + usize::from(
                action.production_recipe() == Some(crate::production::Recipe::BrewBeer),
            );
            if active.claimed_objects().count() != required
                || (required > 0 && active.claimed_objects[0].is_none())
                || active.claimed_objects[0].is_some()
                    && active.claimed_objects[0] == active.claimed_objects[1]
            {
                return Err(invalid());
            }
            if required == 0 {
                continue;
            }
            let site = self
                .map
                .place(action.required_place(worker).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            if worker.position() != site.position {
                return Err(invalid());
            }
            for (index, id) in active.claimed_objects().enumerate() {
                let object = site.object(id).ok_or_else(invalid)?;
                if !object.workers.contains(&worker.id()) || !worker.may_use_object(site, object) {
                    return Err(invalid());
                }
                let matches = if index == 1 {
                    matches!(
                        object.kind,
                        ObjectKind::FermentationVessel {
                            batch: Some(crate::brewing::Batch::Preparing)
                        }
                    )
                } else if let Some(recipe) = action.production_recipe() {
                    crate::production::crop_kind(recipe).is_none()
                        && recipe.duration_ms(worker).is_ok()
                        && object.kind.same_capability(ObjectKind::for_recipe(recipe))
                } else if action == CitizenAction::Sleep {
                    object.kind
                        == ObjectKind::Bed {
                            resident: Some(worker.id()),
                        }
                } else if let Some((_, plot)) = action.crop_plot() {
                    let crop = match object.kind {
                        ObjectKind::CropPlot { crop } => crop,
                        _ => return Err(invalid()),
                    };
                    let state_valid = match action {
                        CitizenAction::Plant { .. } => crop.is_none(),
                        CitizenAction::Tend { stage, .. } => crop.is_some_and(|crop| {
                            crop.tend_window(stage).is_some_and(|(start, end)| {
                                !crop.tended[stage]
                                    && self.current_time_ms >= start
                                    && self
                                        .current_time_ms
                                        .checked_add(active.remaining_ms())
                                        .is_some_and(|finish| finish <= end)
                            })
                        }),
                        CitizenAction::Harvest { .. } => crop.is_some_and(|crop| {
                            crop.status(self.current_time_ms) == crate::farming::CropStatus::Ready
                        }),
                        CitizenAction::Clear { .. } => crop.is_some_and(|crop| {
                            crop.status(self.current_time_ms) == crate::farming::CropStatus::Dead
                        }),
                        _ => false,
                    };
                    state_valid
                        && worker
                            .skill_practice_ms(crate::production::Skill::Farming)
                            .is_some()
                        && site
                            .crop_object(plot)
                            .is_some_and(|candidate| candidate.id == id)
                } else if let CitizenAction::CollectBeer { slot, .. } = action {
                    site.batch_object(slot).is_some_and(|candidate| candidate.id == id && matches!(candidate.kind, ObjectKind::FermentationVessel { batch: Some(batch) } if batch.ready(self.current_time_ms)))
                } else {
                    false
                };
                if !matches {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentId, AgentKind, StartingRole, Universe,
        locations::{Location, Map},
        marketplace::Good,
        production::Recipe,
    };

    fn household(
        role: StartingRole,
        kind: Location,
    ) -> (Universe, AgentId, AgentId, crate::locations::PlaceId) {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen("First", Citizen::new(0.0).unwrap().with_starting_role(role))
            .unwrap();
        let (world, second) = world
            .with_citizen(
                "Second",
                Citizen::new(0.0).unwrap().with_starting_role(role),
            )
            .unwrap();
        let AgentKind::Citizen(worker) = &world.agents[&first].kind;
        let (world, _) = world
            .with_household("Family", worker.home(), &[first, second])
            .unwrap();
        let (world, site) = world.with_property(first, kind).unwrap();
        (world, first, second, site)
    }
    fn worker(world: &Universe, id: AgentId) -> &Citizen {
        let AgentKind::Citizen(worker) = &world.agents[&id].kind;
        worker
    }
    fn stocked(
        mut world: Universe,
        owner: AgentId,
        site: crate::locations::PlaceId,
        recipes: &[Recipe],
    ) -> Universe {
        let id = owner;
        let owner = worker(&world, owner).goods_owner();
        for recipe in recipes {
            for &(good, units) in recipe.inputs() {
                let total = world.storage.units(site, owner, good) + units * 4;
                let allowed = worker(&world, id).accessible_storage_objects(site);
                world
                    .storage
                    .set_units_in(
                        crate::storage::StockKey {
                            place: site,
                            owner,
                            good,
                        },
                        total,
                        &world.map,
                        &world.market,
                        Some(&allowed),
                    )
                    .unwrap();
                world.refresh_market();
            }
        }
        world
    }
    fn round_trip(world: &Universe) {
        assert_eq!(
            &Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
            world
        );
    }

    #[test]
    fn shared_equipment_is_claimed_only_during_work() {
        let (world, first, second, site) = household(StartingRole::Miller, Location::Mill);
        let world = stocked(world, first, site, &[Recipe::MillFlour]);
        let action = CitizenAction::ProduceAt {
            place: site,
            recipe: Recipe::MillFlour,
        };
        assert!(worker(&world, first).action_duration_ms(action).is_ok());
        assert!(worker(&world, second).action_duration_ms(action).is_ok());
        assert!(
            world
                .map
                .place(site)
                .unwrap()
                .objects()
                .all(|object| object.workers.is_empty())
        );
        let world = world.start_action(first, action).unwrap();
        assert_eq!(
            world.start_action(second, action),
            Err(SimulationError::MissingInputs)
        );
        assert!(worker(&world, first).current_room().is_some());
        round_trip(&world);
        let world = world
            .advance(
                worker(&world, first)
                    .active_action()
                    .unwrap()
                    .remaining_ms(),
            )
            .unwrap();
        assert!(worker(&world, first).current_room().is_none());
        assert!(world.start_action(second, action).is_ok());
    }

    #[test]
    fn distinct_equipment_in_one_room_and_public_resources_allow_concurrent_work() {
        let (world, first, second, site) = household(StartingRole::Weaver, Location::Weavery);
        let world = stocked(
            world,
            first,
            site,
            &[Recipe::SpinThread, Recipe::WeaveCloth],
        );
        let world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::SpinThread,
                },
            )
            .unwrap()
            .start_action(
                second,
                CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::WeaveCloth,
                },
            )
            .unwrap();
        assert_eq!(
            worker(&world, first).current_room(),
            worker(&world, second).current_room()
        );
        assert_ne!(
            worker(&world, first)
                .active_action()
                .unwrap()
                .claimed_objects[0],
            worker(&world, second)
                .active_action()
                .unwrap()
                .claimed_objects[0]
        );
        round_trip(&world);
        let (world, first, second, _) = household(StartingRole::Weaver, Location::Weavery);
        let world = world
            .start_action(first, CitizenAction::Produce(Recipe::Forage))
            .unwrap()
            .start_action(second, CitizenAction::Produce(Recipe::Forage))
            .unwrap();
        let forest = world
            .map
            .place(world.map.public_place(Location::Forest))
            .unwrap();
        assert_eq!(
            forest
                .objects()
                .find(|object| object.kind == ObjectKind::BerryBushes)
                .unwrap()
                .workers
                .len(),
            2
        );
        round_trip(&world);
    }

    #[test]
    fn household_residents_have_separate_bedrooms() {
        let (world, first, second, _) = household(StartingRole::Miller, Location::Mill);
        let home = worker(&world, first).home();
        assert_eq!(home, worker(&world, second).home());
        assert_eq!(world.map.place(home).unwrap().buildings[0].rooms.len(), 3);
        assert_eq!(
            world
                .map
                .place(home)
                .unwrap()
                .objects()
                .filter(|object| object.kind.is_storage())
                .count(),
            5
        );
        let world = world
            .start_action(first, CitizenAction::Sleep)
            .unwrap()
            .start_action(second, CitizenAction::Sleep)
            .unwrap();
        assert_ne!(
            worker(&world, first).current_room(),
            worker(&world, second).current_room()
        );
        round_trip(&world);
    }

    #[test]
    fn brewing_workers_finish_their_own_vessels_and_fermentation_keeps_no_worker() {
        let (mut world, first, second, site) = household(StartingRole::Brewer, Location::Brewery);
        let room = &mut world.map.places_mut().get_mut(&site).unwrap().buildings[0].rooms[0];
        room.size = crate::locations::RoomSize::Large;
        room.objects.push(Object::new(ObjectKind::BrewingEquipment));
        world.refresh_market();
        world = stocked(world, first, site, &[Recipe::BrewBeer]);
        let AgentKind::Citizen(second_worker) = &mut world.agents.get_mut(&second).unwrap().kind;
        *second_worker = second_worker
            .with_skill_practice_ms(
                crate::production::Skill::Brewing,
                Some(crate::production::MAX_SKILL_PRACTICE_MS),
            )
            .unwrap();
        let action = CitizenAction::ProduceAt {
            place: site,
            recipe: Recipe::BrewBeer,
        };
        let world = world
            .start_action(first, action)
            .unwrap()
            .start_action(second, action)
            .unwrap();
        let first_vessel = worker(&world, first)
            .active_action()
            .unwrap()
            .claimed_objects[1]
            .unwrap();
        let second_vessel = worker(&world, second)
            .active_action()
            .unwrap()
            .claimed_objects[1]
            .unwrap();
        assert_ne!(first_vessel, second_vessel);
        round_trip(&world);
        let world = world.advance(crate::brewing::PREPARATION_MS / 2).unwrap();
        let site_state = world.map.place(site).unwrap();
        assert!(matches!(
            site_state.object(first_vessel).unwrap().kind,
            ObjectKind::FermentationVessel {
                batch: Some(crate::brewing::Batch::Preparing)
            }
        ));
        assert!(matches!(
            site_state.object(second_vessel).unwrap().kind,
            ObjectKind::FermentationVessel {
                batch: Some(crate::brewing::Batch::Fermenting { .. })
            }
        ));
        assert!(site_state.object(second_vessel).unwrap().workers.is_empty());
        round_trip(&world);
        let world = world.advance(crate::brewing::PREPARATION_MS / 2).unwrap();
        assert!(
            world
                .map
                .place(site)
                .unwrap()
                .objects()
                .all(|object| object.workers.is_empty())
        );
        assert_eq!(
            world
                .map
                .place(site)
                .unwrap()
                .batches()
                .iter()
                .flatten()
                .count(),
            2
        );
        round_trip(&world);
    }

    #[test]
    fn colocated_buildings_use_their_owners_access_and_capabilities() {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen(
                "First",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (mut world, second) = world
            .with_citizen(
                "Second",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Baker),
            )
            .unwrap();
        let site = worker(&world, first).home();
        world.map = world
            .map
            .with_building(site, Location::Mill, first, "Mill")
            .unwrap()
            .0;
        world.map = world
            .map
            .with_building(site, Location::Bakery, second, "Bakery")
            .unwrap()
            .0;
        world.refresh_market();
        assert!(
            worker(&world, first)
                .validate_production_site(site, Recipe::MillFlour)
                .is_ok()
        );
        assert!(
            worker(&world, first)
                .validate_production_site(site, Recipe::BakeRolls)
                .is_err()
        );
        assert!(
            worker(&world, second)
                .validate_production_site(site, Recipe::BakeRolls)
                .is_ok()
        );
        assert!(
            worker(&world, second)
                .validate_production_site(site, Recipe::MillFlour)
                .is_err()
        );
        world = stocked(world, second, site, &[Recipe::BakeRolls]);
        let world = world
            .start_action(
                second,
                CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::BakeRolls,
                },
            )
            .unwrap();
        assert_eq!(world.map.place(site).unwrap().owner, Some(first));
        round_trip(&world);
    }
    #[test]
    fn abandoning_work_releases_equipment_inputs_and_preparing_vessel() {
        let (world, first, _, site) = household(StartingRole::Miller, Location::Mill);
        let world = stocked(world, first, site, &[Recipe::MillFlour]);
        let world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::MillFlour,
                },
            )
            .unwrap();
        let mut citizen = worker(&world, first).clone();
        citizen.abandon_action().unwrap();
        assert!(
            citizen
                .map
                .place(site)
                .unwrap()
                .objects()
                .all(|object| object.workers.is_empty())
        );
        assert_eq!(
            citizen
                .storage
                .work_reserved_units(crate::storage::StockKey {
                    place: site,
                    owner: citizen.goods_owner(),
                    good: Good::Wheat
                }),
            0
        );
        assert!(
            citizen
                .start_action(CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::MillFlour
                })
                .is_ok()
        );
        let (world, first, _, site) = household(StartingRole::Brewer, Location::Brewery);
        let world = stocked(world, first, site, &[Recipe::BrewBeer]);
        let world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    place: site,
                    recipe: Recipe::BrewBeer,
                },
            )
            .unwrap();
        let mut citizen = worker(&world, first).clone();
        citizen.abandon_action().unwrap();
        assert!(
            citizen
                .map
                .place(site)
                .unwrap()
                .objects()
                .all(|object| object.workers.is_empty())
        );
        assert!(
            citizen
                .map
                .place(site)
                .unwrap()
                .batches()
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn saved_collection_requires_ready_batch_and_saved_snapshots_cannot_panic() {
        let (mut world, first, _, site) = household(StartingRole::Brewer, Location::Brewery);
        world
            .map
            .set_batch(
                site,
                0,
                Some(crate::brewing::Batch::Fermenting {
                    ready_at_ms: crate::brewing::FERMENTATION_MS,
                }),
            )
            .unwrap();
        world.refresh_market();
        let world = world
            .advance(crate::brewing::FERMENTATION_MS)
            .unwrap()
            .start_action(
                first,
                CitizenAction::CollectBeer {
                    brewery: site,
                    slot: 0,
                },
            )
            .unwrap();
        round_trip(&world);
        for batch in [
            None,
            Some(crate::brewing::Batch::Fermenting {
                ready_at_ms: world.current_time_ms + 1,
            }),
        ] {
            let mut invalid = world.clone();
            invalid.map.set_batch(site, 0, batch).unwrap();
            invalid.refresh_market();
            assert!(matches!(
                Universe::from_save_str(&invalid.to_save_string().unwrap()),
                Err(crate::persistence::SaveError::InvalidState(_))
            ));
        }
        let (world, first, _, _) = household(StartingRole::Miller, Location::Mill);
        let mut world = world
            .start_action(first, CitizenAction::Produce(Recipe::Forage))
            .unwrap();
        let forest = world.map.public_place(Location::Forest);
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&first).unwrap().kind;
        citizen.map.places_mut().remove(&forest);
        assert!(matches!(
            Universe::from_save_str(&world.to_save_string().unwrap()),
            Err(crate::persistence::SaveError::InvalidState(_))
        ));
    }
    #[test]
    fn collection_candidates_skip_private_and_busy_vessels() {
        let (world, first, second, site) = household(StartingRole::Brewer, Location::Brewery);
        let (mut world, outsider) = world
            .with_citizen(
                "Other brewer",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Brewer),
            )
            .unwrap();
        world.map.places_mut().get_mut(&site).unwrap().buildings[0].owner = outsider;
        world.map = world
            .map
            .with_building(site, Location::Brewery, first, "Family brewery")
            .unwrap()
            .0;
        for slot in [0, 4, 5] {
            world
                .map
                .set_batch(
                    site,
                    slot,
                    Some(crate::brewing::Batch::Fermenting {
                        ready_at_ms: crate::brewing::FERMENTATION_MS,
                    }),
                )
                .unwrap();
        }
        world.refresh_market();
        let world = world.advance(crate::brewing::FERMENTATION_MS).unwrap();
        assert_eq!(
            worker(&world, second).brewing_actions_at(site),
            vec![
                CitizenAction::CollectBeer {
                    brewery: site,
                    slot: 4
                },
                CitizenAction::CollectBeer {
                    brewery: site,
                    slot: 5
                }
            ]
        );
        crate::production::assert_production_stocks_match(worker(&world, second));
        assert_eq!(
            crate::production::production_stock(Good::Beer, worker(&world, second)),
            2000.0
        );
        let world = world
            .start_action(
                first,
                CitizenAction::CollectBeer {
                    brewery: site,
                    slot: 4,
                },
            )
            .unwrap();
        assert_eq!(
            worker(&world, second).brewing_actions_at(site),
            vec![CitizenAction::CollectBeer {
                brewery: site,
                slot: 5
            }]
        );
        assert!(crate::planning::plan(worker(&world, second)).is_ok());
        round_trip(&world);
    }
    #[test]
    fn brewing_trial_tracks_selected_accessible_vessel() {
        let (world, first, _, site) = household(StartingRole::Brewer, Location::Brewery);
        let (mut world, outsider) = world
            .with_citizen(
                "Other brewer",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Brewer),
            )
            .unwrap();
        world.map.places_mut().get_mut(&site).unwrap().buildings[0].owner = outsider;
        world.map = world
            .map
            .with_building(site, Location::Brewery, first, "Family brewery")
            .unwrap()
            .0;
        world.refresh_market();
        world = stocked(world, first, site, &[Recipe::BrewBeer]);
        let action = CitizenAction::ProduceAt {
            place: site,
            recipe: Recipe::BrewBeer,
        };
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&first).unwrap().kind;
        citizen.prices = citizen.prices.with_price(Good::Beer, 0.01).unwrap();
        citizen.market.prices = citizen.prices;
        citizen
            .claim_production_trial(Recipe::BrewBeer, Some(site))
            .unwrap();
        citizen.start_trial_production(action).unwrap();
        let account = citizen.economic_id();
        world.market = citizen.market.clone();
        let world = world.start_action(first, action).unwrap();
        assert_eq!(
            world
                .market
                .production_trial(account, Good::Beer)
                .unwrap()
                .source,
            Some(CitizenAction::CollectBeer {
                brewery: site,
                slot: 4
            })
        );
        round_trip(&world);
        let world = world
            .advance(crate::brewing::PREPARATION_MS + crate::brewing::FERMENTATION_MS)
            .unwrap()
            .start_action(
                first,
                CitizenAction::CollectBeer {
                    brewery: site,
                    slot: 4,
                },
            )
            .unwrap()
            .advance(crate::brewing::COLLECTION_MS)
            .unwrap();
        assert_eq!(
            world
                .storage
                .units(site, worker(&world, first).goods_owner(), Good::Beer),
            1000
        );
        round_trip(&world);
    }
}
