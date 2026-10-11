use crate::{
    AgentId, AgentKind, Citizen, Coins, Quantity, SimulationError, Universe,
    locations::PlaceId,
    marketplace::{Good, MarketParty},
    storage::{GoodsOwner, StockKey},
};
use imbl::HashMap;

pub const MAINTENANCE_COOLDOWN_MS: u64 = 6 * 60 * 60 * 1000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Household {
    pub id: AgentId,
    pub name: String,
    pub home: PlaceId,
    pub members: Vec<AgentId>,
    pub coins: Coins,
    pub maintenance: crate::maintenance::MaintenanceState,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct HouseholdContext {
    pub id: AgentId,
    pub members: Vec<AgentId>,
    pub allegiance: f64,
    pub other_carried: [Quantity; Good::COUNT],
    pub other_consumption_value: f64,
}

impl Citizen {
    pub fn maintenance(&self) -> &crate::maintenance::MaintenanceState {
        &self.maintenance
    }
    pub fn household_id(&self) -> Option<AgentId> {
        self.household.as_ref().map(|household| household.id)
    }
    pub fn economic_id(&self) -> AgentId {
        self.household_id().unwrap_or(self.id)
    }
    pub fn goods_owner(&self) -> GoodsOwner {
        self.household_id()
            .map_or(GoodsOwner::Agent(self.id), GoodsOwner::Household)
    }
    pub fn market_party(&self) -> MarketParty {
        self.household_id()
            .map_or(MarketParty::Citizen(self.id), MarketParty::Household)
    }
    pub fn designated_workplace(&self) -> Option<PlaceId> {
        self.designated_workplace
    }
    pub(crate) fn may_use_owner(&self, owner: AgentId) -> bool {
        owner == self.id
            || self
                .household
                .as_ref()
                .is_some_and(|household| household.members.contains(&owner))
    }
    pub fn decision_score(&self) -> Result<f64, SimulationError> {
        let value = self.personal_wellbeing()?
            + self.household.as_ref().map_or(0.0, |household| {
                household.allegiance
                    * self.economic_wealth().unwrap_or(f64::NAN)
                    * crate::WEALTH_WELLBEING_PER_COIN
            });
        if value.is_finite() {
            Ok(value)
        } else {
            Err(SimulationError::WellbeingOverflow)
        }
    }
}

impl Universe {
    pub fn households(&self) -> &HashMap<AgentId, Household> {
        &self.households
    }
    pub(crate) fn economic_id(&self, worker: AgentId) -> Result<AgentId, SimulationError> {
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&worker)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        Ok(citizen.economic_id())
    }
    pub(crate) fn account_owner(&self, account: AgentId) -> Result<GoodsOwner, SimulationError> {
        if self.households.contains_key(&account) {
            Ok(GoodsOwner::Household(account))
        } else if self.agents.contains_key(&account) {
            Ok(GoodsOwner::Agent(account))
        } else {
            Err(SimulationError::AgentNotFound)
        }
    }
    pub(crate) fn account_coins(&self, account: AgentId) -> Result<Coins, SimulationError> {
        if let Some(household) = self.households.get(&account) {
            return Ok(household.coins);
        }
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&account)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        Ok(citizen.coins)
    }
    pub(crate) fn set_account_coins(
        &mut self,
        account: AgentId,
        coins: Coins,
    ) -> Result<(), SimulationError> {
        if let Some(household) = self.households.get_mut(&account) {
            household.coins = coins;
        } else {
            let AgentKind::Citizen(citizen) = &mut self
                .agents
                .get_mut(&account)
                .ok_or(SimulationError::AgentNotFound)?
                .kind;
            citizen.coins = coins;
        }
        Ok(())
    }
    pub fn account_owned_units(
        &self,
        account: AgentId,
        good: Good,
    ) -> Result<Quantity, SimulationError> {
        let owner = self.account_owner(account)?;
        self.agents
            .values()
            .filter_map(|agent| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                (citizen.economic_id() == account).then_some(citizen.units(good))
            })
            .try_fold(self.storage.owned_units(owner, good), Quantity::checked_add)
            .and_then(|units| {
                units.checked_add(self.market.listed_units(
                    if self.households.contains_key(&account) {
                        MarketParty::Household(account)
                    } else {
                        MarketParty::Citizen(account)
                    },
                    good,
                ))
            })
            .ok_or(SimulationError::InventoryOverflow)
    }
    pub fn with_designated_workplace(
        &self,
        worker: AgentId,
        place: PlaceId,
    ) -> Result<Self, SimulationError> {
        let mut universe = self.clone();
        let AgentKind::Citizen(citizen) = &mut universe
            .agents
            .get_mut(&worker)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        citizen.accessible_place(place)?;
        citizen.designated_workplace = Some(place);
        universe.refresh_market();
        universe.refresh_workplace_supplies()?;
        universe.refresh_household_history(universe.current_time_ms, false)?;
        Ok(universe)
    }
    pub fn with_household(
        &self,
        name: impl Into<String>,
        home: PlaceId,
        members: &[AgentId],
    ) -> Result<(Self, AgentId), SimulationError> {
        if members.is_empty()
            || members
                .iter()
                .enumerate()
                .any(|(index, id)| members[..index].contains(id))
        {
            return Err(SimulationError::InvalidOwnership);
        }
        let home_place = self.map.place(home)?;
        if home_place.kind != crate::locations::Location::Home
            || !home_place
                .owner
                .is_some_and(|owner| members.contains(&owner))
        {
            return Err(SimulationError::PrivateProperty);
        }
        let mut coins: Coins = 0;
        for id in members {
            let AgentKind::Citizen(citizen) = &self
                .agents
                .get(id)
                .ok_or(SimulationError::AgentNotFound)?
                .kind;
            if citizen.household.is_some()
                || citizen.active_action.is_some()
                || citizen.active_plan.is_some()
                || citizen.tax_reserved.values().any(|units| *units != 0)
            {
                return Err(SimulationError::CitizenBusy);
            }
            if self
                .market
                .orders()
                .any(|order| order.seller.account() == Some(*id))
                || self.taxation.accounts.keys().any(|key| key.payer == *id)
            {
                return Err(SimulationError::InvalidOwnership);
            }
            coins = coins
                .checked_add(citizen.coins)
                .ok_or(SimulationError::WealthOverflow)?;
        }
        let id = AgentId(uuid::Uuid::new_v4());
        let mut universe = self.clone();
        universe.households.insert(
            id,
            Household {
                id,
                name: name.into(),
                home,
                members: members.to_vec(),
                coins,
                maintenance: crate::maintenance::MaintenanceState::default(),
            },
        );
        for member in members {
            let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(member).unwrap().kind;
            citizen.home = home;
            citizen.position = universe.map.position(home);
            citizen.household = Some(HouseholdContext {
                id,
                members: members.to_vec(),
                allegiance: 1.0,
                other_carried: [0; Good::COUNT],
                other_consumption_value: 0.0,
            });
            citizen.coins = coins;
        }
        let stock: Vec<_> = universe.storage.stock().iter().filter(|(key,_)| matches!(key.owner, GoodsOwner::Agent(owner) if members.contains(&owner))).map(|(key,units)| (*key,*units)).collect();
        for (key, units) in stock {
            if universe
                .storage
                .reserved_units(key.place, key.owner, key.good)
                > 0
            {
                return Err(SimulationError::MissingInputs);
            }
            universe.storage.set_units(key, 0)?;
            let household_key = StockKey {
                owner: GoodsOwner::Household(id),
                ..key
            };
            let total = universe
                .storage
                .units(key.place, household_key.owner, key.good)
                .checked_add(units)
                .ok_or(SimulationError::InventoryOverflow)?;
            universe.storage.set_units(household_key, total)?;
        }
        universe.household_histories.insert(
            id,
            crate::household_history::HouseholdHistory::new(universe.current_time_ms, coins),
        );
        universe.refresh_household_history(universe.current_time_ms, false)?;
        universe.refresh_market();
        Ok((universe, id))
    }
}

