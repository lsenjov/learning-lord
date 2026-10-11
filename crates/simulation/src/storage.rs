use crate::{AgentId, Quantity, SimulationError, locations::PlaceId, marketplace::Good};
use imbl::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GoodsOwner {
    Agent(AgentId),
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
        if units < self.reserved_units(key.place, key.owner, key.good) {
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
        if totals != self.totals
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
                if key.owner != GoodsOwner::Agent(self.id)
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
        let owner = GoodsOwner::Agent(self.id);
        self.storage
            .units(place, owner, good)
            .saturating_sub(self.storage.reserved_units(place, owner, good))
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
                self.available_units(good)
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
                owner: GoodsOwner::Agent(self.id),
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

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::{
        AgentKind, Citizen, CitizenAction, TRADE_DURATION_MS, Universe, marketplace::ShoppingList,
    };

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
