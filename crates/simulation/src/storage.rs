use crate::{
    AgentId, Quantity, SimulationError,
    locations::{Map, ObjectId, PlaceId},
    marketplace::{Good, Market},
};
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
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObjectStockKey {
    pub stock: StockKey,
    pub object: ObjectId,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Storage {
    contents: HashMap<ObjectStockKey, Quantity>,
    stock: HashMap<StockKey, Quantity>,
    totals: HashMap<(GoodsOwner, Good), Quantity>,
    reserved: HashMap<StockKey, Quantity>,
    tax_allocations: HashMap<ObjectStockKey, Quantity>,
    work_reserved: HashMap<StockKey, Quantity>,
    work_allocations: HashMap<(AgentId, ObjectStockKey), Quantity>,
}
impl Storage {
    pub fn stock(&self) -> &HashMap<StockKey, Quantity> {
        &self.stock
    }
    pub fn contents(&self) -> &HashMap<ObjectStockKey, Quantity> {
        &self.contents
    }
    pub fn units(&self, place: PlaceId, owner: GoodsOwner, good: Good) -> Quantity {
        self.stock
            .get(&StockKey { place, owner, good })
            .copied()
            .unwrap_or(0)
    }
    pub fn object_units(&self, key: ObjectStockKey) -> Quantity {
        self.contents.get(&key).copied().unwrap_or(0)
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
    pub fn object_tax_reserved(&self, key: ObjectStockKey) -> Quantity {
        self.tax_allocations.get(&key).copied().unwrap_or(0)
    }
    pub fn object_work_reserved(&self, key: ObjectStockKey) -> Quantity {
        self.work_allocations
            .iter()
            .filter(|((_, stock), _)| *stock == key)
            .map(|(_, units)| *units)
            .fold(0, Quantity::saturating_add)
    }
    fn object_available(&self, key: ObjectStockKey) -> Quantity {
        self.object_units(key)
            .saturating_sub(self.object_tax_reserved(key))
            .saturating_sub(self.object_work_reserved(key))
    }
    pub(crate) fn work_allocations(&self) -> &HashMap<(AgentId, ObjectStockKey), Quantity> {
        &self.work_allocations
    }
    pub(crate) fn work_reserved_units(&self, key: StockKey) -> Quantity {
        self.work_reserved.get(&key).copied().unwrap_or(0)
    }
    fn keys(&self, key: StockKey) -> Vec<ObjectStockKey> {
        let mut keys: Vec<_> = self
            .contents
            .keys()
            .filter(|entry| entry.stock == key)
            .copied()
            .collect();
        keys.sort_by_key(|entry| entry.object.0);
        keys
    }
    pub(crate) fn reserve_work_for(
        &mut self,
        worker: AgentId,
        key: StockKey,
        units: Quantity,
        allowed: &[ObjectId],
    ) -> Result<(), SimulationError> {
        let keys: Vec<_> = self
            .keys(key)
            .into_iter()
            .filter(|entry| allowed.contains(&entry.object))
            .collect();
        if units
            > keys
                .iter()
                .map(|entry| self.object_available(*entry))
                .fold(0_u64, u64::saturating_add)
        {
            return Err(SimulationError::MissingInputs);
        }
        let mut remaining = units;
        for entry in keys {
            let taken = remaining.min(self.object_available(entry));
            if taken > 0 {
                let held = self
                    .work_allocations
                    .get(&(worker, entry))
                    .copied()
                    .unwrap_or(0);
                self.work_allocations.insert((worker, entry), held + taken);
                remaining -= taken;
            }
        }
        let total = self
            .work_reserved_units(key)
            .checked_add(units)
            .ok_or(SimulationError::InventoryOverflow)?;
        if total > 0 {
            self.work_reserved.insert(key, total);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn reserve_work(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let allowed: Vec<_> = self
            .keys(key)
            .into_iter()
            .map(|entry| entry.object)
            .collect();
        self.reserve_work_for(AgentId(uuid::Uuid::nil()), key, units, &allowed)
    }
    pub(crate) fn release_work_for(
        &mut self,
        worker: AgentId,
        key: StockKey,
        units: Quantity,
        consume: bool,
    ) -> Result<(), SimulationError> {
        let mut keys: Vec<_> = self
            .work_allocations
            .iter()
            .filter(|((id, entry), _)| *id == worker && entry.stock == key)
            .map(|((_, entry), units)| (*entry, *units))
            .collect();
        keys.sort_by_key(|(entry, _)| entry.object.0);
        if units > keys.iter().map(|(_, held)| *held).sum() {
            return Err(SimulationError::MissingInputs);
        }
        let mut remaining = units;
        for (entry, held) in keys {
            let taken = remaining.min(held);
            if taken == 0 {
                continue;
            }
            if taken == held {
                self.work_allocations.remove(&(worker, entry));
            } else {
                self.work_allocations.insert((worker, entry), held - taken);
            }
            if consume {
                self.change_object(entry, self.object_units(entry) - taken)?;
            }
            remaining -= taken;
        }
        let total = self.work_reserved_units(key) - units;
        if total == 0 {
            self.work_reserved.remove(&key);
        } else {
            self.work_reserved.insert(key, total);
        }
        Ok(())
    }
    pub(crate) fn set_reserved(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        self.set_reserved_in(key, units, None)
    }
    pub(crate) fn set_reserved_in(
        &mut self,
        key: StockKey,
        units: Quantity,
        allowed: Option<&[ObjectId]>,
    ) -> Result<(), SimulationError> {
        let current = self.reserved_units(key.place, key.owner, key.good);
        let mut remaining = units.abs_diff(current);
        let keys: Vec<_> = self
            .keys(key)
            .into_iter()
            .filter(|entry| allowed.is_none_or(|ids| ids.contains(&entry.object)))
            .collect();
        if units < current
            && remaining
                > keys
                    .iter()
                    .map(|entry| self.object_tax_reserved(*entry))
                    .fold(0_u64, Quantity::saturating_add)
        {
            return Err(SimulationError::MissingInputs);
        }
        if units > current
            && remaining
                > keys
                    .iter()
                    .map(|entry| self.object_available(*entry))
                    .fold(0_u64, u64::saturating_add)
        {
            return Err(SimulationError::MissingInputs);
        }
        for entry in keys {
            let held = self.object_tax_reserved(entry);
            let taken = remaining.min(if units > current {
                self.object_available(entry)
            } else {
                held
            });
            let next = if units > current {
                held + taken
            } else {
                held - taken
            };
            if next == 0 {
                self.tax_allocations.remove(&entry);
            } else {
                self.tax_allocations.insert(entry, next);
            }
            remaining -= taken;
        }
        if units == 0 {
            self.reserved.remove(&key);
        } else {
            self.reserved.insert(key, units);
        }
        Ok(())
    }
    pub(crate) fn object_good(&self, id: ObjectId, market: &Market) -> Option<Good> {
        self.contents
            .keys()
            .find(|entry| entry.object == id)
            .map(|entry| entry.stock.good)
            .or_else(|| {
                market
                    .orders()
                    .find(|order| market.order_container(order.id) == Some(id))
                    .map(|order| order.good)
            })
    }
    pub(crate) fn select_object(
        &self,
        place: PlaceId,
        good: Good,
        map: &Map,
        market: &Market,
        allowed: Option<&[ObjectId]>,
    ) -> Result<ObjectId, SimulationError> {
        self.select_object_with_claims(place, good, map, market, allowed, &HashMap::new())
    }
    pub(crate) fn select_object_with_claims(
        &self,
        place: PlaceId,
        good: Good,
        map: &Map,
        market: &Market,
        allowed: Option<&[ObjectId]>,
        claims: &HashMap<ObjectId, Good>,
    ) -> Result<ObjectId, SimulationError> {
        let mut objects: Vec<_> = map
            .place(place)?
            .objects()
            .filter(|object| {
                object.kind.accepts(good) && allowed.is_none_or(|ids| ids.contains(&object.id))
            })
            .filter(|object| {
                !object.kind.single_good()
                    || self
                        .object_good(object.id, market)
                        .or_else(|| claims.get(&object.id).copied())
                        .is_none_or(|held| held == good)
            })
            .collect();
        objects.sort_by_key(|object| {
            (
                claims.get(&object.id) != Some(&good)
                    && !self
                        .contents
                        .keys()
                        .any(|entry| entry.object == object.id && entry.stock.good == good)
                    && !market.orders().any(|order| {
                        market.order_container(order.id) == Some(object.id) && order.good == good
                    }),
                object.id.0,
            )
        });
        objects
            .first()
            .map(|object| object.id)
            .ok_or(SimulationError::MissingInputs)
    }
    pub(crate) fn set_units(
        &mut self,
        key: StockKey,
        units: Quantity,
        map: &Map,
        market: &Market,
    ) -> Result<(), SimulationError> {
        self.set_units_in(key, units, map, market, None)
    }
    pub(crate) fn set_units_in(
        &mut self,
        key: StockKey,
        units: Quantity,
        map: &Map,
        market: &Market,
        allowed: Option<&[ObjectId]>,
    ) -> Result<(), SimulationError> {
        let previous = self.units(key.place, key.owner, key.good);
        if units >= previous {
            if units == previous {
                return Ok(());
            }
            let object = self.select_object(key.place, key.good, map, market, allowed)?;
            let entry = ObjectStockKey { stock: key, object };
            let next = self
                .object_units(entry)
                .checked_add(units - previous)
                .ok_or(SimulationError::InventoryOverflow)?;
            self.change_object(entry, next)
        } else {
            let keys: Vec<_> = self
                .keys(key)
                .into_iter()
                .filter(|entry| allowed.is_none_or(|ids| ids.contains(&entry.object)))
                .collect();
            let mut remaining = previous - units;
            if remaining
                > keys
                    .iter()
                    .map(|entry| self.object_available(*entry))
                    .fold(0_u64, u64::saturating_add)
            {
                return Err(SimulationError::MissingInputs);
            }
            for entry in keys {
                let taken = remaining.min(self.object_available(entry));
                if taken > 0 {
                    self.change_object(entry, self.object_units(entry) - taken)?;
                    remaining -= taken;
                }
            }
            Ok(())
        }
    }
    fn change_object(
        &mut self,
        key: ObjectStockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let previous = self.object_units(key);
        let aggregate = self
            .units(key.stock.place, key.stock.owner, key.stock.good)
            .checked_sub(previous)
            .and_then(|value| value.checked_add(units))
            .ok_or(SimulationError::InventoryOverflow)?;
        let total = self
            .owned_units(key.stock.owner, key.stock.good)
            .checked_sub(previous)
            .and_then(|value| value.checked_add(units))
            .ok_or(SimulationError::InventoryOverflow)?;
        if units == 0 {
            self.contents.remove(&key);
        } else {
            self.contents.insert(key, units);
        }
        if aggregate == 0 {
            self.stock.remove(&key.stock);
        } else {
            self.stock.insert(key.stock, aggregate);
        }
        if total == 0 {
            self.totals.remove(&(key.stock.owner, key.stock.good));
        } else {
            self.totals.insert((key.stock.owner, key.stock.good), total);
        }
        Ok(())
    }
    pub(crate) fn reassign_owner(
        &mut self,
        key: StockKey,
        owner: GoodsOwner,
    ) -> Result<(), SimulationError> {
        if self.reserved_units(key.place, key.owner, key.good) > 0
            || self.work_reserved_units(key) > 0
        {
            return Err(SimulationError::MissingInputs);
        }
        for entry in self.keys(key) {
            let units = self.object_units(entry);
            let target = ObjectStockKey {
                stock: StockKey { owner, ..key },
                ..entry
            };
            let next = self
                .object_units(target)
                .checked_add(units)
                .ok_or(SimulationError::InventoryOverflow)?;
            self.change_object(entry, 0)?;
            self.change_object(target, next)?;
        }
        Ok(())
    }
    pub(crate) fn settle_tax(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let mut remaining = units;
        for entry in self.keys(key) {
            let taken = remaining.min(self.object_tax_reserved(entry));
            if taken == 0 {
                continue;
            }
            let held = self.object_tax_reserved(entry) - taken;
            if held == 0 {
                self.tax_allocations.remove(&entry);
            } else {
                self.tax_allocations.insert(entry, held);
            }
            self.change_object(entry, self.object_units(entry) - taken)?;
            let town = ObjectStockKey {
                stock: StockKey {
                    owner: GoodsOwner::Town,
                    ..key
                },
                ..entry
            };
            self.change_object(
                town,
                self.object_units(town)
                    .checked_add(taken)
                    .ok_or(SimulationError::InventoryOverflow)?,
            )?;
            remaining -= taken;
        }
        if remaining > 0 {
            return Err(SimulationError::MissingInputs);
        }
        let reserved = self.reserved_units(key.place, key.owner, key.good) - units;
        if reserved == 0 {
            self.reserved.remove(&key);
        } else {
            self.reserved.insert(key, reserved);
        }
        Ok(())
    }
    pub(crate) fn validate_save(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        let invalid = || SaveError::InvalidState("inconsistent physical storage or reservations");
        let mut stock = HashMap::new();
        let mut totals = HashMap::new();
        for (key, units) in &self.contents {
            if *units == 0
                || self
                    .object_tax_reserved(*key)
                    .checked_add(self.object_work_reserved(*key))
                    .is_none_or(|held| held > *units)
            {
                return Err(invalid());
            }
            let aggregate = stock.entry(key.stock).or_insert(0_u64);
            *aggregate = aggregate.checked_add(*units).ok_or_else(invalid)?;
            let total = totals
                .entry((key.stock.owner, key.stock.good))
                .or_insert(0_u64);
            *total = total.checked_add(*units).ok_or_else(invalid)?;
        }
        let mut tax = HashMap::new();
        let mut work = HashMap::new();
        for (key, units) in &self.tax_allocations {
            if *units == 0 || !self.contents.contains_key(key) {
                return Err(invalid());
            }
            let held = tax.entry(key.stock).or_insert(0_u64);
            *held = held.checked_add(*units).ok_or_else(invalid)?;
        }
        for ((_, key), units) in &self.work_allocations {
            if *units == 0 || !self.contents.contains_key(key) {
                return Err(invalid());
            }
            let held = work.entry(key.stock).or_insert(0_u64);
            *held = held.checked_add(*units).ok_or_else(invalid)?;
        }
        if stock != self.stock
            || totals != self.totals
            || tax != self.reserved
            || work != self.work_reserved
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn validate_objects(
        &self,
        map: &Map,
        market: &Market,
    ) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        let mut held: HashMap<ObjectId, Good> = HashMap::new();
        for (place, object, good) in self
            .contents
            .keys()
            .map(|key| (key.stock.place, key.object, key.stock.good))
            .chain(market.orders().map(|order| {
                (
                    order.place,
                    market
                        .order_container(order.id)
                        .unwrap_or(ObjectId(uuid::Uuid::nil())),
                    order.good,
                )
            }))
        {
            let object = map
                .place(place)
                .ok()
                .and_then(|place| place.object(object))
                .ok_or(SaveError::InvalidState("missing storage object"))?;
            if !object.kind.accepts(good)
                || (object.kind.single_good()
                    && held.get(&object.id).is_some_and(|prior| *prior != good))
            {
                return Err(SaveError::InvalidState("incompatible storage contents"));
            }
            if object.kind.single_good() {
                held.insert(object.id, good);
            }
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

    pub(crate) fn accessible_storage_objects(&self, place: PlaceId) -> Vec<ObjectId> {
        self.map
            .place(place)
            .ok()
            .into_iter()
            .flat_map(|site| {
                site.objects()
                    .filter(|object| object.kind.is_storage() && self.may_use_object(site, object))
                    .map(|object| object.id)
            })
            .collect()
    }
    pub(crate) fn listing_baskets(
        &self,
        list: crate::marketplace::ShoppingList,
    ) -> Result<Vec<(PlaceId, crate::marketplace::ShoppingList)>, SimulationError> {
        let preferred = self.selling_place();
        let market = self.map.public_place(crate::locations::Location::Market);
        let places = if preferred == market {
            vec![preferred]
        } else {
            vec![preferred, market]
        };
        let mut baskets = vec![Vec::new(); places.len()];
        let mut claims = HashMap::new();
        for (good, units) in list.items() {
            let mut destination = None;
            for (index, place) in places.iter().enumerate() {
                let allowed = self.accessible_storage_objects(*place);
                if let Ok(object) = self.storage.select_object_with_claims(
                    *place,
                    good,
                    &self.map,
                    &self.market,
                    Some(&allowed),
                    &claims,
                ) {
                    claims.insert(object, good);
                    destination = Some(index);
                    break;
                }
            }
            baskets[destination.ok_or(SimulationError::MissingInputs)?].push((good, units));
        }
        places
            .into_iter()
            .zip(baskets)
            .filter(|(_, goods)| !goods.is_empty())
            .map(|(place, goods)| Ok((place, crate::marketplace::ShoppingList::new(goods)?)))
            .collect()
    }
    pub(crate) fn storage_object(
        &self,
        place: PlaceId,
        good: Good,
    ) -> Result<ObjectId, SimulationError> {
        let allowed = self.accessible_storage_objects(place);
        self.storage
            .select_object(place, good, &self.map, &self.market, Some(&allowed))
    }
    pub(crate) fn set_stored_units(
        &mut self,
        key: StockKey,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let allowed = self.accessible_storage_objects(key.place);
        self.storage
            .set_units_in(key, units, &self.map, &self.market, Some(&allowed))
    }
    pub fn stored_available_units(&self, place: PlaceId, good: Good) -> Quantity {
        let stock = StockKey {
            place,
            owner: self.goods_owner(),
            good,
        };
        self.map
            .place(place)
            .ok()
            .into_iter()
            .flat_map(|site| {
                site.objects()
                    .filter(|object| object.kind.accepts(good) && self.may_use_object(site, object))
                    .map(|object| {
                        self.storage.object_available(ObjectStockKey {
                            stock,
                            object: object.id,
                        })
                    })
            })
            .fold(0, Quantity::saturating_add)
    }

    pub(crate) fn transfer_stored_action(
        &mut self,
        place: PlaceId,
        list: crate::marketplace::ShoppingList,
        deposit: bool,
    ) -> Result<(), SimulationError> {
        let mut next = self.clone();
        next.transfer_stored_inner(place, list, deposit)?;
        *self = next;
        Ok(())
    }
    fn transfer_stored_inner(
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
            if deposit && units > 0 {
                self.storage_object(place, good)?;
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
            self.set_stored_units(key, next_stored)?;
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

    pub(crate) fn home_food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| {
                self.stored_available_units(self.home, good) as f64
                    * good.nutrition_per_unit().unwrap()
            })
            .sum::<f64>()
            + self.stored_available_units(self.home, Good::Beer) as f64 * 0.05
    }
    pub(crate) fn home_drink_units(&self) -> Quantity {
        self.stored_available_units(self.home, Good::Water)
            .saturating_add(self.stored_available_units(self.home, Good::Beer))
    }
    pub(crate) fn home_meal_nutrition(&self) -> f64 {
        let available = self.consumption_available_units();
        Good::FOOD
            .into_iter()
            .map(|good| available[good as usize] as f64 * good.nutrition_per_unit().unwrap())
            .sum::<f64>()
            + available[Good::Beer as usize] as f64 * 0.05
    }
    pub(crate) fn consumption_available_units(&self) -> [Quantity; Good::COUNT] {
        let mut available = [0; Good::COUNT];
        if self.position != self.map.position(self.home)
            || self.accessible_place(self.home).is_err()
        {
            return available;
        }
        for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
            available[good as usize] = self.stored_available_units(self.home, good);
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
    pub(crate) fn consume_home_goods(
        &mut self,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        if units == 0 {
            return Ok(());
        }
        if self.position != self.map.position(self.home) {
            return Err(SimulationError::WrongLocation);
        }
        self.accessible_place(self.home)?;
        if units > self.stored_available_units(self.home, good) {
            return Err(SimulationError::MissingInputs);
        }
        let key = StockKey {
            place: self.home,
            owner: self.goods_owner(),
            good,
        };
        self.set_stored_units(key, self.storage.units(self.home, key.owner, good) - units)
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
            self.set_stored_units(key, self.storage.units(place, key.owner, good) - taken)?;
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
            self.set_stored_units(
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
            self.set_stored_units(key, self.storage.units(place, key.owner, good) - stored)?;
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
                    other_clothing_replacements: 0,
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
                citizen
                    .storage
                    .set_units(home_key, 30, &citizen.map, &citizen.market)
                    .unwrap();
                citizen.storage.set_reserved(home_key, 5).unwrap();
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
                            &citizen.map,
                            &citizen.market,
                        )
                        .unwrap();
                }
            }
            let available = citizen.consumption_available_units();
            for good in Good::FOOD.into_iter().chain([Good::Water, Good::Beer]) {
                assert_eq!(
                    available[good as usize],
                    citizen.stored_available_units(citizen.home, good)
                );
                assert_eq!(available[good as usize], 18);
            }
            citizen.inventory.insert(Good::Water, Quantity::MAX);
            citizen.tax_reserved.remove(&Good::Water);
            assert_eq!(
                citizen.consumption_available_units()[Good::Water as usize],
                18
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
                    .with_good(Good::BreadRoll, 100)
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents()[&id].kind;
        let home = citizen.home();
        let action = CitizenAction::DepositStored {
            place: home,
            list: ShoppingList::single(Good::BreadRoll, 70),
        };
        let deposited = world
            .start_action(id, action)
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let AgentKind::Citizen(after) = &deposited.agents()[&id].kind;
        assert_eq!(after.units(Good::BreadRoll), 30);
        assert_eq!(
            deposited
                .storage()
                .units(home, GoodsOwner::Agent(id), Good::BreadRoll),
            70
        );
        assert_eq!(citizen.units(Good::BreadRoll), 100);
        assert_eq!(
            world
                .storage()
                .owned_units(GoodsOwner::Agent(id), Good::BreadRoll),
            0
        );
        let predicted = after
            .start_action(CitizenAction::CollectStored {
                place: home,
                list: ShoppingList::single(Good::BreadRoll, 20),
            })
            .unwrap()
            .advance_predicted(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(predicted.units(Good::BreadRoll), 50);
        assert_eq!(predicted.stored_available_units(home, Good::BreadRoll), 50);
        assert_eq!(after.stored_available_units(home, Good::BreadRoll), 70);
    }

    #[test]
    fn storage_transport_cannot_take_tax_reserved_stock() {
        let mut citizen = Citizen::new(0.0).unwrap();
        let place = citizen.home();
        let key = StockKey {
            place,
            owner: GoodsOwner::Agent(citizen.id()),
            good: Good::BreadRoll,
        };
        citizen
            .storage
            .set_units(key, 100, &citizen.map, &citizen.market)
            .unwrap();
        citizen.storage.set_reserved(key, 40).unwrap();
        assert_eq!(
            citizen.start_action(CitizenAction::CollectStored {
                place,
                list: ShoppingList::single(Good::BreadRoll, 61)
            }),
            Err(SimulationError::MissingInputs)
        );
        let collected = citizen
            .start_action(CitizenAction::CollectStored {
                place,
                list: ShoppingList::single(Good::BreadRoll, 60),
            })
            .unwrap()
            .advance_predicted(TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(
            collected.storage.units(place, key.owner, Good::BreadRoll),
            40
        );
        assert_eq!(
            collected
                .storage
                .reserved_units(place, key.owner, Good::BreadRoll),
            40
        );
    }
}

#[cfg(test)]
mod physical_tests {
    use super::*;
    use crate::{
        AgentKind, Citizen, Universe,
        locations::{Location, ObjectKind, Position},
    };

    fn owner() -> GoodsOwner {
        GoodsOwner::Agent(AgentId(uuid::Uuid::new_v4()))
    }
    fn single_bin() -> (Map, PlaceId, ObjectId) {
        let mut map = Map::default();
        let place = map.public_place(Location::Market);
        let bin = map
            .place(place)
            .unwrap()
            .objects()
            .find(|object| object.kind == ObjectKind::DryGoodsBin)
            .unwrap()
            .id;
        map.places_mut()
            .get_mut(&place)
            .unwrap()
            .outdoor_objects
            .retain(|object| object.kind != ObjectKind::DryGoodsBin || object.id == bin);
        (map, place, bin)
    }

    #[test]
    fn every_good_has_one_storage_capability_and_kits_have_agreed_counts() {
        let kinds = [
            ObjectKind::PantryShelving,
            ObjectKind::DryGoodsBin,
            ObjectKind::FishCrate,
            ObjectKind::WoodRack,
            ObjectKind::TextileChest,
            ObjectKind::LiquidBarrel,
        ];
        for good in Good::ALL {
            assert_eq!(kinds.iter().filter(|kind| kind.accepts(good)).count(), 1);
        }
        let citizen = Citizen::new(0.0).unwrap();
        for (kind, count) in [
            (Location::Home, 5),
            (Location::Field, 4),
            (Location::Mill, 2),
            (Location::Bakery, 4),
            (Location::Weavery, 1),
            (Location::Tailory, 1),
            (Location::Brewery, 5),
        ] {
            let (map, place) = citizen
                .map
                .with_place(
                    kind,
                    Position::default(),
                    Some(citizen.id),
                    "Storage fixture",
                )
                .unwrap();
            let site = map.place(place).unwrap();
            assert_eq!(
                site.objects()
                    .filter(|object| object.kind.is_storage())
                    .count(),
                count
            );
            assert_eq!(
                site.buildings[0]
                    .rooms
                    .iter()
                    .filter(|room| room.name == "Storeroom")
                    .count(),
                1
            );
        }
        for (kind, count) in [
            (Location::Forest, 2),
            (Location::River, 2),
            (Location::Market, 9),
            (Location::Warehouse, 9),
        ] {
            assert_eq!(
                citizen
                    .map
                    .place(citizen.map.public_place(kind))
                    .unwrap()
                    .objects()
                    .filter(|object| object.kind.is_storage())
                    .count(),
                count
            );
        }
    }

    #[test]
    fn bins_share_a_good_across_owners_and_switch_only_after_stock_and_escrow_leave() {
        let (map, place, bin) = single_bin();
        let mut storage = Storage::default();
        let mut market = Market::default();
        let first = StockKey {
            place,
            owner: owner(),
            good: Good::Wheat,
        };
        let second = StockKey {
            owner: owner(),
            ..first
        };
        storage.set_units(first, 10, &map, &market).unwrap();
        storage.set_units(second, 20, &map, &market).unwrap();
        assert_eq!(
            storage
                .contents()
                .keys()
                .filter(|key| key.object == bin)
                .count(),
            2
        );
        let flour = StockKey {
            good: Good::Flour,
            ..first
        };
        assert_eq!(
            storage.set_units(flour, 1, &map, &market),
            Err(SimulationError::MissingInputs)
        );
        storage.set_units(first, 0, &map, &market).unwrap();
        storage.set_units(second, 0, &map, &market).unwrap();
        let seller = AgentId(uuid::Uuid::new_v4());
        market
            .list_in(seller.into(), place, Good::Wheat, 30, bin)
            .unwrap();
        assert_eq!(
            storage.set_units(flour, 1, &map, &market),
            Err(SimulationError::MissingInputs)
        );
        market.withdraw(seller, place, Good::Wheat, 30).unwrap();
        storage.set_units(flour, 1, &map, &market).unwrap();
        storage.validate_save().unwrap();
        storage.validate_objects(&map, &market).unwrap();
    }

    #[test]
    fn occupied_compatible_mixed_storage_is_preferred_over_an_empty_object() {
        let citizen = Citizen::new(0.0).unwrap();
        let (mut map, place) = citizen
            .map
            .with_place(
                Location::Home,
                Position::default(),
                Some(citizen.id),
                "Mixed",
            )
            .unwrap();
        let first = map
            .place(place)
            .unwrap()
            .objects()
            .find(|object| object.kind == ObjectKind::PantryShelving)
            .unwrap()
            .id;
        let room = &mut map.places_mut().get_mut(&place).unwrap().buildings[0].rooms[1];
        room.object_capacity += 1;
        room.objects
            .push(crate::locations::Object::new(ObjectKind::PantryShelving));
        let mut storage = Storage::default();
        let market = Market::default();
        for good in [Good::Berries, Good::BreadRoll] {
            storage
                .set_units_in(
                    StockKey {
                        place,
                        owner: owner(),
                        good,
                    },
                    10,
                    &map,
                    &market,
                    Some(&[first]),
                )
                .unwrap();
        }
        for _ in 0..20 {
            assert_eq!(
                storage
                    .select_object(place, Good::Berries, &map, &market, None)
                    .unwrap(),
                first
            );
        }
    }

    #[test]
    fn completion_consumes_its_own_physical_reservation_and_preserves_other_workers() {
        let map = Map::default();
        let place = map.public_place(Location::Market);
        let market = Market::default();
        let bins: Vec<_> = map
            .place(place)
            .unwrap()
            .objects()
            .filter(|object| object.kind == ObjectKind::DryGoodsBin)
            .map(|object| object.id)
            .collect();
        let key = StockKey {
            place,
            owner: owner(),
            good: Good::Wheat,
        };
        let first = AgentId(uuid::Uuid::new_v4());
        let second = AgentId(uuid::Uuid::new_v4());
        let mut storage = Storage::default();
        storage
            .set_units_in(key, 10, &map, &market, Some(&bins[..1]))
            .unwrap();
        storage
            .set_units_in(key, 30, &map, &market, Some(&bins[1..2]))
            .unwrap();
        storage
            .reserve_work_for(first, key, 10, &bins[..1])
            .unwrap();
        storage
            .reserve_work_for(second, key, 20, &bins[1..2])
            .unwrap();
        let before = storage.clone();
        storage.release_work_for(second, key, 20, true).unwrap();
        assert_eq!(
            storage.object_units(ObjectStockKey {
                stock: key,
                object: bins[0]
            }),
            10
        );
        assert_eq!(
            storage.object_units(ObjectStockKey {
                stock: key,
                object: bins[1]
            }),
            0
        );
        assert_eq!(storage.work_reserved_units(key), 10);
        assert_eq!(before.units(place, key.owner, key.good), 30);
        storage.release_work_for(first, key, 10, false).unwrap();
        assert_eq!(storage.units(place, key.owner, key.good), 10);
        storage.validate_save().unwrap();
    }

    #[test]
    fn tax_collection_changes_owner_in_place_and_keeps_work_reservations() {
        let (map, place, bin) = single_bin();
        let market = Market::default();
        let mut storage = Storage::default();
        let key = StockKey {
            place,
            owner: owner(),
            good: Good::Wheat,
        };
        let worker = AgentId(uuid::Uuid::new_v4());
        storage.set_units(key, 30, &map, &market).unwrap();
        storage.set_reserved(key, 10).unwrap();
        storage.reserve_work_for(worker, key, 20, &[bin]).unwrap();
        storage.settle_tax(key, 10).unwrap();
        assert_eq!(
            storage.object_units(ObjectStockKey {
                stock: StockKey {
                    owner: GoodsOwner::Town,
                    ..key
                },
                object: bin
            }),
            10
        );
        assert_eq!(
            storage.object_work_reserved(ObjectStockKey {
                stock: key,
                object: bin
            }),
            20
        );
        assert_eq!(
            storage.set_units(key, 19, &map, &market),
            Err(SimulationError::MissingInputs)
        );
        storage.validate_save().unwrap();
    }

    #[test]
    fn goods_in_a_foreign_building_are_unavailable_even_at_an_accessible_site() {
        let (world, id) = Universe::default()
            .with_citizen("Owner", Citizen::new(0.0).unwrap())
            .unwrap();
        let (mut world, place) = world.with_property(id, Location::Mill).unwrap();
        let (world2, other) = world
            .with_citizen("Other", Citizen::new(0.0).unwrap())
            .unwrap();
        world = world2;
        let (map, building) = world
            .map
            .with_building(place, Location::Mill, other, "Private mill")
            .unwrap();
        world.map = map;
        let object = world
            .map
            .place(place)
            .unwrap()
            .buildings
            .iter()
            .find(|entry| entry.id == building)
            .unwrap()
            .rooms
            .iter()
            .flat_map(|room| &room.objects)
            .find(|object| object.kind == ObjectKind::DryGoodsBin)
            .unwrap()
            .id;
        let key = StockKey {
            place,
            owner: GoodsOwner::Agent(id),
            good: Good::Wheat,
        };
        world
            .storage
            .set_units_in(key, 50, &world.map, &world.market, Some(&[object]))
            .unwrap();
        world.refresh_market();
        let AgentKind::Citizen(citizen) = &world.agents[&id].kind;
        assert!(citizen.accessible_place(place).is_ok());
        assert_eq!(citizen.stored_available_units(place, Good::Wheat), 0);
        assert_eq!(world.storage.units(place, key.owner, key.good), 50);
        assert!(
            !citizen.may_use_place(
                world
                    .map
                    .place(world.map.public_place(Location::Warehouse))
                    .unwrap()
            )
        );
    }
    #[test]
    fn private_output_preflight_allows_freed_bins_and_refuses_blocked_output() {
        use crate::{CitizenAction, StartingRole, production::Recipe};
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen(
                "Miller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (world, mill) = world.with_property(id, Location::Mill).unwrap();
        let owner = GoodsOwner::Agent(id);
        let blocked = world
            .with_stored_good(mill, owner, Good::Wheat, 600)
            .unwrap()
            .with_stored_good(mill, owner, Good::Hops, 1)
            .unwrap();
        let action = CitizenAction::ProduceAt {
            place: mill,
            recipe: Recipe::MillFlour,
        };
        assert_eq!(
            blocked.start_action(id, action),
            Err(SimulationError::MissingInputs)
        );
        assert!(
            crate::planning::plan(match &blocked.agents[&id].kind {
                AgentKind::Citizen(citizen) => citizen,
            })
            .is_ok()
        );
        let freed = blocked
            .with_stored_good(mill, owner, Good::Wheat, 300)
            .unwrap()
            .start_action(id, action)
            .unwrap();
        let duration = match &freed.agents[&id].kind {
            AgentKind::Citizen(citizen) => citizen.active_action().unwrap().duration_ms(),
        };
        let completed = freed.advance(duration).unwrap();
        assert_eq!(completed.storage.units(mill, owner, Good::Wheat), 0);
        assert_eq!(completed.storage.units(mill, owner, Good::Flour), 240);
        assert_eq!(completed.storage.units(mill, owner, Good::Hops), 1);
    }

    #[test]
    fn output_contention_preserves_inputs_releases_claims_and_records_no_production() {
        use crate::{CitizenAction, StartingRole, production::Recipe};
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen(
                "Miller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (world, mill) = world.with_property(id, Location::Mill).unwrap();
        let owner = GoodsOwner::Agent(id);
        let started = world
            .with_stored_good(mill, owner, Good::Wheat, 600)
            .unwrap()
            .start_action(
                id,
                CitizenAction::ProduceAt {
                    place: mill,
                    recipe: Recipe::MillFlour,
                },
            )
            .unwrap();
        let duration = match &started.agents[&id].kind {
            AgentKind::Citizen(citizen) => citizen.active_action().unwrap().duration_ms(),
        };
        let contested = started
            .with_stored_good(mill, GoodsOwner::Town, Good::Hops, 1)
            .unwrap();
        let completed = contested.advance(duration).unwrap();
        assert_eq!(completed.storage.units(mill, owner, Good::Wheat), 600);
        assert_eq!(completed.storage.units(mill, owner, Good::Flour), 0);
        assert_eq!(
            completed.storage.work_reserved_units(StockKey {
                place: mill,
                owner,
                good: Good::Wheat
            }),
            0
        );
        assert!(
            completed
                .map
                .place(mill)
                .unwrap()
                .objects()
                .all(|object| object.workers.is_empty())
        );
        assert_eq!(
            completed.citizen_history(id).unwrap().current.produced[Good::Flour as usize],
            0
        );
        assert_eq!(
            completed.citizen_history(id).unwrap().current.consumed[Good::Wheat as usize],
            0
        );
        assert_eq!(
            Universe::from_save_str(&completed.to_save_string().unwrap()).unwrap(),
            completed
        );
        let AgentKind::Citizen(predicted) = &contested.agents[&id].kind;
        assert_eq!(
            predicted.advance_predicted(duration),
            Err(SimulationError::MissingInputs)
        );
    }

    #[test]
    fn listing_contention_keeps_goods_carried_and_rolls_back_every_order() {
        use crate::{CitizenAction, marketplace::ShoppingList};
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen(
                "Seller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_good(Good::Wheat, 100)
                    .unwrap()
                    .with_good(Good::Wood, 100)
                    .unwrap(),
            )
            .unwrap();
        let (world, bakery) = world.with_property(id, Location::Bakery).unwrap();
        let started = world
            .start_action(
                id,
                CitizenAction::ListAt {
                    place: bakery,
                    list: ShoppingList::new([(Good::Wheat, 100), (Good::Wood, 100)]).unwrap(),
                },
            )
            .unwrap();
        let contested = started
            .with_stored_good(bakery, GoodsOwner::Town, Good::Flour, 1)
            .unwrap();
        let completed = contested.advance(crate::TRADE_DURATION_MS).unwrap();
        let AgentKind::Citizen(citizen) = &completed.agents[&id].kind;
        assert_eq!(citizen.units(Good::Wheat), 100);
        assert_eq!(citizen.units(Good::Wood), 100);
        assert_eq!(completed.market.orders().count(), 0);
        assert_eq!(
            Universe::from_save_str(&completed.to_save_string().unwrap()).unwrap(),
            completed
        );
    }
    #[test]
    fn harvest_output_contention_preserves_crop_and_does_not_tax_nonexistent_goods() {
        use crate::{
            CitizenAction, StartingRole,
            farming::{Crop, CropKind, GROWTH_MS},
            taxation::{TaxKind, TaxRate},
        };
        let mut world = Universe::starting_at(GROWTH_MS);
        world.map = Map::default();
        let (world, id) = world
            .with_citizen(
                "Farmer",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Farmer),
            )
            .unwrap();
        let (mut world, field) = world.with_property(id, Location::Field).unwrap();
        let crop = Crop {
            kind: CropKind::Hops,
            planted_at_ms: 0,
            tended: [true; 2],
        };
        world.map.set_crop(field, 0, Some(crop)).unwrap();
        world.refresh_market();
        let (world, rule) = world
            .with_tax_rule(
                field,
                "Harvest share",
                TaxKind::Socage {
                    rates: [(Good::Hops, TaxRate::new(5000).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let started = world
            .start_action(id, CitizenAction::Harvest { field, plot: 0 })
            .unwrap();
        let duration = match &started.agents[&id].kind {
            AgentKind::Citizen(citizen) => citizen.active_action().unwrap().duration_ms(),
        };
        let contested = started
            .with_stored_good(field, GoodsOwner::Town, Good::Wheat, 1)
            .unwrap()
            .with_stored_good(field, GoodsOwner::Town, Good::Flour, 1)
            .unwrap();
        let completed = contested.advance(duration).unwrap();
        assert_eq!(completed.map.place(field).unwrap().plots()[0], Some(crop));
        assert_eq!(
            completed
                .storage
                .owned_units(GoodsOwner::Agent(id), Good::Hops),
            0
        );
        assert_eq!(completed.tax_arrears(rule, id), 0);
        assert!(completed.tax_history().is_empty());
        assert_eq!(
            completed.citizen_history(id).unwrap().current.produced[Good::Hops as usize],
            0
        );
        assert_eq!(
            Universe::from_save_str(&completed.to_save_string().unwrap()).unwrap(),
            completed
        );
    }
    #[test]
    fn save_rejects_work_allocations_assigned_to_a_different_worker() {
        use crate::{CitizenAction, StartingRole, production::Recipe};
        let (world, worker) = Universe::with_map(Map::default())
            .with_citizen(
                "Miller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (world, other) = world
            .with_citizen("Other", Citizen::new(0.0).unwrap())
            .unwrap();
        let home = match &world.agents[&worker].kind {
            AgentKind::Citizen(citizen) => citizen.home(),
        };
        let (world, household) = world
            .with_household("Family", home, &[worker, other])
            .unwrap();
        let (world, mill) = world.with_property(worker, Location::Mill).unwrap();
        let mut world = world
            .with_stored_good(mill, GoodsOwner::Household(household), Good::Wheat, 300)
            .unwrap()
            .start_action(
                worker,
                CitizenAction::ProduceAt {
                    place: mill,
                    recipe: Recipe::MillFlour,
                },
            )
            .unwrap();
        let ((_, physical), units) = world
            .storage
            .work_allocations
            .iter()
            .next()
            .map(|(key, units)| (*key, *units))
            .unwrap();
        world.storage.work_allocations.remove(&(worker, physical));
        world
            .storage
            .work_allocations
            .insert((other, physical), units);
        world.refresh_market();
        assert!(matches!(
            Universe::from_save_str(&world.to_save_string().unwrap()),
            Err(crate::persistence::SaveError::InvalidState(
                "production reservation mismatch"
            ))
        ));
    }
    #[test]
    fn manual_tax_transfers_use_accessible_objects_and_leave_foreign_allocations_untouched() {
        use crate::taxation::{TaxKind, TaxRate};
        for reserve_foreign in [false, true] {
            let (world, id) = Universe::with_map(Map::default())
                .with_citizen("Owner", Citizen::new(0.0).unwrap())
                .unwrap();
            let (world, other) = world
                .with_citizen("Other", Citizen::new(0.0).unwrap())
                .unwrap();
            let (mut world, place) = world.with_property(id, Location::Bakery).unwrap();
            let (map, building) = world
                .map
                .with_building(place, Location::Bakery, other, "Foreign bakery")
                .unwrap();
            world.map = map;
            let own = world.map.place(place).unwrap().buildings[0]
                .rooms
                .iter()
                .flat_map(|room| &room.objects)
                .find(|object| object.kind == ObjectKind::PantryShelving)
                .unwrap()
                .id;
            let foreign = world
                .map
                .place(place)
                .unwrap()
                .buildings
                .iter()
                .find(|entry| entry.id == building)
                .unwrap()
                .rooms
                .iter()
                .flat_map(|room| &room.objects)
                .find(|object| object.kind == ObjectKind::PantryShelving)
                .unwrap()
                .id;
            let key = StockKey {
                place,
                owner: GoodsOwner::Agent(id),
                good: Good::BreadRoll,
            };
            world
                .storage
                .set_units_in(key, 10, &world.map, &world.market, Some(&[own]))
                .unwrap();
            world.refresh_market();
            let (updated, _) = world
                .with_tax_rule(
                    place,
                    "Half",
                    TaxKind::Socage {
                        rates: [(Good::BreadRoll, TaxRate::new(5000).unwrap())]
                            .into_iter()
                            .collect(),
                    },
                )
                .unwrap();
            world = updated;
            world
                .tax_production(id, place, Good::BreadRoll, 20, 0)
                .unwrap();
            world
                .storage
                .set_units_in(key, 20, &world.map, &world.market, Some(&[foreign]))
                .unwrap();
            if reserve_foreign {
                world
                    .tax_production(id, place, Good::BreadRoll, 20, 0)
                    .unwrap();
            }
            world.refresh_market();
            let withdrawn = world
                .withdraw_goods(id, place, Good::BreadRoll, 10)
                .unwrap();
            let AgentKind::Citizen(citizen) = &withdrawn.agents[&id].kind;
            assert_eq!(citizen.units(Good::BreadRoll), 10);
            assert_eq!(citizen.tax_reserved_units(Good::BreadRoll), 10);
            assert_eq!(
                withdrawn.storage.object_units(ObjectStockKey {
                    stock: key,
                    object: own
                }),
                0
            );
            assert_eq!(
                withdrawn.storage.object_units(ObjectStockKey {
                    stock: key,
                    object: foreign
                }),
                10
            );
            assert_eq!(
                withdrawn.storage.object_tax_reserved(ObjectStockKey {
                    stock: key,
                    object: foreign
                }),
                if reserve_foreign { 10 } else { 0 }
            );
            let deposited = withdrawn
                .deposit_goods(id, place, Good::BreadRoll, 10)
                .unwrap();
            assert_eq!(
                deposited.storage.object_tax_reserved(ObjectStockKey {
                    stock: key,
                    object: own
                }),
                10
            );
            assert_eq!(
                deposited.storage.object_tax_reserved(ObjectStockKey {
                    stock: key,
                    object: foreign
                }),
                if reserve_foreign { 10 } else { 0 }
            );
            assert_eq!(
                Universe::from_save_str(&deposited.to_save_string().unwrap()).unwrap(),
                deposited
            );
            assert_eq!(
                world.storage.object_units(ObjectStockKey {
                    stock: key,
                    object: own
                }),
                10
            );
        }
    }
}
