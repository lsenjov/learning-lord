use crate::{
    AgentId, AgentKind, Coins, SimulationError, Universe,
    construction::{ConstructionClient, ContractStatus},
    locations::{Location, ObjectKind},
    marketplace::{Good, MarketParty, ShoppingList},
    storage::{GoodsOwner, StockKey},
    taxation::{TaxAmount, TaxKind, TaxPayer, TaxReceipt},
};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct DepartedCitizen {
    pub name: String,
    pub household: AgentId,
    pub guilds: Vec<AgentId>,
    pub time_ms: u64,
    pub history: crate::history::CitizenHistory,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct DepartedHousehold {
    pub name: String,
    pub time_ms: u64,
    pub history: crate::household_history::HouseholdHistory,
}

impl Universe {
    pub fn departed_citizens(&self) -> &imbl::HashMap<AgentId, DepartedCitizen> {
        &self.departed_citizens
    }

    pub fn departed_households(&self) -> &imbl::HashMap<AgentId, DepartedHousehold> {
        &self.departed_households
    }

    pub(crate) fn historical_member(&self, citizen: AgentId, household: AgentId) -> bool {
        self.households
            .get(&household)
            .is_some_and(|h| h.members.contains(&citizen))
            || self
                .departed_citizens
                .get(&citizen)
                .is_some_and(|c| c.household == household)
    }

    pub fn remove_citizen(&self, id: AgentId) -> Result<Self, SimulationError> {
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        let household = citizen.economic_id();
        if self.households[&household].members.len() == 1 {
            return self.remove_household(household);
        }
        self.depart(household, &[id], false)
    }

    pub fn remove_household(&self, id: AgentId) -> Result<Self, SimulationError> {
        let household = self
            .households
            .get(&id)
            .ok_or(SimulationError::AgentNotFound)?;
        self.depart(id, &household.members, true)
    }

    fn depart(
        &self,
        household: AgentId,
        members: &[AgentId],
        whole: bool,
    ) -> Result<Self, SimulationError> {
        let mut world = self.clone();
        let affected: Vec<_> = world
            .construction
            .contracts
            .values()
            .filter(|contract| {
                contract.status.open()
                    && (whole
                        && world.construction.projects[&contract.project].client
                            == ConstructionClient::Account(household)
                        || contract.carpenter.is_some_and(|id| members.contains(&id)))
            })
            .map(|contract| contract.id)
            .collect();
        for contract in affected {
            if let Some(worker) = world.construction.contracts[&contract].carpenter {
                let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&worker).unwrap().kind;
                if citizen.active_plan.is_none() {
                    citizen.map = world.map.clone();
                    citizen.storage = world.storage.clone();
                    citizen.abandon_action()?;
                    citizen.market_request = ShoppingList::default();
                    citizen.request_group = None;
                    world.map = citizen.map.clone();
                    world.storage = citizen.storage.clone();
                }
            }
            world.close_construction(contract, ContractStatus::Cancelled)?;
        }
        for (_, contract) in world.construction.contracts.iter_mut() {
            contract
                .quotes
                .retain(|quote| !members.contains(&quote.carpenter));
        }
        world.compact_construction();
        for id in members {
            world.cancel_maintenance(*id)?;
            let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(id).unwrap().kind;
            citizen.map = world.map.clone();
            citizen.storage = world.storage.clone();
            citizen.abandon_action()?;
            world.map = citizen.map.clone();
            world.storage = citizen.storage.clone();
        }
        let taxes = world.taxation.history.len();
        world.settle_departure_reservations(world.current_time_ms, members, whole)?;
        if whole {
            let mut keys: Vec<_> = world
                .taxation
                .accounts
                .keys()
                .filter(|key| key.payer == household)
                .copied()
                .collect();
            keys.sort_by_key(|key| (key.rule, key.place.0, key.good.map(|good| good as usize)));
            for key in keys {
                world.collect_coin_account(key, 0, world.current_time_ms)?;
                let account = world.taxation.accounts.remove(&key).unwrap();
                let goods = account.goods_due_scaled as f64 / 10_000.0;
                let fractional_coins =
                    account.income_remainder as f64 / 10_000.0 + account.asset_remainder;
                if account.arrears > 0 || fractional_coins > 0.0 || goods > 0.0 {
                    world.taxation.record(TaxReceipt {
                        time_ms: world.current_time_ms,
                        rule: key.rule,
                        place: key.place,
                        payer: household,
                        good: key.good,
                        amount: TaxAmount::WrittenOff {
                            coins: account.arrears,
                            fractional_coins,
                            goods,
                        },
                    });
                }
            }
            let mut fees: Vec<_> = world
                .workshop_fees
                .keys()
                .filter(|(payer, _)| *payer == household)
                .copied()
                .collect();
            fees.sort_by_key(|(_, guild)| guild.0);
            for key in fees {
                let fee = world.workshop_fees.remove(&key).unwrap();
                let paid = fee.unpaid.min(world.households[&household].coins);
                world.households.get_mut(&household).unwrap().coins -= paid;
                let guild = world.guilds.get_mut(&key.1).unwrap();
                guild.coins = guild
                    .coins
                    .checked_add(paid)
                    .ok_or(SimulationError::WealthOverflow)?;
                world.record_guild_payment(key.1, paid)?;
                let total = &mut world
                    .household_histories
                    .get_mut(&household)
                    .unwrap()
                    .current
                    .workshop_fees_paid;
                *total = total
                    .checked_add(paid)
                    .ok_or(SimulationError::WealthOverflow)?;
                let loss = &mut world
                    .guild_histories
                    .get_mut(&key.1)
                    .unwrap()
                    .current
                    .fees_written_off;
                loss.accrue(1, fee.remainder)?;
                loss.unpaid = loss
                    .unpaid
                    .checked_add(fee.unpaid - paid)
                    .ok_or(SimulationError::WealthOverflow)?;
            }
        }
        world.record_household_events(world.market.trades().len(), taxes)?;
        let coins = if whole {
            world.households[&household].coins
        } else {
            let committed = world.construction_commitments(household, None)?;
            let liabilities = world.household_budget(household, committed)?.liabilities;
            world.households[&household]
                .coins
                .saturating_sub(committed)
                .saturating_sub(liabilities)
                .max(0)
                / world.households[&household].members.len() as Coins
        };
        world.households.get_mut(&household).unwrap().coins -= coins;
        let total = &mut world
            .household_histories
            .get_mut(&household)
            .unwrap()
            .current
            .departed_coins;
        *total = total
            .checked_add(coins)
            .ok_or(SimulationError::WealthOverflow)?;
        if whole {
            let orders: Vec<_> = world
                .market
                .orders()
                .filter(|order| order.seller == MarketParty::Household(household))
                .cloned()
                .collect();
            for order in orders {
                world
                    .market
                    .withdraw(order.seller, order.place, order.good, order.units)?;
            }
            let stocks: Vec<_> = world
                .storage
                .stock()
                .keys()
                .filter(|key| key.owner == GoodsOwner::Household(household))
                .copied()
                .collect();
            for key in stocks {
                world.storage.set_units(key, 0, &world.map, &world.market)?;
            }
            for (_, place) in world.map.places_mut().iter_mut() {
                if place.owner == Some(household) {
                    place.owner = None;
                }
                for building in &mut place.buildings {
                    if building.owner == Some(household) {
                        building.owner = None;
                    }
                }
            }
            for (_, project) in world.construction.projects.iter_mut() {
                if project.client == ConstructionClient::Account(household) {
                    project.client = ConstructionClient::Town;
                }
            }
            let rules: Vec<_> = world.taxation.rules.iter().filter_map(|(id, rule)| matches!(rule.kind, TaxKind::FlatFee {payer: TaxPayer::Agent(payer), ..} if payer == household).then_some(*id)).collect();
            for id in rules {
                world.taxation.rules.get_mut(&id).unwrap().active = false;
            }
        }
        for id in members {
            let AgentKind::Citizen(citizen) = &world.agents[id].kind;
            for good in Good::ALL {
                let units = citizen.town_carried_units(good);
                if units == 0 {
                    continue;
                }
                let place = world
                    .map
                    .places()
                    .values()
                    .find(|place| place.kind == Location::Warehouse)
                    .ok_or(SimulationError::MissingProperty)?
                    .id;
                let key = StockKey {
                    place,
                    owner: GoodsOwner::Town,
                    good,
                };
                let next = world
                    .storage
                    .units(place, GoodsOwner::Town, good)
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?;
                world
                    .storage
                    .set_units(key, next, &world.map, &world.market)?;
            }
        }
        world.refresh_market();
        world.refresh_household_history(world.current_time_ms, false)?;
        for id in members {
            let agent = world.agents.remove(id).unwrap();
            let AgentKind::Citizen(citizen) = agent.kind;
            world.departed_citizens.insert(
                *id,
                DepartedCitizen {
                    name: agent.name,
                    household,
                    guilds: citizen.guild_ids().collect(),
                    time_ms: world.current_time_ms,
                    history: world.citizen_histories.remove(id).unwrap(),
                },
            );
            world
                .households
                .get_mut(&household)
                .unwrap()
                .members
                .retain(|member| member != id);
            for (_, guild) in world.guilds.iter_mut() {
                guild.members.retain(|member| member != id);
            }
            world.construction.released_carpenters.remove(id);
        }
        for (_, place) in world.map.places_mut().iter_mut() {
            for object in place.objects_mut() {
                if matches!(object.kind, ObjectKind::Bed {resident: Some(id)} if members.contains(&id))
                {
                    object.kind = ObjectKind::Bed { resident: None };
                }
            }
        }
        if whole {
            let old = world.households.remove(&household).unwrap();
            world.departed_households.insert(
                household,
                DepartedHousehold {
                    name: old.name,
                    time_ms: world.current_time_ms,
                    history: world.household_histories.remove(&household).unwrap(),
                },
            );
        } else {
            let remaining = world.households[&household].members.clone();
            for id in remaining {
                let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&id).unwrap().kind;
                citizen.household.as_mut().unwrap().members =
                    world.households[&household].members.clone();
            }
        }
        world
            .market
            .remove_departed(members, whole.then_some(household));
        world.refresh_account_requests()?;
        world.refresh_tax_rates();
        world.refresh_construction_context()?;
        world.refresh_market();
        world.refresh_workplace_supplies()?;
        world.refresh_household_history(world.current_time_ms, false)?;
        world.refresh_guild_history(world.current_time_ms, false);
        Ok(world)
    }

    pub(crate) fn validate_departures(&self) -> Result<(), crate::persistence::SaveError> {
        let invalid = || crate::persistence::SaveError::InvalidState("invalid departure history");
        for (id, citizen) in &self.departed_citizens {
            if self.departed_households.contains_key(id)
                || self.agents.contains_key(id)
                || self.households.contains_key(id)
                || self.guilds.contains_key(id)
                || citizen.name.trim().is_empty()
                || citizen.time_ms > self.current_time_ms
                || !(self.households.contains_key(&citizen.household)
                    || self.departed_households.contains_key(&citizen.household))
                || citizen
                    .guilds
                    .iter()
                    .any(|guild| !self.guilds.contains_key(guild))
            {
                return Err(invalid());
            }
            citizen.history.validate_save(citizen.time_ms)?;
        }
        for (id, household) in &self.departed_households {
            if self.households.contains_key(id)
                || self.agents.contains_key(id)
                || self.guilds.contains_key(id)
                || household.name.trim().is_empty()
                || household.time_ms > self.current_time_ms
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Citizen, CitizenAction, StartingRole,
        locations::Map,
        taxation::{AccountKey, TaxAccount},
    };

    fn citizen(coins: Coins) -> Citizen {
        Citizen::new(-100.0)
            .unwrap()
            .with_thirst(-100.0)
            .unwrap()
            .with_coins(coins)
            .unwrap()
            .with_starting_role(StartingRole::Weaver)
    }

    fn pair(coins: Coins) -> (Universe, AgentId, AgentId, AgentId) {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen("First", citizen(coins).with_furnished_home().unwrap())
            .unwrap();
        let account = world.economic_id(first).unwrap();
        let (world, second) = world
            .with_household_member(
                account,
                "Second",
                citizen(0)
                    .with_good(Good::Berries, 73)
                    .unwrap()
                    .with_good(Good::Flax, 73)
                    .unwrap()
                    .with_garment_condition(Some(1.0))
                    .unwrap(),
            )
            .unwrap();
        (world, account, first, second)
    }

    fn round_trip(world: &Universe) {
        let saved = world.to_save_string().unwrap();
        let restored = Universe::from_save_str(&saved).unwrap();
        assert_eq!(restored, *world);
    }

    #[test]
    fn individual_takes_share_and_personal_belongings_leaves_shared_property_and_history() {
        let (world, account, first, second) = pair(11);
        let home = world.households[&account].home.unwrap();
        let world = world
            .with_stored_good(home, GoodsOwner::Household(account), Good::Berries, 200)
            .unwrap();
        let world = world
            .start_action(second, CitizenAction::Sleep)
            .unwrap()
            .advance(30_000)
            .unwrap();
        let map = world.map.clone();
        let world = world.remove_citizen(second).unwrap();
        assert_eq!(world.households[&account].members, vec![first]);
        assert_eq!(world.households[&account].coins, 6);
        assert_eq!(
            world.household_histories[&account].current.departed_coins,
            5
        );
        assert_eq!(
            world
                .storage
                .units(home, GoodsOwner::Household(account), Good::Berries),
            200
        );
        assert_eq!(world.map, map);
        assert!(!world.agents.contains_key(&second));
        assert_eq!(world.town_stock(Good::Berries), 200);
        assert_eq!(world.town_stock(Good::Flax), 0);
        assert_eq!(world.town_stock(Good::FlaxGarment), 0);
        let AgentKind::Citizen(remaining) = &world.agents[&first].kind;
        assert_eq!(remaining.garment_condition(), None);
        assert!(world.departed_citizens.contains_key(&second));
        assert!(
            world.departed_citizens[&second]
                .history
                .current
                .activity
                .sleep_ms
                > 0
        );
        assert!(world.agents.contains_key(&first));
        round_trip(&world);
        round_trip(&world.advance(1000).unwrap());
    }

    #[test]
    fn final_member_and_full_household_use_same_cleanup_and_town_keeps_buildings() {
        let (world, account, first, second) = pair(11);
        let home = world.households[&account].home.unwrap();
        let world = world
            .with_stored_good(home, GoodsOwner::Household(account), Good::Berries, 200)
            .unwrap();
        let remaining = world.remove_citizen(second).unwrap();
        let actual = remaining.remove_citizen(first).unwrap();
        assert_eq!(actual, remaining.remove_household(account).unwrap());
        assert!(actual.agents.is_empty());
        assert!(actual.households.is_empty());
        let site = actual.map.place(home).unwrap();
        assert_eq!(site.kind, Location::Home);
        assert_eq!(site.owner, None);
        assert!(
            site.buildings
                .iter()
                .all(|building| building.owner.is_none())
        );
        assert_eq!(
            actual
                .storage
                .owned_units(GoodsOwner::Household(account), Good::Berries),
            0
        );
        assert_eq!(
            actual.departed_households[&account]
                .history
                .current
                .departed_coins,
            11
        );
        round_trip(&actual);
        round_trip(&actual.advance(1000).unwrap());
        let full = world.remove_household(account).unwrap();
        assert_eq!(full.departed_citizens.len(), 2);
        round_trip(&full);
    }

    #[test]
    fn single_departure_retains_debt_and_only_divides_uncommitted_coins() {
        let (mut world, account, _, second) = pair(11);
        let home = world.households[&account].home.unwrap();
        let (next, rule) = world
            .with_tax_rule(
                home,
                "Fee",
                TaxKind::FlatFee {
                    payer: TaxPayer::Agent(account),
                    coins: 1,
                },
            )
            .unwrap();
        world = next;
        let key = AccountKey {
            place: home,
            rule,
            payer: account,
            good: None,
        };
        world.taxation.accounts.insert(
            key,
            TaxAccount {
                arrears: 4,
                ..Default::default()
            },
        );
        let world = world.remove_citizen(second).unwrap();
        assert_eq!(world.households[&account].coins, 8);
        assert_eq!(world.taxation.accounts[&key].arrears, 4);
        assert_eq!(world.town_treasury, 0);
        round_trip(&world);
    }

    #[test]
    fn whole_departure_pays_then_writes_off_taxes_and_guild_fees_empty_guild_is_valid() {
        for funds in [3, 10] {
            let (world, account, first, second) = pair(funds);
            let (world, workshop) = world.with_property(first, Location::Weavery).unwrap();
            let (mut world, guild) = world
                .with_guild("Weavers", &[first, second], &[workshop], 1)
                .unwrap();
            let home = world.households[&account].home.unwrap();
            let (next, rule) = world
                .with_tax_rule(
                    home,
                    "Fee",
                    TaxKind::FlatFee {
                        payer: TaxPayer::Agent(account),
                        coins: 1,
                    },
                )
                .unwrap();
            world = next;
            let key = AccountKey {
                place: home,
                rule,
                payer: account,
                good: None,
            };
            world.taxation.accounts.insert(
                key,
                TaxAccount {
                    arrears: 5,
                    income_remainder: 1000,
                    asset_remainder: 0.2,
                    ..Default::default()
                },
            );
            let fee = crate::guilds::WorkshopFeeAccount {
                unpaid: 2,
                remainder: 123,
            };
            world.workshop_fees.insert((account, guild), fee);
            world
                .guild_histories
                .get_mut(&guild)
                .unwrap()
                .current
                .opening_outstanding
                .insert(account, fee);
            world.refresh_market();
            let world = world.remove_household(account).unwrap();
            assert_eq!(world.town_treasury, funds.min(5));
            assert!(world.taxation.accounts.is_empty());
            assert!(world.workshop_fees.is_empty());
            assert!(world.guilds[&guild].members.is_empty());
            assert_eq!(world.map.place(workshop).unwrap().owner, Some(guild));
            let paid = (funds - 5).clamp(0, 2);
            assert_eq!(world.guilds[&guild].coins, paid);
            assert_eq!(
                world.guild_histories[&guild].current.fees_written_off,
                crate::guilds::WorkshopFeeAccount {
                    unpaid: fee.unpaid - paid,
                    remainder: fee.remainder
                }
            );
            assert_eq!(
                world.departed_households[&account]
                    .history
                    .current
                    .departed_coins,
                (funds - 7).max(0)
            );
            assert!(
            world
                .tax_history()
                .iter()
                .any(|r| matches!(r.amount, TaxAmount::WrittenOff { coins, fractional_coins, .. } if coins == (5 - funds).max(0) && (fractional_coins - 0.3).abs() < 1e-9))
        );
            round_trip(&world);
            round_trip(&world.advance(1000).unwrap());
        }
    }

    #[test]
    fn removal_releases_active_production_and_preserves_foreign_and_town_stock() {
        let (world, account, first, _) = pair(0);
        let (world, workshop) = world.with_property(first, Location::Weavery).unwrap();
        let (world, foreign) = world.with_citizen("Other", citizen(0)).unwrap();
        let foreign_account = world.economic_id(foreign).unwrap();
        let world = world
            .with_stored_good(workshop, GoodsOwner::Household(account), Good::Flax, 5000)
            .unwrap()
            .with_stored_good(
                workshop,
                GoodsOwner::Household(foreign_account),
                Good::Flax,
                3,
            )
            .unwrap()
            .with_stored_good(workshop, GoodsOwner::Town, Good::Flax, 7)
            .unwrap();
        let world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    place: workshop,
                    recipe: crate::production::Recipe::SpinThread,
                },
            )
            .unwrap();
        assert!(
            world
                .storage
                .work_allocations()
                .keys()
                .any(|(worker, _)| *worker == first)
        );
        let world = world.remove_household(account).unwrap();
        assert!(world.storage.work_allocations().is_empty());
        assert!(
            world
                .map
                .place(workshop)
                .unwrap()
                .objects()
                .all(|object| object.workers.is_empty())
        );
        assert_eq!(
            world
                .storage
                .units(workshop, GoodsOwner::Household(foreign_account), Good::Flax),
            3
        );
        assert_eq!(
            world.storage.units(workshop, GoodsOwner::Town, Good::Flax),
            7
        );
        assert_eq!(
            world
                .storage
                .units(workshop, GoodsOwner::Household(account), Good::Flax),
            0
        );
        round_trip(&world);
    }

    #[test]
    fn missing_targets_fail_without_changing_world() {
        let (world, _, _, _) = pair(1);
        let before = world.clone();
        let missing = AgentId(uuid::Uuid::new_v4());
        assert_eq!(
            world.remove_citizen(missing),
            Err(SimulationError::AgentNotFound)
        );
        assert_eq!(
            world.remove_household(missing),
            Err(SimulationError::AgentNotFound)
        );
        assert_eq!(world, before);
    }
    #[test]
    fn single_departure_keeps_household_guild_debt_when_last_guild_member_leaves() {
        let (world, account, first, second) = pair(0);
        let (world, workshop) = world.with_property(first, Location::Weavery).unwrap();
        let (world, guild) = world
            .with_guild("Weavers", &[first], &[workshop], 2)
            .unwrap();
        let world = world
            .with_stored_good(workshop, GoodsOwner::Household(account), Good::Flax, 5000)
            .unwrap();
        let world = world
            .start_action(
                first,
                CitizenAction::ProduceAt {
                    place: workshop,
                    recipe: crate::production::Recipe::SpinThread,
                },
            )
            .unwrap()
            .advance(30 * 60 * 1000)
            .unwrap();
        let fee = world.workshop_fees[&(account, guild)];
        assert!(fee.liability() > 0.0);
        let world = world.remove_citizen(first).unwrap();
        assert!(world.guilds[&guild].members.is_empty());
        assert_eq!(world.households[&account].members, vec![second]);
        assert_eq!(world.workshop_fees[&(account, guild)], fee);
        assert!(
            world.guild_histories[&guild]
                .current
                .contributions
                .keys()
                .any(|(member, _)| *member == first)
        );
        assert!(
            world.household_histories[&account]
                .current
                .members
                .contains_key(&first)
        );
        round_trip(&world);
        round_trip(&world.advance(1000).unwrap());
        let world = world.remove_household(account).unwrap();
        assert_eq!(world.guild_histories[&guild].current.fees_written_off, fee);
        round_trip(&world);
    }

    #[test]
    fn whole_departure_takes_listings_keeps_other_buildings_at_same_site() {
        let (world, account, first, second) = pair(0);
        let (world, site) = world.with_property(first, Location::Weavery).unwrap();
        let (mut world, other) = world.with_citizen("Other", citizen(0)).unwrap();
        let other_account = world.economic_id(other).unwrap();
        let (map, building) = world
            .map
            .with_building(site, Location::Tailory, other_account, "Other building")
            .unwrap();
        world.map = map;
        world.refresh_market();
        let world = world
            .start_action(
                second,
                CitizenAction::ListAt {
                    place: site,
                    list: ShoppingList::single(Good::Flax, 40),
                },
            )
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(
            world
                .market
                .listed_units(MarketParty::Household(account), Good::Flax),
            40
        );
        let world = world.remove_household(account).unwrap();
        assert_eq!(
            world
                .market
                .listed_units(MarketParty::Household(account), Good::Flax),
            0
        );
        assert!(
            world
                .map
                .place(site)
                .unwrap()
                .buildings
                .iter()
                .any(|b| b.id == building && b.owner == Some(other_account))
        );
        round_trip(&world);
    }
    #[test]
    fn removal_rebuilds_household_purchase_requests() {
        let (world, account, first, second) = pair(100);
        let world = world
            .with_purchase_request(first, ShoppingList::single(Good::Water, 20))
            .unwrap()
            .with_purchase_request(second, ShoppingList::single(Good::Berries, 10))
            .unwrap();
        let world = world.remove_citizen(second).unwrap();
        assert_eq!(world.market.requested(account).units(Good::Water), 20);
        assert_eq!(world.market.requested(account).units(Good::Berries), 0);
        round_trip(&world);
        let world = world.remove_citizen(first).unwrap();
        assert_eq!(world.market.requested(account), ShoppingList::default());
        round_trip(&world);
    }

    #[test]
    fn save_rejects_invalid_departed_identity_history_and_reopened_tax_rule() {
        let (world, account, first, _) = pair(1);
        let home = world.households[&account].home.unwrap();
        let (world, rule) = world
            .with_tax_rule(
                home,
                "Rent",
                TaxKind::FlatFee {
                    payer: TaxPayer::Agent(account),
                    coins: 1,
                },
            )
            .unwrap();
        let world = world.remove_household(account).unwrap();
        for mutation in 0..4 {
            let mut invalid = world.clone();
            match mutation {
                0 => {
                    invalid.departed_citizens.get_mut(&first).unwrap().household =
                        AgentId(uuid::Uuid::new_v4())
                }
                1 => invalid.departed_citizens.get_mut(&first).unwrap().time_ms += 1,
                2 => {
                    invalid
                        .departed_households
                        .get_mut(&account)
                        .unwrap()
                        .history
                        .current
                        .departed_coins += 1
                }
                3 => invalid.taxation.rules.get_mut(&rule).unwrap().active = true,
                _ => unreachable!(),
            }
            assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        }
    }
}
