use crate::{AgentId, Quantity, SimulationError, locations::PlaceId, marketplace::Good};
use imbl::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GoodsOwner {
    Agent(AgentId),
    Town,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StockKey {
    pub place: PlaceId,
    pub owner: GoodsOwner,
    pub good: Good,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Storage {
    stock: HashMap<StockKey, Quantity>,
    totals: HashMap<(GoodsOwner, Good), Quantity>,
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

    pub(crate) fn set_units(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
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
