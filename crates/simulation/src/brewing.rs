use crate::{
    Citizen, CitizenAction, SimulationError,
    locations::{Location, PlaceId},
    marketplace::Good,
    production::{Recipe, Skill, skill_duration_reduction},
};

pub const BREWERY_SLOTS: usize = 4;
pub const FERMENTATION_MS: u64 = 48 * 3_600_000;
pub const PREPARATION_MS: u64 = 3_600_000;
pub const COLLECTION_MS: u64 = 15 * 60_000;
pub const BATCH_BEER_GRAMS: u64 = 1000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Batch {
    Preparing,
    Fermenting { ready_at_ms: u64 },
}

impl Batch {
    pub fn ready(self, now_ms: u64) -> bool {
        matches!(self, Self::Fermenting { ready_at_ms } if now_ms >= ready_at_ms)
    }
    pub(crate) fn valid(self) -> bool {
        match self {
            Self::Preparing => true,
            Self::Fermenting { ready_at_ms } => ready_at_ms >= FERMENTATION_MS,
        }
    }
}

impl crate::locations::Map {
    pub(crate) fn set_batch(
        &mut self,
        brewery: PlaceId,
        slot: usize,
        batch: Option<Batch>,
    ) -> Result<(), SimulationError> {
        let place = self
            .places_mut()
            .get_mut(&brewery)
            .ok_or(SimulationError::PlaceNotFound)?;
        *place
            .batches
            .get_mut(slot)
            .ok_or(SimulationError::MissingProperty)? = batch;
        Ok(())
    }
}