impl Universe {
    pub fn household_wealth(&self, id: AgentId) -> Result<f64, SimulationError> {
        let household = self
            .households
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?;
        let member = *household
            .members
            .first()
            .ok_or(SimulationError::InvalidOwnership)?;
        let AgentKind::Citizen(citizen) = &self.agents[&member].kind;
        citizen.economic_wealth()
    }

    pub fn place_starting_supplies(&self) -> Result<Self, SimulationError> {
        let mut universe = self.clone();
        let ids: Vec<_> = universe.agents.keys().copied().collect();
        for id in ids {
            let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(&id).unwrap().kind;
            if citizen.active_action.is_some()
                || citizen.active_plan.is_some()
                || citizen.tax_reserved.values().any(|units| *units > 0)
            {
                return Err(SimulationError::CitizenBusy);
            }
            let goods: Vec<_> = citizen
                .inventory
                .iter()
                .map(|(good, units)| (*good, *units))
                .collect();
            for (good, units) in goods {
                if units == 0 {
                    continue;
                }
                let production_units = citizen
                    .starting_role
                    .map_or(0, |role| {
                        crate::production::starting_inputs(role).units(good)
                    })
                    .min(units);
                let portions = [
                    (
                        citizen.designated_workplace.unwrap_or(citizen.home),
                        production_units,
                    ),
                    (citizen.home, units - production_units),
                ];
                for (place, units) in portions {
                    if units == 0 {
                        continue;
                    }
                    let key = StockKey {
                        place,
                        owner: citizen.goods_owner(),
                        good,
                    };
                    let next = universe
                        .storage
                        .units(place, key.owner, good)
                        .checked_add(units)
                        .ok_or(SimulationError::InventoryOverflow)?;
                    universe.storage.set_units(key, next)?;
                }
                citizen.inventory.remove(&good);
            }
            citizen.position = universe.map.position(citizen.home);
        }
        universe.refresh_market();
        universe.refresh_household_history(universe.current_time_ms, false)?;
        Ok(universe)
    }
}

