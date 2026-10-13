use crate::{AgentId, Quantity, SimulationError, locations::PlaceId, marketplace::Good};
use imbl::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GoodsOwner {
    Agent(AgentId),
    Household(AgentId),
    Town,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StockKey {
    pub place: PlaceId,
    pub owner: GoodsOwner,
    pub good: Good,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Storage {
    stock: HashMap<StockKey, Quantity>,
    totals: HashMap<(GoodsOwner, Good), Quantity>,
    reserved: HashMap<StockKey, Quantity>,
    work_reserved: HashMap<StockKey, Quantity>,
}

impl Storage {
    pub fn stock(&self) -> &HashMap<StockKey, Quantity> {
        &self.stock
    }

    pub fn units(&self, place: PlaceId, owner: GoodsOwner, good: Good) -> Quantity {
        self.stock
            .get(&StockKey { place, owner, good })
            .copied()
            .unwrap_or(0)
    }

    pub fn owned_units(&self, owner: GoodsOwner, good: Good) -> Quantity {
        self.totals.get(&(owner, good)).copied().unwrap_or(0)
    }

    pub fn reserved_units(&self, place: PlaceId, owner: GoodsOwner, good: Good) -> Quantity {
        self.reserved
            .get(&StockKey { place, owner, good })
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn work_reservations(&self) -> &HashMap<StockKey, Quantity> {
        &self.work_reserved
    }
    pub(crate) fn work_reserved_units(&self, key: StockKey) -> Quantity {
        self.work_reserved.get(&key).copied().unwrap_or(0)
    }
    pub(crate) fn reserve_work(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let current = self.work_reserved_units(key);
        let reserved = current
            .checked_add(units)
            .ok_or(SimulationError::InventoryOverflow)?;
        if reserved
            > self
                .units(key.place, key.owner, key.good)
                .saturating_sub(self.reserved_units(key.place, key.owner, key.good))
        {
            return Err(SimulationError::MissingInputs);
        }
        if reserved == 0 {
            self.work_reserved.remove(&key);
        } else {
            self.work_reserved.insert(key, reserved);
        }
        Ok(())
    }
    pub(crate) fn release_work(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let reserved = self
            .work_reserved_units(key)
            .checked_sub(units)
            .ok_or(SimulationError::MissingInputs)?;
        if reserved == 0 {
            self.work_reserved.remove(&key);
        } else {
            self.work_reserved.insert(key, reserved);
        }
        Ok(())
    }

    pub(crate) fn set_reserved(&mut self, key: StockKey, units: Quantity) {
        if units == 0 {
            self.reserved.remove(&key);
        } else {
            self.reserved.insert(key, units);
        }
    }

    pub(crate) fn set_units(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        if units
            < self
                .reserved_units(key.place, key.owner, key.good)
                .saturating_add(self.work_reserved_units(key))
        {
            return Err(SimulationError::MissingInputs);
        }
        let previous = self.units(key.place, key.owner, key.good);
        let total = self
            .owned_units(key.owner, key.good)
            .checked_sub(previous)
            .and_then(|remaining| remaining.checked_add(units))
            .ok_or(SimulationError::InventoryOverflow)?;
        if units == 0 {
            self.stock.remove(&key);
        } else {
            self.stock.insert(key, units);
        }
        if total == 0 {
            self.totals.remove(&(key.owner, key.good));
        } else {
            self.totals.insert((key.owner, key.good), total);
        }
        Ok(())
    }
}

impl Storage {
    pub(crate) fn validate_save(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        let mut totals = HashMap::new();
        for (key, units) in &self.stock {
            let total = totals.entry((key.owner, key.good)).or_insert(0_u64);
            *total = total
                .checked_add(*units)
                .ok_or(SaveError::InvalidState("storage total overflow"))?;
        }
        if self.work_reserved.iter().any(|(key, units)| {
            units
                .checked_add(self.reserved_units(key.place, key.owner, key.good))
                .is_none_or(|total| total > self.units(key.place, key.owner, key.good))
        }) || totals != self.totals
            || self
                .reserved
                .iter()
                .any(|(key, units)| *units > self.stock.get(key).copied().unwrap_or(0))
        {
            return Err(SaveError::InvalidState(
                "inconsistent storage totals or reservations",
            ));
        }
        Ok(())
    }
}

impl crate::Citizen {
    pub(crate) fn stored_sources(&self, good: Good) -> Vec<(PlaceId, Quantity)> {
        let mut sources: Vec<_> = self
            .storage
            .stock()
            .iter()
            .filter_map(|(key, _)| {
                if key.owner != self.goods_owner()
                    || key.good != good
                    || self.accessible_place(key.place).is_err()
                {
                    return None;
                }
                let units = self.stored_available_units(key.place, good);
                (units > 0).then_some((key.place, units))
            })
            .collect();
        sources.sort_by(|(a, _), (b, _)| {
            self.position
                .distance(self.map.position(*a))
                .total_cmp(&self.position.distance(self.map.position(*b)))
                .then_with(|| a.0.cmp(&b.0))
        });
        sources
    }

    pub fn stored_available_units(&self, place: PlaceId, good: Good) -> Quantity {
        let owner = self.goods_owner();
        self.storage
            .units(place, owner, good)
            .saturating_sub(self.storage.reserved_units(place, owner, good))
            .saturating_sub(
                self.storage
                    .work_reserved_units(StockKey { place, owner, good }),
            )
    }

    pub(crate) fn transfer_stored_action(
        &mut self,
        place: PlaceId,
        list: crate::marketplace::ShoppingList,
        deposit: bool,
    ) -> Result<(), SimulationError> {
        self.accessible_place(place)?;
        for (good, units) in list.items() {
            let available = if deposit {
                self.carried_available_units(good)
            } else {
                self.stored_available_units(place, good)
            };
            if units > available {
                return Err(SimulationError::MissingInputs);
            }
        }
        for (good, units) in list.items() {
            let key = StockKey {
                place,
                owner: self.goods_owner(),
                good,
            };
            let stored = self.storage.units(place, key.owner, good);
            let carried = self.units(good);
            let (next_stored, next_carried) = if deposit {
                (
                    stored
                        .checked_add(units)
                        .ok_or(SimulationError::InventoryOverflow)?,
                    carried - units,
                )
            } else {
                (
                    stored - units,
                    carried
                        .checked_add(units)
                        .ok_or(SimulationError::InventoryOverflow)?,
                )
            };
            self.storage.set_units(key, next_stored)?;
            self.set_inventory_units(good, next_carried);
            if !deposit {
                self.retain_production_input(good, units)?;
            }
        }
        Ok(())
    }
}

impl crate::Citizen {
    fn accessible_colocated_storage_places(&self) -> impl Iterator<Item = PlaceId> + '_ {
        self.map
            .places()
            .values()
            .filter(|place| {
                self.position.distance(place.position) < 0.001
                    && self.accessible_place(place.id).is_ok()
            })
            .map(|place| place.id)
    }

    pub(crate) fn consumption_available_units(&self) -> [Quantity; Good::COUNT] {
        let mut available = [0; Good::COUNT];
        for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
            available[good as usize] = self.carried_available_units(good);
        }
        for place in self.accessible_colocated_storage_places() {
            for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
                available[good as usize] = available[good as usize]
                    .saturating_add(self.stored_available_units(place, good));
            }
        }
        available
    }

    pub(crate) fn colocated_storage_places(&self) -> Vec<PlaceId> {
        let mut places: Vec<_> = self.accessible_colocated_storage_places().collect();
        places.sort_by_key(|place| {
            (
                self.active_action
                    .and_then(|active| active.action.required_place(self))
                    != Some(*place),
                self.designated_workplace != Some(*place),
                self.home != *place,
                place.0,
            )
        });
        places
    }
    pub(crate) fn current_storage_place(&self) -> Option<PlaceId> {
        self.colocated_storage_places().first().copied()
    }
    pub(crate) fn carried_available_units(&self, good: Good) -> Quantity {
        self.units(good)
            .saturating_sub(self.tax_reserved_units(good))
    }
    pub(crate) fn consume_goods(
        &mut self,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let carried = units.min(self.carried_available_units(good));
        let mut stored = units - carried;
        let places = self.colocated_storage_places();
        if stored
            > places
                .iter()
                .map(|place| self.stored_available_units(*place, good))
                .fold(0_u64, u64::saturating_add)
        {
            return Err(SimulationError::MissingInputs);
        }
        for place in places {
            let taken = stored.min(self.stored_available_units(place, good));
            if taken == 0 {
                continue;
            }
            let key = StockKey {
                place,
                owner: self.goods_owner(),
                good,
            };
            self.storage
                .set_units(key, self.storage.units(place, key.owner, good) - taken)?;
            stored -= taken;
        }
        self.set_inventory_units(good, self.units(good) - carried);
        Ok(())
    }
    pub(crate) fn receive_production_output(
        &mut self,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        if self.map.place(place)?.kind.is_public() {
            self.set_inventory_units(
                good,
                self.units(good)
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?,
            );
        } else {
            let key = StockKey {
                place,
                owner: self.goods_owner(),
                good,
            };
            self.storage.set_units(
                key,
                self.storage
                    .units(place, key.owner, good)
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?,
            )?;
        }
        Ok(())
    }
    pub(crate) fn resource_units(&self, good: Good) -> Result<Quantity, SimulationError> {
        self.units(good)
            .checked_add(self.storage.owned_units(self.goods_owner(), good))
            .ok_or(SimulationError::InventoryOverflow)
    }
}

