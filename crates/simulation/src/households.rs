use crate::{
    AgentId, AgentKind, Citizen, Coins, Quantity, SimulationError, Universe,
    locations::PlaceId,
    marketplace::{Good, MarketParty},
    storage::{GoodsOwner, StockKey},
};
use imbl::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Household {
    pub id: AgentId,
    pub name: String,
    pub home: Option<PlaceId>,
    pub members: Vec<AgentId>,
    pub coins: Coins,
    pub maintenance: crate::maintenance::MaintenanceState,
    pub below_zero_since_ms: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct HouseholdContext {
    pub id: AgentId,
    pub members: Vec<AgentId>,
    pub allegiance: f64,
    pub other_carried: [Quantity; Good::COUNT],
    pub other_consumption_value: f64,
    #[serde(default)]
    pub other_clothing_replacements: u64,
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
        owner == self.economic_id()
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
    pub(crate) fn sample_household_departure(
        &mut self,
        member: AgentId,
    ) -> Result<bool, SimulationError> {
        let household = self.economic_id(member)?;
        let members = &self.households[&household].members;
        let average = members.iter().try_fold(0.0, |total, id| {
            self.agents[id]
                .personal_wellbeing()
                .map(|value| total + value / members.len() as f64)
        })?;
        let state = self.households.get_mut(&household).unwrap();
        if average >= 0.0 {
            state.below_zero_since_ms = None;
        } else if let Some(start) = state.below_zero_since_ms {
            if self.current_time_ms - start >= crate::marketplace::DAY_MS {
                *self = self.remove_household(household)?;
                return Ok(true);
            }
        } else {
            state.below_zero_since_ms = Some(self.current_time_ms);
        }
        Ok(false)
    }

    pub fn households(&self) -> &HashMap<AgentId, Household> {
        &self.households
    }
    pub fn economic_id(&self, worker: AgentId) -> Result<AgentId, SimulationError> {
        if self.guilds.contains_key(&worker) || self.households.contains_key(&worker) {
            return Ok(worker);
        }
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&worker)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        citizen
            .household_id()
            .filter(|id| self.households.contains_key(id))
            .ok_or(SimulationError::InvalidOwnership)
    }
    pub(crate) fn account_owner(&self, account: AgentId) -> Result<GoodsOwner, SimulationError> {
        let account = self.economic_id(account)?;
        if self.households.contains_key(&account) {
            Ok(GoodsOwner::Household(account))
        } else if self.guilds.contains_key(&account) {
            Ok(GoodsOwner::Agent(account))
        } else {
            Err(SimulationError::AgentNotFound)
        }
    }
    pub(crate) fn account_coins(&self, account: AgentId) -> Result<Coins, SimulationError> {
        let account = self.economic_id(account)?;
        if let Some(guild) = self.guilds.get(&account) {
            return Ok(guild.coins);
        }
        if let Some(household) = self.households.get(&account) {
            return Ok(household.coins);
        }
        Err(SimulationError::AgentNotFound)
    }

    pub(crate) fn set_account_coins(
        &mut self,
        account: AgentId,
        coins: Coins,
    ) -> Result<(), SimulationError> {
        let account = self.economic_id(account)?;
        if let Some(guild) = self.guilds.get_mut(&account) {
            guild.coins = coins;
        } else if let Some(household) = self.households.get_mut(&account) {
            household.coins = coins;
        } else {
            return Err(SimulationError::AgentNotFound);
        }
        Ok(())
    }
    pub fn account_owned_units(
        &self,
        account: AgentId,
        good: Good,
    ) -> Result<Quantity, SimulationError> {
        let account = self.economic_id(account)?;
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
                        citizen.designated_workplace.or(citizen.home),
                        production_units,
                    ),
                    (citizen.home, units - production_units),
                ];
                for (place, units) in portions {
                    let Some(place) = place else { continue };
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
                    let allowed = citizen.accessible_storage_objects(place);
                    universe.storage.set_units_in(
                        key,
                        next,
                        &universe.map,
                        &universe.market,
                        Some(&allowed),
                    )?;
                }
                let retained = portions
                    .into_iter()
                    .filter(|(place, _)| place.is_none())
                    .map(|(_, units)| units)
                    .sum();
                citizen.inventory.insert(good, retained);
            }
            if let Some(home) = citizen.home {
                citizen.position = universe.map.position(home);
            }
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
        let mut own_requirements = std::collections::HashMap::new();
        for agent in self.agents.values() {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let Some(place) = citizen.designated_workplace else {
                continue;
            };
            let entry = requirements
                .entry((citizen.economic_id(), place))
                .or_insert(([0; Good::COUNT], [0; Good::COUNT]));
            let mut own = [crate::marketplace::ShoppingList::default(); 2];
            let snapshot = crate::production::ProjectionSnapshot::new(citizen);
            for (index, horizon) in [
                crate::production::PRODUCTION_BLOCK_MS,
                crate::production::DAILY_CAPACITY_MS,
            ]
            .into_iter()
            .enumerate()
            {
                let recipes = snapshot.recipes_at(citizen, Some(place), horizon)?;
                let stock =
                    crate::production::recipe_requirements_at(citizen, Some(place), &recipes)?;
                own[index] = stock;
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
            own_requirements.insert(citizen.id, (place, own));
        }
        for (_, agent) in self.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.workplace_supplies.clear();
            citizen.own_workplace_supplies.clear();
            if let Some((place, own)) = own_requirements.get(&citizen.id) {
                citizen
                    .own_workplace_supplies
                    .insert(*place, (own[0], own[1]));
            }
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

impl Universe {
    pub(crate) fn account_workers(&self, account: AgentId) -> Vec<AgentId> {
        if self.guilds.contains_key(&account) {
            return Vec::new();
        }
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
    fn household_first_creation_preserves_live_accounts_and_individual_items() {
        let alice = Citizen::new(0.0)
            .unwrap()
            .with_furnished_home()
            .unwrap()
            .with_coins(100)
            .unwrap()
            .with_good(Good::BreadRoll, 7)
            .unwrap()
            .with_home_good(Good::Water, 100)
            .unwrap();
        let (world, first) = Universe::default()
            .with_new_household("Family", "Alice", alice)
            .unwrap();
        let account = world.economic_id(first).unwrap();
        let (world, mill) = world.with_property(first, Location::Mill).unwrap();
        let world = world
            .with_stored_good(mill, GoodsOwner::Household(account), Good::Wheat, 300)
            .unwrap()
            .start_action(first, CitizenAction::Wait)
            .unwrap()
            .advance(1000)
            .unwrap();
        let before = world.household_history(account).unwrap().clone();
        let bob = Citizen::new(0.0)
            .unwrap()
            .with_furnished_home()
            .unwrap()
            .with_coins(50)
            .unwrap()
            .with_good(Good::Berries, 9)
            .unwrap()
            .with_home_good(Good::Water, 200)
            .unwrap();
        let (next, second) = world.with_household_member(account, "Bob", bob).unwrap();
        assert_eq!(next.households.len(), 1);
        assert_eq!(next.households[&account].members, vec![first, second]);
        assert_eq!(next.households[&account].coins, 150);
        assert_eq!(
            next.household_history(account).unwrap().completed,
            before.completed
        );
        assert_eq!(
            next.household_history(account)
                .unwrap()
                .current
                .opening_coins,
            100
        );
        assert_eq!(
            next.household_history(account)
                .unwrap()
                .current
                .introduced_coins,
            50
        );
        for member in [first, second] {
            let AgentKind::Citizen(citizen) = &next.agents[&member].kind;
            assert_eq!(citizen.household_id(), Some(account));
            assert_eq!(citizen.coins(), 150);
            assert!(citizen.accessible_place(mill).is_ok());
            assert_eq!(
                next.map.place(citizen.home.unwrap()).unwrap().owner,
                Some(account)
            );
        }
        for place in next
            .map
            .places()
            .values()
            .filter(|place| place.owner.is_some())
        {
            assert_eq!(place.owner, Some(account));
            assert!(
                place
                    .buildings
                    .iter()
                    .all(|building| building.owner == Some(account))
            );
        }
        let AgentKind::Citizen(alice) = &next.agents[&first].kind;
        let AgentKind::Citizen(bob) = &next.agents[&second].kind;
        assert_eq!(alice.units(Good::BreadRoll), 7);
        assert_eq!(bob.units(Good::BreadRoll), 0);
        assert_eq!(bob.units(Good::Berries), 9);
        assert_eq!(alice.units(Good::Berries), 0);
        assert_eq!(
            next.storage
                .owned_units(GoodsOwner::Household(account), Good::Wheat),
            300
        );
        assert_eq!(bob.home_units(Good::Water), 300);
        assert_eq!(world.households[&account].members, vec![first]);
        assert_eq!(world.households[&account].coins, 100);
        assert_eq!(
            Universe::from_save_str(&next.to_save_string().unwrap()).unwrap(),
            next
        );
        assert!(
            next.with_household_member(account, "Alice again", alice.clone())
                .is_err()
        );
        assert!(
            next.with_household_member(
                AgentId(uuid::Uuid::new_v4()),
                "Missing",
                Citizen::new(0.0).unwrap().with_furnished_home().unwrap()
            )
            .is_err()
        );
        assert!(
            next.with_stored_good(mill, GoodsOwner::Agent(first), Good::Wheat, 1)
                .is_err()
        );
    }

    #[test]
    fn shared_collection_and_equipment_contention_preserve_goods_and_finish_safely() {
        for scenario in 0..3 {
            let (world, first) = Universe::with_map(Map::default())
                .with_citizen(
                    "First",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_furnished_home()
                        .unwrap()
                        .with_starting_role(StartingRole::Brewer),
                )
                .unwrap();
            let (world, second) = world
                .with_household_member(
                    match &world.agents()[&first].kind {
                        crate::AgentKind::Citizen(citizen) => citizen.household_id().unwrap(),
                    },
                    "Second",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_furnished_home()
                        .unwrap()
                        .with_starting_role(StartingRole::Brewer),
                )
                .unwrap();
            let (world, site) = world.with_property(first, Location::Brewery).unwrap();
            let AgentKind::Citizen(_citizen) = &world.agents[&first].kind;
            let household = match &world.agents()[&first].kind {
                crate::AgentKind::Citizen(citizen) => citizen.household_id().unwrap(),
            };
            let mut world = world;
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
                    let home = world.households[&household].home.unwrap();
                    world = world
                        .with_stored_good(home, owner, Good::FlaxGarment, 1)
                        .unwrap();
                    CitizenAction::EquipClothing
                }
            };
            let started = world.start_action(first, action).unwrap();
            let started = if scenario == 1 {
                assert_eq!(
                    started.start_action(second, action),
                    Err(SimulationError::MissingInputs)
                );
                started
            } else {
                started.start_action(second, action).unwrap()
            };
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
                            finished.households[&household].home.unwrap(),
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
                    .with_furnished_home()
                    .unwrap()
                    .with_starting_role(StartingRole::Brewer),
            )
            .unwrap();
        let (world, second) = world
            .with_household_member(
                match &world.agents()[&first].kind {
                    crate::AgentKind::Citizen(citizen) => citizen.household_id().unwrap(),
                },
                "Second",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_starting_role(StartingRole::Brewer),
            )
            .unwrap();
        let (world, brewery) = world.with_property(first, Location::Brewery).unwrap();
        let AgentKind::Citizen(_citizen) = &world.agents[&first].kind;
        let household = match &world.agents()[&first].kind {
            crate::AgentKind::Citizen(citizen) => citizen.household_id().unwrap(),
        };
        let mut world = world;
        let room = &mut world.map.places_mut().get_mut(&brewery).unwrap().buildings[0].rooms[0];
        room.size = crate::locations::RoomSize::Large;
        room.objects.push(crate::locations::Object::new(
            crate::locations::ObjectKind::BrewingEquipment,
        ));
        world.refresh_market();
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
        let mut household = None;
        for index in 0..4 {
            let citizen = Citizen::new(0.0)
                .unwrap()
                .with_furnished_home()
                .unwrap()
                .with_starting_role(StartingRole::Baker)
                .with_coins(1000)
                .unwrap();
            let (next, id) = if let Some(household) = household {
                world.with_household_member(household, format!("Baker{index}"), citizen)
            } else {
                world.with_new_household("Bakers", format!("Baker{index}"), citizen)
            }
            .unwrap();
            world = next;
            let AgentKind::Citizen(citizen) = &world.agents[&id].kind;
            household = citizen.household_id();
            members.push(id);
        }
        let household = household.unwrap();
        let (next, bakery) = world.with_property(members[0], Location::Bakery).unwrap();
        world = next;
        world = world.with_designated_workplace(members[0], bakery).unwrap();
        let AgentKind::Citizen(one) = &world.agents[&members[0]].kind;
        let (one_threshold, one_target) = one.stocking_requirements_at(Some(bakery)).unwrap();
        assert_eq!(
            one.own_workplace_supplies.get(&bakery),
            Some(&(one_threshold, one_target))
        );
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
        let mut legacy = world.clone();
        for (_, agent) in legacy.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.own_workplace_supplies.clear();
        }
        let loaded = Universe::from_save_str(&legacy.to_save_string().unwrap()).unwrap();
        assert_eq!(loaded, world);
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
                    .with_furnished_home()
                    .unwrap()
                    .with_starting_role(StartingRole::Miller),
            )
            .unwrap();
        let (world, first) = world.with_property(worker, Location::Mill).unwrap();
        let (world, second) = world.with_property(worker, Location::Mill).unwrap();
        let world = world
            .with_stored_good(
                second,
                match &world.agents()[&worker].kind {
                    crate::AgentKind::Citizen(citizen) => citizen.goods_owner(),
                },
                Good::Wheat,
                600,
            )
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
            finished.storage.units(
                first,
                match &finished.agents()[&worker].kind {
                    crate::AgentKind::Citizen(citizen) => citizen.goods_owner(),
                },
                Good::Flour
            ),
            0
        );
        assert_eq!(
            finished.storage.units(
                second,
                match &finished.agents()[&worker].kind {
                    crate::AgentKind::Citizen(citizen) => citizen.goods_owner(),
                },
                Good::Flour
            ),
            240
        );
        assert_eq!(
            finished.storage.units(
                second,
                match &finished.agents()[&worker].kind {
                    crate::AgentKind::Citizen(citizen) => citizen.goods_owner(),
                },
                Good::Wheat
            ),
            300
        );
    }

    #[test]
    fn allegiance_counts_shared_wealth_once_and_personal_reserves_scale_per_member() {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen(
                "First",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(100)
                    .unwrap(),
            )
            .unwrap();
        let (world, second) = world
            .with_household_member(
                match &world.agents()[&first].kind {
                    crate::AgentKind::Citizen(citizen) => citizen.household_id().unwrap(),
                },
                "Second",
                Citizen::new(0.0)
                    .unwrap()
                    .with_furnished_home()
                    .unwrap()
                    .with_coins(100)
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        let home = citizen.home().unwrap();
        let household = match &world.agents()[&first].kind {
            crate::AgentKind::Citizen(citizen) => citizen.household_id().unwrap(),
        };

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

#[cfg(test)]
mod departure_tests {
    use super::*;
    use crate::{CitizenAction, PlanningRuntime, marketplace::DAY_MS};
    const HOUR: u64 = 3_600_000;

    fn world() -> (Universe, AgentId, AgentId) {
        let (world, member) = Universe::default()
            .with_citizen("Ada", Citizen::with_needs(100.0, 0.0).unwrap())
            .unwrap();
        let household = world.economic_id(member).unwrap();
        assert_eq!(world.households[&household].below_zero_since_ms, None);
        (world, member, household)
    }

    #[test]
    fn departure_samples_only_at_planning_start_and_never_polls_time() {
        let (world, member, household) = world();
        let world = world.advance(DAY_MS).unwrap();
        assert_eq!(world.households[&household].below_zero_since_ms, None);
        let world = world.start_planning(member).unwrap();
        assert_eq!(
            world.households[&household].below_zero_since_ms,
            Some(DAY_MS)
        );
        assert_eq!(
            world.start_planning(member),
            Err(SimulationError::CitizenBusy)
        );
    }

    #[test]
    fn departure_shared_average_zero_recovery_and_member_changes() {
        let (mut world, first, household) = world();
        let (next, second) = world
            .with_household_member(household, "B", Citizen::new(0.0).unwrap())
            .unwrap();
        world = next;
        assert!(!world.sample_household_departure(first).unwrap());
        assert_eq!(world.households[&household].below_zero_since_ms, Some(0));
        world.current_time_ms = HOUR;
        assert!(!world.sample_household_departure(second).unwrap());
        assert_eq!(world.households[&household].below_zero_since_ms, Some(0));
        for (_, agent) in world.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.hunger = 0.0;
            citizen.garment_condition = Some(1.0);
            citizen.hunger = citizen.personal_wellbeing().unwrap();
            assert_eq!(citizen.personal_wellbeing().unwrap(), 0.0);
        }
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&first).unwrap().kind;
        citizen.hunger += 1.0;
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&second).unwrap().kind;
        citizen.hunger -= 1.0;
        assert!(!world.sample_household_departure(second).unwrap());
        assert_eq!(world.households[&household].below_zero_since_ms, None);
        for (_, agent) in world.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.hunger -= 2.0;
        }
        assert!(!world.sample_household_departure(first).unwrap());
        assert_eq!(world.households[&household].below_zero_since_ms, None);
        for (_, agent) in world.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            citizen.hunger += 4.0;
        }
        assert!(!world.sample_household_departure(first).unwrap());
        assert_eq!(world.households[&household].below_zero_since_ms, Some(HOUR));
        world = world.remove_citizen(first).unwrap();
        assert_eq!(world.households[&household].below_zero_since_ms, Some(HOUR));
        world.current_time_ms = DAY_MS + HOUR - 1;
        assert!(!world.sample_household_departure(second).unwrap());
        world.current_time_ms += 1;
        assert!(world.sample_household_departure(second).unwrap());
        assert!(world.agents.is_empty());
    }

    #[test]
    fn departure_timer_round_trip_and_future_state_rejected() {
        let (mut world, member, household) = world();
        world.sample_household_departure(member).unwrap();
        world = world.advance(HOUR).unwrap();
        let saved = world.to_save_string().unwrap();
        let mut restored = Universe::from_save_str(&saved).unwrap();
        assert_eq!(restored, world);
        restored
            .households
            .get_mut(&household)
            .unwrap()
            .below_zero_since_ms = Some(HOUR + 1);
        assert!(Universe::from_save_str(&restored.to_save_string().unwrap()).is_err());
        restored = world;
        restored.current_time_ms = DAY_MS;
        let restored = restored.start_planning(member).unwrap();
        assert!(restored.agents.is_empty());
    }

    #[test]
    fn departure_different_member_boundaries_start_once_and_predictions_do_not_sample() {
        let source = crate::planning::executing_fixture(
            &Citizen::with_needs(1000.0, 100.0).unwrap(),
            vec![CitizenAction::Sleep],
        );
        let (world, first) = Universe::default()
            .with_citizen("Ada", source.clone())
            .unwrap();
        let household = world.economic_id(first).unwrap();
        let mut second = source.advance(HOUR).unwrap();
        second.id = AgentId(uuid::Uuid::new_v4());
        let (mut world, second_id) = world
            .with_household_member(household, "B", second.clone())
            .unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&second_id).unwrap().kind;
        citizen.active_action = second.active_action;
        citizen.active_plan = second.active_plan;
        let saved = world.clone();
        let mut runtime = PlanningRuntime::default();
        let window = world
            .advance_with_planner(&mut runtime, 7 * HOUR - crate::background::LEAD_MS, || {
                false
            })
            .unwrap()
            .unwrap();
        assert_eq!(world, saved);
        assert_eq!(window.households[&household].below_zero_since_ms, None);
        let boundary = window
            .advance_with_planner(&mut runtime, crate::background::LEAD_MS, || false)
            .unwrap()
            .unwrap();
        assert_eq!(
            boundary.households[&household].below_zero_since_ms,
            Some(7 * HOUR)
        );
        let next = boundary
            .advance_with_planner(&mut runtime, HOUR, || false)
            .unwrap()
            .unwrap();
        assert_eq!(
            next.households[&household].below_zero_since_ms,
            Some(7 * HOUR)
        );
    }

    #[test]
    fn departure_actual_goal_boundary_matches_sync_and_background_and_skips_removed_members() {
        for background in [false, true] {
            let source = crate::planning::executing_fixture(
                &Citizen::with_needs(1000.0, 100.0).unwrap(),
                vec![CitizenAction::Sleep],
            );
            let (mut world, first) = Universe::starting_at(DAY_MS - crate::SLEEP_DURATION_MS)
                .with_citizen("Ada", source.clone())
                .unwrap();
            let household = world.economic_id(first).unwrap();
            let mut source = source;
            source.id = AgentId(uuid::Uuid::new_v4());
            let (next, second) = world
                .with_household_member(household, "B", source.clone())
                .unwrap();
            world = next;
            let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&second).unwrap().kind;
            citizen.active_action = source.active_action;
            citizen.active_plan = source.active_plan;
            world
                .households
                .get_mut(&household)
                .unwrap()
                .below_zero_since_ms = Some(0);
            let mut runtime = PlanningRuntime::default();
            let window = world
                .advance_with_planner(
                    &mut runtime,
                    crate::SLEEP_DURATION_MS - crate::background::LEAD_MS,
                    || false,
                )
                .unwrap()
                .unwrap();
            assert_eq!(window.households[&household].below_zero_since_ms, Some(0));
            assert_eq!(world.households[&household].below_zero_since_ms, Some(0));
            let result = if background {
                window
                    .advance_with_planner(&mut runtime, crate::background::LEAD_MS, || false)
                    .unwrap()
                    .unwrap()
            } else {
                window.advance(crate::background::LEAD_MS).unwrap()
            };
            assert_eq!(result.current_time_ms(), DAY_MS);
            assert!(result.agents.is_empty());
            assert!(result.households.is_empty());
            if background {
                assert!(runtime.history(first).is_none());
                assert!(runtime.history(second).is_none());
            }
            assert_eq!(
                Universe::from_save_str(&result.to_save_string().unwrap()).unwrap(),
                result
            );
        }
    }
}