impl Citizen {
    pub(crate) fn economic_owned_units(&self, good: Good) -> Quantity {
        self.resource_units(good)
            .unwrap_or(Quantity::MAX)
            .saturating_add(self.market.listed_units(self.market_party(), good))
            .saturating_add(
                self.household
                    .as_ref()
                    .map_or(0, |household| household.other_carried[good as usize]),
            )
    }
    pub(crate) fn stocking_requirements_at(
        &self,
        site: Option<PlaceId>,
    ) -> Option<(
        crate::marketplace::ShoppingList,
        crate::marketplace::ShoppingList,
    )> {
        self.workplace_supplies.get(&site?).copied()
    }
    pub(crate) fn collectable_reserve_units(&self, place: PlaceId, good: Good) -> Quantity {
        self.stored_available_units(place, good).saturating_sub(
            self.workplace_supplies
                .get(&place)
                .map_or(0, |(_, target)| target.units(good)),
        )
    }
}

impl Universe {
    pub(crate) fn refresh_workplace_supplies(&mut self) -> Result<(), SimulationError> {
        let mut requirements: std::collections::HashMap<
            (AgentId, PlaceId),
            ([Quantity; Good::COUNT], [Quantity; Good::COUNT]),
        > = std::collections::HashMap::new();
        for agent in self.agents.values() {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let Some(place) = citizen.designated_workplace else {
                continue;
            };
            let entry = requirements
                .entry((citizen.economic_id(), place))
                .or_insert(([0; Good::COUNT], [0; Good::COUNT]));
            for (index, horizon) in [
                crate::production::PRODUCTION_BLOCK_MS,
                crate::production::DAILY_CAPACITY_MS,
            ]
            .into_iter()
            .enumerate()
            {
                let recipes =
                    crate::production::projected_recipes_at(citizen, Some(place), horizon)?;
                let stock =
                    crate::production::recipe_requirements_at(citizen, Some(place), &recipes)?;
                for (good, units) in stock.items() {
                    let target = if index == 0 {
                        &mut entry.0
                    } else {
                        &mut entry.1
                    };
                    target[good as usize] = target[good as usize]
                        .checked_add(units)
                        .ok_or(SimulationError::InventoryOverflow)?;
                }
            }
        }
        for (_, agent) in self.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.workplace_supplies.clear();
            for ((account, place), (threshold, target)) in &requirements {
                if *account != citizen.economic_id() {
                    continue;
                }
                citizen.workplace_supplies.insert(
                    *place,
                    (
                        crate::marketplace::ShoppingList::new(
                            Good::ALL
                                .into_iter()
                                .map(|good| (good, threshold[good as usize])),
                        )?,
                        crate::marketplace::ShoppingList::new(
                            Good::ALL
                                .into_iter()
                                .map(|good| (good, target[good as usize])),
                        )?,
                    ),
                );
            }
        }
        Ok(())
    }
}