impl Citizen {
    pub fn beer_batch(
        &self,
        brewery: PlaceId,
        slot: usize,
    ) -> Result<Option<Batch>, SimulationError> {
        let place = self.map.place(brewery)?;
        if place.kind != Location::Brewery {
            return Err(SimulationError::MissingProperty);
        }
        if place.owner != Some(self.id) {
            return Err(SimulationError::PrivateProperty);
        }
        place
            .batches
            .get(slot)
            .copied()
            .ok_or(SimulationError::MissingProperty)
    }
    fn brewing_labor_duration(&self, base: u64) -> Result<u64, SimulationError> {
        let practice = self
            .skill_practice_ms(Skill::Brewing)
            .ok_or(SimulationError::MissingSkill)?;
        Ok((base as f64 * (1.0 - skill_duration_reduction(practice))).ceil() as u64)
    }
    pub(crate) fn brewing_preparation_duration(
        &self,
        action: CitizenAction,
    ) -> Result<u64, SimulationError> {
        let site = action
            .required_place(self)
            .ok_or(SimulationError::MissingProperty)?;
        if self.map.place(site)?.batches.iter().all(Option::is_some) {
            return Err(SimulationError::MissingInputs);
        }
        let duration = self.brewing_labor_duration(PREPARATION_MS)?;
        self.current_time_ms()
            .checked_add(duration)
            .and_then(|time| time.checked_add(FERMENTATION_MS))
            .ok_or(SimulationError::TimeOverflow)?;
        Ok(duration)
    }
    pub(crate) fn start_brewing(&mut self, action: CitizenAction) -> Result<(), SimulationError> {
        let site = action
            .required_place(self)
            .ok_or(SimulationError::MissingProperty)?;
        let slot = self
            .map
            .place(site)?
            .batches
            .iter()
            .position(Option::is_none)
            .ok_or(SimulationError::MissingInputs)?;
        for &(good, units) in Recipe::BrewBeer.inputs() {
            self.set_inventory_units(
                good,
                self.units(good)
                    .checked_sub(units)
                    .ok_or(SimulationError::MissingInputs)?,
            );
        }
        self.map.set_batch(site, slot, Some(Batch::Preparing))
    }
    pub(crate) fn finish_preparation(
        &mut self,
        action: CitizenAction,
        finish: u64,
    ) -> Result<(), SimulationError> {
        let site = action
            .required_place(self)
            .ok_or(SimulationError::MissingProperty)?;
        let slot = self
            .map
            .place(site)?
            .batches
            .iter()
            .position(|batch| *batch == Some(Batch::Preparing))
            .ok_or(SimulationError::MissingInputs)?;
        self.map.set_batch(
            site,
            slot,
            Some(Batch::Fermenting {
                ready_at_ms: finish
                    .checked_add(FERMENTATION_MS)
                    .ok_or(SimulationError::TimeOverflow)?,
            }),
        )?;
        if let Some(targets) = &mut self.production_targets {
            targets.complete(Recipe::BrewBeer);
        }
        Ok(())
    }
    pub(crate) fn beer_collection_duration(
        &self,
        brewery: PlaceId,
        slot: usize,
    ) -> Result<u64, SimulationError> {
        if !self
            .beer_batch(brewery, slot)?
            .is_some_and(|batch| batch.ready(self.current_time_ms()))
        {
            return Err(SimulationError::MissingInputs);
        }
        self.brewing_labor_duration(COLLECTION_MS)
    }
    pub(crate) fn collect_beer(
        &mut self,
        brewery: PlaceId,
        slot: usize,
    ) -> Result<(), SimulationError> {
        if !self
            .beer_batch(brewery, slot)?
            .is_some_and(|batch| batch.ready(self.current_time_ms()))
        {
            return Err(SimulationError::MissingInputs);
        }
        self.set_inventory_units(
            Good::Beer,
            self.units(Good::Beer)
                .checked_add(BATCH_BEER_GRAMS)
                .ok_or(SimulationError::InventoryOverflow)?,
        );
        self.map.set_batch(brewery, slot, None)
    }
    pub fn brewing_actions_at(&self, brewery: PlaceId) -> Vec<CitizenAction> {
        let Ok(place) = self.map.place(brewery) else {
            return Vec::new();
        };
        if place.owner != Some(self.id) || place.kind != Location::Brewery {
            return Vec::new();
        }
        place
            .batches
            .iter()
            .enumerate()
            .filter_map(|(slot, batch)| {
                batch
                    .filter(|batch| batch.ready(self.current_time_ms()))
                    .map(|_| CitizenAction::CollectBeer { brewery, slot })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentKind, StartingRole, Universe, locations::Map, persistence::SaveError};

    #[test]
    fn prediction_counts_batches_once_and_never_reserves_consumed_inputs() {
        let mut worker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Brewer);
        for &(good, units) in Recipe::BrewBeer.inputs() {
            worker = worker.with_good(good, units * 4).unwrap();
        }
        let (map, site) = worker
            .map()
            .with_place(
                Location::Brewery,
                crate::locations::Position::default(),
                Some(worker.id()),
                "Brewery",
            )
            .unwrap();
        worker = worker.with_map(map).unwrap();
        worker.production_targets = Some(
            crate::production::ProductionTargets::calculate_at(&worker, 0, Some(site)).unwrap(),
        );
        let source = worker.clone();
        let action = CitizenAction::ProduceAt {
            place: site,
            recipe: Recipe::BrewBeer,
        };
        let started = worker.start_action(action).unwrap();
        assert_eq!(source, worker);
        assert!(
            worker
                .map
                .place(site)
                .unwrap()
                .batches
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(started.reserved_goods().unwrap().units(Good::Wheat), 600);
        assert_eq!(
            crate::production::production_stock(Good::Beer, &started),
            1000.0
        );
        let predicted = started.advance_predicted(PREPARATION_MS).unwrap();
        let actual = started.advance(PREPARATION_MS).unwrap();
        assert_eq!(predicted, actual);
        assert_eq!(predicted.units(Good::Beer), 0);
        assert_eq!(
            crate::production::production_stock(Good::Beer, &predicted),
            1000.0
        );
        let mut worker = predicted;
        for _ in 1..4 {
            let duration = worker.action_duration_ms(action).unwrap();
            worker = worker
                .start_action(action)
                .unwrap()
                .advance_predicted(duration)
                .unwrap();
        }
        worker.production_targets = Some(
            crate::production::ProductionTargets::calculate_at(
                &worker,
                worker.current_time_ms(),
                Some(site),
            )
            .unwrap(),
        );
        let shortages = worker.purchase_shortages().unwrap();
        for good in [Good::Wheat, Good::Hops, Good::Wood] {
            assert_eq!(shortages.units(good), 0);
        }
    }

    #[test]
    fn saves_reject_orphan_preparation_and_invalid_slots_or_timestamps() {
        let worker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Brewer);
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen("Brewer", worker)
            .unwrap();
        let (world, site) = world.with_property(id, Location::Brewery).unwrap();
        for batch in [Batch::Preparing, Batch::Fermenting { ready_at_ms: 1 }] {
            let mut malformed = world.clone();
            malformed.map.set_batch(site, 0, Some(batch)).unwrap();
            assert!(matches!(
                Universe::from_save_str(&malformed.to_save_string().unwrap()),
                Err(SaveError::InvalidState(_))
            ));
        }
        let mut malformed = world.clone();
        malformed
            .map
            .places_mut()
            .get_mut(&site)
            .unwrap()
            .batches
            .pop();
        assert!(matches!(
            Universe::from_save_str(&malformed.to_save_string().unwrap()),
            Err(SaveError::InvalidState(_))
        ));
        let mut worker = match &world.agents()[&id].kind {
            AgentKind::Citizen(worker) => worker.clone(),
        };
        for &(good, units) in Recipe::BrewBeer.inputs() {
            worker = worker.with_good(good, units).unwrap();
        }
        let mut started = worker
            .start_action(CitizenAction::ProduceAt {
                place: site,
                recipe: Recipe::BrewBeer,
            })
            .unwrap();
        started.map.set_batch(site, 0, None).unwrap();
        let mut malformed = world.clone();
        malformed.map = started.map.clone();
        malformed.agents.get_mut(&id).unwrap().kind = AgentKind::Citizen(started);
        assert!(matches!(
            Universe::from_save_str(&malformed.to_save_string().unwrap()),
            Err(SaveError::InvalidState(_))
        ));
    }
}