impl crate::Citizen {
    pub(crate) fn consume_at(
        &mut self,
        place: PlaceId,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let carried = units.min(self.carried_available_units(good));
        let stored = units - carried;
        if stored > self.stored_available_units(place, good) {
            return Err(SimulationError::MissingInputs);
        }
        if stored > 0 {
            let key = StockKey {
                place,
                owner: self.goods_owner(),
                good,
            };
            self.storage
                .set_units(key, self.storage.units(place, key.owner, good) - stored)?;
        }
        self.set_inventory_units(good, self.units(good) - carried);
        Ok(())
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::{
        AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe, marketplace::ShoppingList,
    };

    #[test]
    fn consumption_availability_matches_individual_counts_with_access_and_reservations() {
        use crate::locations::{Location, Position};

        for shared in [false, true] {
            let mut citizen = Citizen::new(80.0).unwrap();
            let other = AgentId(uuid::Uuid::new_v4());
            if shared {
                citizen.household = Some(crate::households::HouseholdContext {
                    id: other,
                    members: vec![citizen.id, other],
                    allegiance: 0.0,
                    other_carried: [0; Good::COUNT],
                    other_consumption_value: 0.0,
                });
            }
            let owner = citizen.goods_owner();
            let (map, other_home) = citizen
                .map
                .with_place(Location::Home, citizen.position, Some(other), "Other home")
                .unwrap();
            let (map, distant) = map
                .with_place(
                    Location::Home,
                    Position { x: 10.0, y: 0.0 },
                    Some(citizen.id),
                    "Distant home",
                )
                .unwrap();
            citizen.map = map;
            for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
                citizen.inventory.insert(good, 20);
                citizen.tax_reserved.insert(good, 3);
                let home_key = StockKey {
                    place: citizen.home,
                    owner,
                    good,
                };
                citizen.storage.set_units(home_key, 30).unwrap();
                citizen.storage.set_reserved(home_key, 5);
                citizen.storage.reserve_work(home_key, 7).unwrap();
                for (place, stock_owner, units) in [
                    (other_home, owner, 11),
                    (distant, owner, 100),
                    (citizen.home, GoodsOwner::Town, 200),
                    (
                        citizen.home,
                        GoodsOwner::Agent(AgentId(uuid::Uuid::new_v4())),
                        300,
                    ),
                ] {
                    citizen
                        .storage
                        .set_units(
                            StockKey {
                                place,
                                owner: stock_owner,
                                good,
                            },
                            units,
                        )
                        .unwrap();
                }
            }
            let available = citizen.consumption_available_units();
            for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
                assert_eq!(available[good as usize], citizen.available_units(good));
                assert_eq!(available[good as usize], if shared { 46 } else { 35 });
            }
            citizen.inventory.insert(Good::Water, Quantity::MAX);
            citizen.tax_reserved.remove(&Good::Water);
            assert_eq!(
                citizen.consumption_available_units()[Good::Water as usize],
                Quantity::MAX
            );
            assert_eq!(citizen.available_units(Good::Water), Quantity::MAX);
        }
    }

    #[test]
    fn planned_storage_transfers_conserve_stock_and_leave_snapshots_unchanged() {
        let (world, id) = Universe::default()
            .with_citizen(
                "Ada",
                Citizen::new(0.0)
                    .unwrap()
                    .with_good(Good::Wheat, 100)
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
        let home = citizen.home();
        let action = CitizenAction::DepositStored {
            place: home,
            list: ShoppingList::single(Good::Wheat, 70),
        };
        let deposited = world
            .start_action(id, action)
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let AgentKind::Citizen(after) = &deposited.agents()[&id].kind;
        assert_eq!(after.units(Good::Wheat), 30);
        assert_eq!(
            deposited
                .storage()
                .units(home, GoodsOwner::Agent(id), Good::Wheat),
            70
        );
        assert_eq!(citizen.units(Good::Wheat), 100);
        assert_eq!(
            world
                .storage()
                .owned_units(GoodsOwner::Agent(id), Good::Wheat),
            0
        );
        let predicted = after
            .start_action(CitizenAction::CollectStored {
                place: home,
                list: ShoppingList::single(Good::Wheat, 20),
            })
            .unwrap()
            .advance_predicted(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(predicted.units(Good::Wheat), 50);
        assert_eq!(predicted.stored_available_units(home, Good::Wheat), 50);
        assert_eq!(after.stored_available_units(home, Good::Wheat), 70);
    }

    #[test]
    fn storage_transport_cannot_take_tax_reserved_stock() {
        let mut citizen = Citizen::new(0.0).unwrap();
        let place = citizen.home();
        let key = StockKey {
            place,
            owner: GoodsOwner::Agent(citizen.id()),
            good: Good::Wheat,
        };
        citizen.storage.set_units(key, 100).unwrap();
        citizen.storage.set_reserved(key, 40);
        assert_eq!(
            citizen.start_action(CitizenAction::CollectStored {
                place,
                list: ShoppingList::single(Good::Wheat, 61)
            }),
            Err(SimulationError::MissingInputs)
        );
        let collected = citizen
            .start_action(CitizenAction::CollectStored {
                place,
                list: ShoppingList::single(Good::Wheat, 60),
            })
            .unwrap()
            .advance_predicted(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(collected.storage.units(place, key.owner, Good::Wheat), 40);
        assert_eq!(
            collected
                .storage
                .reserved_units(place, key.owner, Good::Wheat),
            40
        );
    }
}