impl Citizen {
    fn reserve_units(&self, good: Good, available: bool) -> Quantity {
        let stored = if available {
            self.stored_available_units(self.home, good)
        } else {
            self.storage.units(self.home, self.goods_owner(), good)
        };
        let carried = if available {
            self.carried_available_units(good)
        } else {
            self.units(good)
        };
        stored.saturating_add(carried).saturating_add(
            self.household
                .as_ref()
                .map_or(0, |household| household.other_carried[good as usize]),
        )
    }
    pub(crate) fn reserve_food_nutrition(&self) -> f64 {
        Good::FOOD
            .into_iter()
            .map(|good| self.reserve_units(good, false) as f64 * good.nutrition_per_unit().unwrap())
            .sum::<f64>()
            + self.reserve_units(Good::Beer, false) as f64 * 0.05
    }
    pub(crate) fn reserve_drink_units(&self) -> Quantity {
        self.reserve_units(Good::Water, true)
            .saturating_add(self.reserve_units(Good::Beer, true))
    }
}
impl Universe {
    pub(crate) fn account_workers(&self, account: AgentId) -> Vec<AgentId> {
        self.households
            .get(&account)
            .map_or_else(|| vec![account], |household| household.members.clone())
    }
}

impl Citizen {
    pub(crate) fn consumption_value(&self) -> f64 {
        self.active_action.map_or(0.0, |active| {
            let elapsed = active.duration_ms - active.remaining_ms;
            let drink_duration =
                active.drink_units * crate::DRINK_DURATION_MS / crate::DRINK_WATER_GRAMS;
            let drink = self
                .prices
                .value(active.drink_good, active.drink_units)
                .unwrap_or(0.0)
                * drink_duration.saturating_sub(elapsed) as f64
                / drink_duration.max(1) as f64;
            let meal = Good::FOOD
                .into_iter()
                .map(|good| {
                    self.prices
                        .value(good, active.meal_units[good as usize])
                        .unwrap_or(0.0)
                })
                .sum::<f64>()
                * active.meal_duration_ms.saturating_sub(elapsed) as f64
                / active.meal_duration_ms.max(1) as f64;
            drink + meal
        })
    }
}

impl Citizen {
    pub(crate) fn incoming_workplace_units(&self, place: PlaceId, good: Good) -> Quantity {
        self.incoming_supplies
            .get(&place)
            .map_or(0, |list| list.units(good))
    }
}
impl Universe {
    pub(crate) fn refresh_account_requests(&mut self) -> Result<(), SimulationError> {
        let mut groups: std::collections::HashMap<(AgentId, uuid::Uuid), [Quantity; Good::COUNT]> =
            std::collections::HashMap::new();
        for (worker, agent) in &self.agents {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let key = (
                citizen.economic_id(),
                citizen.request_group.map_or(worker.0, |place| place.0),
            );
            let group = groups.entry(key).or_insert([0; Good::COUNT]);
            for good in Good::ALL {
                group[good as usize] = group[good as usize].max(citizen.market_request.units(good));
            }
        }
        let mut accounts: std::collections::HashMap<AgentId, [Quantity; Good::COUNT]> =
            std::collections::HashMap::new();
        for ((account, _), group) in groups {
            let total = accounts.entry(account).or_insert([0; Good::COUNT]);
            for index in 0..Good::COUNT {
                total[index] = total[index]
                    .checked_add(group[index])
                    .ok_or(SimulationError::InventoryOverflow)?;
            }
        }
        for (account, quantities) in accounts {
            self.market.set_request(
                account,
                crate::marketplace::ShoppingList::new(
                    Good::ALL
                        .into_iter()
                        .map(|good| (good, quantities[good as usize])),
                )?,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CitizenAction, StartingRole,
        locations::{Location, Map},
        production::Recipe,
    };

    #[test]
    fn shared_collection_and_equipment_contention_preserve_goods_and_finish_safely() {
        for scenario in 0..3 {
            let (world, first) = Universe::with_map(Map::default())
                .with_citizen(
                    "First",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_starting_role(StartingRole::Brewer),
                )
                .unwrap();
            let (world, second) = world
                .with_citizen(
                    "Second",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_starting_role(StartingRole::Brewer),
                )
                .unwrap();
            let (world, site) = world.with_property(first, Location::Brewery).unwrap();
            let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
            let (mut world, household) = world
                .with_household("Shared", citizen.home(), &[first, second])
                .unwrap();
            let owner = GoodsOwner::Household(household);
            let action = match scenario {
                0 => {
                    world = world
                        .with_stored_good(site, owner, Good::Wheat, 100)
                        .unwrap();
                    CitizenAction::CollectStored {
                        place: site,
                        list: crate::marketplace::ShoppingList::single(Good::Wheat, 100),
                    }
                }
                1 => {
                    world
                        .map
                        .set_batch(
                            site,
                            0,
                            Some(crate::brewing::Batch::Fermenting { ready_at_ms: 0 }),
                        )
                        .unwrap();
                    world.refresh_market();
                    CitizenAction::CollectBeer {
                        brewery: site,
                        slot: 0,
                    }
                }
                _ => {
                    let home = world.households[&household].home;
                    world = world
                        .with_stored_good(home, owner, Good::FlaxGarment, 1)
                        .unwrap();
                    CitizenAction::EquipClothing
                }
            };
            let started = world
                .start_action(first, action)
                .unwrap()
                .start_action(second, action)
                .unwrap();
            let AgentKind::Citizen(worker) = &started.agents[&first].kind;
            let elapsed = worker.active_action().unwrap().remaining_ms();
            let finished = started.advance(elapsed).unwrap();
            for id in [first, second] {
                let AgentKind::Citizen(worker) = &finished.agents[&id].kind;
                assert!(worker.active_action().is_none());
                assert_eq!(worker.current_time_ms(), elapsed);
            }
            match scenario {
                0 => assert_eq!(
                    finished
                        .account_owned_units(household, Good::Wheat)
                        .unwrap(),
                    100
                ),
                1 => {
                    assert_eq!(
                        finished.account_owned_units(household, Good::Beer).unwrap(),
                        1000
                    );
                    assert_eq!(
                        finished.citizen_history(first).unwrap().current.produced
                            [Good::Beer as usize]
                            + finished.citizen_history(second).unwrap().current.produced
                                [Good::Beer as usize],
                        1000
                    );
                }
                _ => {
                    assert_eq!(
                        [first, second]
                            .into_iter()
                            .filter(|id| {
                                let AgentKind::Citizen(worker) = &finished.agents[id].kind;
                                worker.garment_condition.is_some()
                            })
                            .count(),
                        1
                    );
                    assert_eq!(
                        finished.storage.units(
                            finished.households[&household].home,
                            owner,
                            Good::FlaxGarment
                        ),
                        0
                    );
                }
            }
        }
    }

    #[test]
    fn orphan_household_storage_is_rejected() {
        let world = Universe::default();
        let home = world.map.public_place(Location::Market);
        assert_eq!(
            world.with_stored_good(
                home,
                GoodsOwner::Household(AgentId(uuid::Uuid::new_v4())),
                Good::Wheat,
                1
            ),
            Err(SimulationError::AgentNotFound)
        );
    }

    #[test]
    fn shared_brewery_preparations_round_trip_and_preserve_physical_output_ownership() {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen(
                "First",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Brewer),
            )
            .unwrap();
        let (world, second) = world
            .with_citizen(
                "Second",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Brewer),
            )
            .unwrap();
        let (world, brewery) = world.with_property(first, Location::Brewery).unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        let (mut world, household) = world
            .with_household("Brewers", citizen.home(), &[first, second])
            .unwrap();
        for &(good, units) in Recipe::BrewBeer.inputs() {
            world = world
                .with_stored_good(brewery, GoodsOwner::Household(household), good, units * 2)
                .unwrap();
        }
        world = world
            .start_action(first, CitizenAction::Travel(brewery))
            .unwrap()
            .advance(0)
            .unwrap();
        world = world
            .start_action(second, CitizenAction::Travel(brewery))
            .unwrap()
            .advance(0)
            .unwrap();
        world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    place: brewery,
                    recipe: Recipe::BrewBeer,
                },
            )
            .unwrap();
        world = world
            .start_action(
                second,
                CitizenAction::ProduceAt {
                    place: brewery,
                    recipe: Recipe::BrewBeer,
                },
            )
            .unwrap();
        assert_eq!(
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
            world
        );
    }

    #[test]
    fn workplace_requirements_scale_with_designated_workers_and_shared_requests_accumulate() {
        let mut world = Universe::with_map(Map::default());
        let mut members = Vec::new();
        for index in 0..4 {
            let (next, id) = world
                .with_citizen(
                    format!("Baker{index}"),
                    Citizen::new(0.0)
                        .unwrap()
                        .with_starting_role(StartingRole::Baker)
                        .with_coins(1000)
                        .unwrap(),
                )
                .unwrap();
            world = next;
            members.push(id);
        }
        let (next, bakery) = world.with_property(members[0], Location::Bakery).unwrap();
        world = next;
        let AgentKind::Citizen(citizen) = &world.agents[&members[0]].kind;
        let (next, household) = world
            .with_household("Bakers", citizen.home(), &members)
            .unwrap();
        world = next;
        world = world.with_designated_workplace(members[0], bakery).unwrap();
        let AgentKind::Citizen(one) = &world.agents[&members[0]].kind;
        let (one_threshold, one_target) = one.stocking_requirements_at(Some(bakery)).unwrap();
        for &worker in &members[1..] {
            world = world.with_designated_workplace(worker, bakery).unwrap();
        }
        let AgentKind::Citizen(four) = &world.agents[&members[0]].kind;
        let (four_threshold, four_target) = four.stocking_requirements_at(Some(bakery)).unwrap();
        for good in Good::ALL {
            assert_eq!(four_threshold.units(good), one_threshold.units(good) * 4);
            assert_eq!(four_target.units(good), one_target.units(good) * 4);
        }
        world = world
            .with_purchase_request(
                members[0],
                crate::marketplace::ShoppingList::single(Good::Wheat, 100),
            )
            .unwrap();
        world = world
            .with_purchase_request(
                members[1],
                crate::marketplace::ShoppingList::single(Good::Flour, 200),
            )
            .unwrap();
        assert_eq!(world.market.requested(household).units(Good::Wheat), 100);
        assert_eq!(world.market.requested(household).units(Good::Flour), 200);
    }
}
#[cfg(test)]
mod physical_tests {
    use super::*;
    use crate::{
        CitizenAction, StartingRole,
        locations::{Location, Map},
        production::Recipe,
    };

    #[test]
    fn colocated_workplaces_reserve_and_consume_only_the_selected_sites_inputs() {
        let (world, worker) = Universe::with_map(Map::default())
            .with_citizen(
                "Miller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (world, first) = world.with_property(worker, Location::Mill).unwrap();
        let (world, second) = world.with_property(worker, Location::Mill).unwrap();
        let world = world
            .with_stored_good(second, GoodsOwner::Agent(worker), Good::Wheat, 600)
            .unwrap();
        assert_eq!(
            world.start_action(
                worker,
                CitizenAction::ProduceAt {
                    place: first,
                    recipe: Recipe::MillFlour
                }
            ),
            Err(SimulationError::MissingInputs)
        );
        let started = world
            .start_action(
                worker,
                CitizenAction::ProduceAt {
                    place: second,
                    recipe: Recipe::MillFlour,
                },
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &started.agents[&worker].kind;
        let finished = started
            .advance(citizen.active_action().unwrap().remaining_ms())
            .unwrap();
        assert_eq!(
            finished
                .storage
                .units(first, GoodsOwner::Agent(worker), Good::Flour),
            0
        );
        assert_eq!(
            finished
                .storage
                .units(second, GoodsOwner::Agent(worker), Good::Flour),
            240
        );
        assert_eq!(
            finished
                .storage
                .units(second, GoodsOwner::Agent(worker), Good::Wheat),
            300
        );
    }

    #[test]
    fn allegiance_counts_shared_wealth_once_and_personal_reserves_scale_per_member() {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen("First", Citizen::new(0.0).unwrap().with_coins(100).unwrap())
            .unwrap();
        let (world, second) = world
            .with_citizen(
                "Second",
                Citizen::new(0.0).unwrap().with_coins(100).unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        let home = citizen.home();
        let (world, household) = world
            .with_household("Family", home, &[first, second])
            .unwrap();
        let world = world
            .with_stored_good(home, GoodsOwner::Household(household), Good::BreadRoll, 20)
            .unwrap();
        let AgentKind::Citizen(first) = &world.agents[&first].kind;
        let AgentKind::Citizen(second) = &world.agents[&second].kind;
        assert_eq!(first.wealth().unwrap(), 0.0);
        assert_eq!(
            first.food_reserve_wellbeing(),
            crate::food_reserve_bonus(100.0)
        );
        assert_eq!(
            first.decision_score().unwrap() - first.personal_wellbeing().unwrap(),
            world.household_wealth(household).unwrap() * crate::WEALTH_WELLBEING_PER_COIN
        );
        assert_eq!(first.decision_score(), second.decision_score());
    }
}
