use crate::{
    AgentId, AgentKind, Citizen, Coins, SimulationError, Universe,
    locations::{Place, PlaceId},
};
use imbl::HashMap;

pub const FEE_HOUR_MS: u64 = 60 * 60 * 1000;

fn supported_workshop(kind: crate::locations::Location) -> bool {
    matches!(
        kind,
        crate::locations::Location::Mill
            | crate::locations::Location::Bakery
            | crate::locations::Location::Weavery
            | crate::locations::Location::Tailory
    )
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Guild {
    pub id: AgentId,
    pub name: String,
    pub members: Vec<AgentId>,
    pub workshops: Vec<PlaceId>,
    pub coins: Coins,
    pub hourly_fee: Coins,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct GuildAccess {
    pub workshops: Vec<PlaceId>,
    pub hourly_fee: Coins,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkshopFeeAccount {
    pub unpaid: Coins,
    pub remainder: u64,
}
impl WorkshopFeeAccount {
    pub fn liability(self) -> f64 {
        self.unpaid as f64 + self.remainder as f64 / FEE_HOUR_MS as f64
    }
    pub(crate) fn accrue(
        &mut self,
        hourly_fee: Coins,
        elapsed_ms: u64,
    ) -> Result<(), SimulationError> {
        let numerator = u128::from(self.remainder) + (hourly_fee as u128) * u128::from(elapsed_ms);
        let whole = Coins::try_from(numerator / u128::from(FEE_HOUR_MS))
            .map_err(|_| SimulationError::WealthOverflow)?;
        self.unpaid = self
            .unpaid
            .checked_add(whole)
            .ok_or(SimulationError::WealthOverflow)?;
        self.remainder = (numerator % u128::from(FEE_HOUR_MS)) as u64;
        Ok(())
    }
}

impl Citizen {
    pub fn guild_ids(&self) -> impl Iterator<Item = AgentId> + '_ {
        self.guilds.keys().copied()
    }
    pub fn workshop_fee_accounts(&self) -> &HashMap<AgentId, WorkshopFeeAccount> {
        &self.workshop_fees
    }
    pub fn workshop_fee_liability(&self) -> f64 {
        self.workshop_fees.values().map(|fee| fee.liability()).sum()
    }
    pub(crate) fn may_use_place(&self, place: &Place) -> bool {
        place.owner.is_some_and(|owner| self.may_use_owner(owner))
            || self
                .guilds
                .values()
                .any(|access| access.workshops.contains(&place.id))
    }
    pub(crate) fn workshop_fee_cost(&self, site: Option<PlaceId>, elapsed_ms: u64) -> f64 {
        let site = site.or(self.designated_workplace);
        self.guilds
            .values()
            .filter(|access| site.is_some_and(|site| access.workshops.contains(&site)))
            .map(|access| access.hourly_fee as f64 * elapsed_ms as f64 / FEE_HOUR_MS as f64)
            .sum()
    }
    pub(crate) fn accrue_workshop_fee(
        &mut self,
        place: PlaceId,
        elapsed_ms: u64,
    ) -> Result<Option<(AgentId, Coins)>, SimulationError> {
        let Some((payee, access)) = self
            .guilds
            .iter()
            .find(|(_, access)| access.workshops.contains(&place))
        else {
            return Ok(None);
        };
        let payee = *payee;
        let fee = self.workshop_fees.entry(payee).or_default();
        fee.accrue(access.hourly_fee, elapsed_ms)?;
        let paid = fee.unpaid.min(self.coins);
        fee.unpaid -= paid;
        self.coins -= paid;
        Ok(Some((payee, paid)))
    }
}

impl Universe {
    pub fn guilds(&self) -> &HashMap<AgentId, Guild> {
        &self.guilds
    }
    pub fn workshop_fee_accounts(&self) -> &HashMap<(AgentId, AgentId), WorkshopFeeAccount> {
        &self.workshop_fees
    }
    pub fn with_guild(
        &self,
        name: impl Into<String>,
        members: &[AgentId],
        workshops: &[PlaceId],
        hourly_fee: Coins,
    ) -> Result<(Self, AgentId), SimulationError> {
        let name = name.into();
        if name.trim().is_empty()
            || members.is_empty()
            || workshops.is_empty()
            || hourly_fee < 0
            || members
                .iter()
                .enumerate()
                .any(|(i, id)| members[..i].contains(id))
            || workshops
                .iter()
                .enumerate()
                .any(|(i, id)| workshops[..i].contains(id))
        {
            return Err(SimulationError::InvalidOwnership);
        }
        for member in members {
            let AgentKind::Citizen(citizen) = &self
                .agents
                .get(member)
                .ok_or(SimulationError::AgentNotFound)?
                .kind;
            if citizen.active_action.is_some() || citizen.active_plan.is_some() {
                return Err(SimulationError::CitizenBusy);
            }
        }
        for id in workshops {
            let place = self.map.place(*id)?;
            if !supported_workshop(place.kind)
                || !place.owner.is_some_and(|owner| members.contains(&owner))
            {
                return Err(SimulationError::InvalidOwnership);
            }
        }
        let id = AgentId(uuid::Uuid::new_v4());
        let mut universe = self.clone();
        universe.guilds.insert(
            id,
            Guild {
                id,
                name,
                members: members.to_vec(),
                workshops: workshops.to_vec(),
                coins: 0,
                hourly_fee,
            },
        );
        universe.guild_histories.insert(
            id,
            crate::guild_history::GuildHistory::new(universe.current_time_ms, 0),
        );
        for workshop in workshops {
            universe.map.places_mut().get_mut(workshop).unwrap().owner = Some(id);
        }
        for member in members {
            let AgentKind::Citizen(citizen) = &mut universe.agents.get_mut(member).unwrap().kind;
            citizen.guilds.insert(
                id,
                GuildAccess {
                    workshops: workshops.to_vec(),
                    hourly_fee,
                },
            );
        }
        universe.refresh_market();
        universe.refresh_workplace_supplies()?;
        universe.refresh_household_history(universe.current_time_ms, false)?;
        Ok((universe, id))
    }
    pub(crate) fn settle_workshop_fees(&mut self) -> Result<(), SimulationError> {
        let mut keys: Vec<_> = self.workshop_fees.keys().copied().collect();
        keys.sort_by_key(|(payer, payee)| (payer.0, payee.0));
        for key @ (payer, payee) in keys {
            let coins = self.account_coins(payer)?;
            let paid = self.workshop_fees[&key].unpaid.min(coins);
            let balance = self
                .account_coins(payee)?
                .checked_add(paid)
                .ok_or(SimulationError::WealthOverflow)?;
            self.set_account_coins(payer, coins - paid)?;
            self.set_account_coins(payee, balance)?;
            self.workshop_fees.get_mut(&key).unwrap().unpaid -= paid;
            self.record_guild_payment(payee, paid)?;
            if let Some(history) = self.household_histories.get_mut(&payer) {
                history.current.workshop_fees_paid = history
                    .current
                    .workshop_fees_paid
                    .checked_add(paid)
                    .ok_or(SimulationError::WealthOverflow)?;
            }
        }
        self.refresh_market();
        Ok(())
    }
    pub(crate) fn validate_guilds(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        let invalid = || SaveError::InvalidState("invalid guild or workshop fee account");
        for (id, guild) in &self.guilds {
            if *id != guild.id
                || self.agents.contains_key(id)
                || self.households.contains_key(id)
                || guild.name.trim().is_empty()
                || guild.coins < 0
                || guild.hourly_fee < 0
                || guild.members.is_empty()
                || guild.workshops.is_empty()
                || guild
                    .members
                    .iter()
                    .enumerate()
                    .any(|(i, member)| guild.members[..i].contains(member))
                || guild
                    .workshops
                    .iter()
                    .enumerate()
                    .any(|(i, place)| guild.workshops[..i].contains(place))
            {
                return Err(invalid());
            }
            for member in &guild.members {
                let AgentKind::Citizen(citizen) =
                    &self.agents.get(member).ok_or_else(invalid)?.kind;
                if citizen.guilds.get(id)
                    != Some(&GuildAccess {
                        workshops: guild.workshops.clone(),
                        hourly_fee: guild.hourly_fee,
                    })
                {
                    return Err(invalid());
                }
            }
            for workshop in &guild.workshops {
                if !self
                    .map
                    .place(*workshop)
                    .is_ok_and(|place| place.owner == Some(*id) && supported_workshop(place.kind))
                {
                    return Err(invalid());
                }
            }
        }
        for (id, agent) in &self.agents {
            let AgentKind::Citizen(citizen) = &agent.kind;
            if citizen.guilds.keys().any(|guild| {
                !self
                    .guilds
                    .get(guild)
                    .is_some_and(|guild| guild.members.contains(id))
            }) {
                return Err(invalid());
            }
            let expected: HashMap<_, _> = self
                .workshop_fees
                .iter()
                .filter_map(|((payer, payee), fee)| {
                    (*payer == citizen.economic_id()).then_some((*payee, *fee))
                })
                .collect();
            if citizen.workshop_fees != expected {
                return Err(invalid());
            }
        }
        for ((payer, payee), fee) in &self.workshop_fees {
            if fee.unpaid < 0
                || fee.remainder >= FEE_HOUR_MS
                || (!self.agents.contains_key(payer) && !self.households.contains_key(payer))
                || self.economic_id(*payer).ok() != Some(*payer)
                || !self.guilds.contains_key(payee)
                || !self
                    .account_workers(*payer)
                    .iter()
                    .any(|id| self.guilds[payee].members.contains(id))
            {
                return Err(invalid());
            }
        }
        for place in self.map.places().values() {
            if place.owner.is_some_and(|owner| {
                !self.agents.contains_key(&owner)
                    && !self
                        .guilds
                        .get(&owner)
                        .is_some_and(|guild| guild.workshops.contains(&place.id))
            }) {
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
        CitizenAction, StartingRole,
        locations::{Location, Map},
        marketplace::Good,
        production::Recipe,
        storage::GoodsOwner,
    };

    fn workshop(coins: Coins) -> (Universe, AgentId, AgentId, PlaceId, AgentId) {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen(
                "First",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Weaver)
                    .with_coins(coins)
                    .unwrap(),
            )
            .unwrap();
        let (world, second) = world
            .with_citizen(
                "Second",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Weaver)
                    .with_coins(coins)
                    .unwrap(),
            )
            .unwrap();
        let (world, place) = world.with_property(first, Location::Weavery).unwrap();
        let (world, guild) = world
            .with_guild("Weavers", &[first, second], &[place], 1)
            .unwrap();
        let mut world = world
            .with_designated_workplace(first, place)
            .unwrap()
            .with_designated_workplace(second, place)
            .unwrap();
        for worker in [first, second] {
            for &(good, units) in Recipe::WeaveCloth.inputs() {
                world = world
                    .with_stored_good(place, GoodsOwner::Agent(worker), good, units * 20)
                    .unwrap();
            }
        }
        (world, first, second, place, guild)
    }

    fn start(world: Universe, worker: AgentId, place: PlaceId) -> Universe {
        world
            .start_action(
                worker,
                CitizenAction::ProduceAt {
                    place,
                    recipe: Recipe::WeaveCloth,
                },
            )
            .unwrap()
    }

    #[test]
    fn shared_workshop_keeps_members_stocks_homes_accounts_and_skills_separate() {
        let (world, first, second, place, guild) = workshop(10);
        assert_eq!(world.guilds[&guild].coins, 0);
        let AgentKind::Citizen(a) = &world.agents[&first].kind;
        let AgentKind::Citizen(b) = &world.agents[&second].kind;
        assert_ne!(a.home(), b.home());
        assert_ne!(a.goods_owner(), b.goods_owner());
        assert!(b.start_action(CitizenAction::Travel(a.home())).is_err());
        assert!(b.production_place(Recipe::WeaveCloth).is_ok());
        assert!(b.production_place(Recipe::AssembleGarment).is_err());
        let (_, first_target) = a.stocking_requirements_at(Some(place)).unwrap();
        let (_, second_target) = b.stocking_requirements_at(Some(place)).unwrap();
        assert_eq!(first_target, second_target);
        let duration = a
            .action_duration_ms(CitizenAction::ProduceAt {
                place,
                recipe: Recipe::WeaveCloth,
            })
            .unwrap();
        let finished = start(world, first, place).advance(duration).unwrap();
        for &(good, units) in Recipe::WeaveCloth.outputs() {
            assert_eq!(
                finished
                    .storage
                    .units(place, GoodsOwner::Agent(first), good),
                units
            );
            assert_eq!(
                finished
                    .storage
                    .units(place, GoodsOwner::Agent(second), good),
                0
            );
        }
        let (world, outsider) = finished
            .with_citizen(
                "Visitor",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Weaver),
            )
            .unwrap();
        assert_eq!(
            world.start_action(
                outsider,
                CitizenAction::ProduceAt {
                    place,
                    recipe: Recipe::WeaveCloth
                }
            ),
            Err(SimulationError::MissingProperty)
        );
        assert!(
            world
                .start_action(outsider, CitizenAction::Travel(place))
                .is_ok()
        );
    }

    #[test]
    fn fees_are_exact_split_invariant_and_only_charge_actual_work() {
        let (world, first, _, place, guild) = workshop(10);
        let started = start(world, first, place);
        let AgentKind::Citizen(citizen) = &started.agents[&first].kind;
        let duration = citizen.active_action().unwrap().remaining_ms();
        let once = started.advance(duration).unwrap();
        let split = started
            .advance(1234)
            .unwrap()
            .advance(duration - 1234)
            .unwrap();
        assert_eq!(once.guild_history(guild), split.guild_history(guild));
        assert_eq!(once.workshop_fees, split.workshop_fees);
        assert_eq!(once.guilds[&guild].coins, split.guilds[&guild].coins);
        let fee = once.workshop_fees[&(first, guild)];
        assert_eq!(fee.remainder, duration % FEE_HOUR_MS);
        assert_eq!(fee.unpaid, 0);
        assert_eq!(once.guilds[&guild].coins, (duration / FEE_HOUR_MS) as Coins);
        let idle = once.advance(FEE_HOUR_MS).unwrap();
        assert_eq!(
            idle.guild_history(guild).unwrap().current.contributions,
            once.guild_history(guild).unwrap().current.contributions
        );
        assert_eq!(idle.workshop_fees, once.workshop_fees);
        let AgentKind::Citizen(before) = &started.agents[&first].kind;
        let predicted = before.advance_predicted(duration).unwrap();
        let AgentKind::Citizen(actual) = &once.agents[&first].kind;
        assert_eq!(predicted.workshop_fees, actual.workshop_fees);
        assert_eq!(predicted.coins(), actual.coins());
        assert_eq!(started.guilds[&guild].coins, 0);
    }

    #[test]
    fn unpaid_fees_reduce_wealth_without_blocking_work_and_settle_when_cash_arrives() {
        let (world, first, _, place, guild) = workshop(0);
        let AgentKind::Citizen(before) = &world.agents[&first].kind;
        let gross_profit =
            crate::production::recipe_profit_for_site(Recipe::WeaveCloth, before, Some(place))
                .unwrap();
        let mut free = before.clone();
        free.guilds.get_mut(&guild).unwrap().hourly_fee = 0;
        let free_profit =
            crate::production::recipe_profit_for_site(Recipe::WeaveCloth, &free, Some(place))
                .unwrap();
        let duration = before
            .action_duration_ms(CitizenAction::ProduceAt {
                place,
                recipe: Recipe::WeaveCloth,
            })
            .unwrap();
        assert!((free_profit - gross_profit - duration as f64 / FEE_HOUR_MS as f64).abs() < 1e-9);
        let mut world = world;
        for _ in 0..3 {
            world = start(world, first, place).advance(duration).unwrap();
        }
        let fee = world.workshop_fees[&(first, guild)];
        assert!(fee.unpaid > 0);
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        assert_eq!(citizen.coins(), 0);
        let mut forgiven = citizen.clone();
        forgiven.workshop_fees.clear();
        assert!(
            (forgiven.wealth().unwrap() - citizen.wealth().unwrap() - fee.liability()).abs() < 1e-9
        );
        let day = &world.guild_history(guild).unwrap().current;
        assert_eq!(day.fees_accrued, fee);
        assert_eq!(day.closing_outstanding[&first], fee);
        assert_eq!(day.payments_received, 0);
        let restored = Universe::from_save_str(&world.to_save_string().unwrap()).unwrap();
        assert_eq!(restored, world);
        world.set_account_coins(first, 10).unwrap();
        world.settle_workshop_fees().unwrap();
        world.refresh_guild_history(world.current_time_ms, false);
        let day = &world.guild_history(guild).unwrap().current;
        assert_eq!(day.payments_received, fee.unpaid);
        assert_eq!(day.closing_outstanding[&first].remainder, fee.remainder);
        assert_eq!(day.fees_accrued, fee);
        assert_eq!(world.guilds[&guild].coins, fee.unpaid);
        assert_eq!(world.workshop_fees[&(first, guild)].unpaid, 0);
        assert_eq!(
            world.workshop_fees[&(first, guild)].remainder,
            fee.remainder
        );
        assert_eq!(world.account_coins(first).unwrap(), 10 - fee.unpaid);
    }

    #[test]
    fn fee_overflow_is_atomic_and_corrupt_fee_saves_are_rejected() {
        let (world, first, _, place, guild) = workshop(10);
        let mut world = start(world, first, place);
        world.guilds.get_mut(&guild).unwrap().coins = Coins::MAX;
        world.workshop_fees.insert(
            (first, guild),
            WorkshopFeeAccount {
                unpaid: 0,
                remainder: FEE_HOUR_MS - 1,
            },
        );
        world.refresh_market();
        let before = world.clone();
        assert_eq!(
            world.advance(FEE_HOUR_MS),
            Err(SimulationError::WealthOverflow)
        );
        assert_eq!(world, before);
        let mut world = before;
        world.guilds.get_mut(&guild).unwrap().coins = 0;
        world.workshop_fees.insert(
            (first, guild),
            WorkshopFeeAccount {
                unpaid: 0,
                remainder: FEE_HOUR_MS,
            },
        );
        world.refresh_market();
        assert!(Universe::from_save_str(&world.to_save_string().unwrap()).is_err());
    }

    #[test]
    fn guild_property_fee_uses_guild_account_and_goods_tax_keeps_producer_owner() {
        use crate::taxation::{TaxKind, TaxPayer, TaxRate, TaxScope};
        let (world, first, _, place, guild) = workshop(10);
        let (world, rule) = world
            .with_scoped_tax_rule(
                TaxScope::LocationType(Location::Weavery),
                "Rent",
                TaxKind::FlatFee {
                    payer: TaxPayer::LocationOwner,
                    coins: 2,
                },
            )
            .unwrap();
        let mut world = world;
        world.guilds.get_mut(&guild).unwrap().coins = 10;
        world.tax_week(0).unwrap();
        assert_eq!(world.guilds[&guild].coins, 8);
        assert_eq!(world.account_coins(first).unwrap(), 10);
        assert_eq!(world.tax_arrears(rule, guild), 0);
        world = world
            .with_stored_good(place, GoodsOwner::Agent(first), Good::Cloth, 200)
            .unwrap();
        let (mut world, _) = world
            .with_tax_rule(
                place,
                "Production tax",
                TaxKind::Socage {
                    rates: [(Good::Cloth, TaxRate::new(5000).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        world
            .tax_production(first, place, Good::Cloth, 200, 0)
            .unwrap();
        assert_eq!(
            world
                .storage
                .reserved_units(place, GoodsOwner::Agent(first), Good::Cloth),
            100
        );
        assert!(
            world
                .taxation
                .accounts
                .keys()
                .any(|key| key.payer == first && key.good == Some(Good::Cloth))
        );
        assert!(
            !world
                .taxation
                .accounts
                .keys()
                .any(|key| key.payer == guild && key.good == Some(Good::Cloth))
        );

        assert_eq!(
            world
                .storage
                .owned_units(GoodsOwner::Agent(first), Good::Cloth),
            200
        );
    }
    #[test]
    fn concurrent_workers_share_account_fractions_without_pooling_independent_goods() {
        for shared in [false, true] {
            let (mut world, first, second, place, guild) = workshop(10);
            let account = if shared {
                let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
                let (next, household) = world
                    .with_household("Weaving family", citizen.home(), &[first, second])
                    .unwrap();
                world = next;
                household
            } else {
                first
            };
            let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
            let duration = citizen
                .action_duration_ms(CitizenAction::ProduceAt {
                    place,
                    recipe: Recipe::WeaveCloth,
                })
                .unwrap();
            let started = start(start(world, first, place), second, place);
            let once = started.advance(duration).unwrap();
            let split = started
                .advance(duration / 3)
                .unwrap()
                .advance(duration - duration / 3)
                .unwrap();
            assert_eq!(once.guild_history(guild), split.guild_history(guild));
            assert_eq!(once.workshop_fees, split.workshop_fees);
            assert_eq!(once.guilds[&guild].coins, split.guilds[&guild].coins);
            let fee = once.workshop_fees[&(account, guild)];
            let expected = duration * if shared { 2 } else { 1 };
            assert_eq!(fee.remainder, expected % FEE_HOUR_MS);
            assert_eq!(fee.unpaid, 0);
            if shared {
                let day = &once.household_histories[&account].current;
                assert_eq!(day.workshop_fees_paid, (expected / FEE_HOUR_MS) as Coins);
                assert_eq!(
                    day.opening_coins - day.workshop_fees_paid,
                    day.closing_coins
                );
            }
            assert_eq!(
                Universe::from_save_str(&once.to_save_string().unwrap()).unwrap(),
                once
            );
        }
    }

    #[test]
    fn historical_individual_debt_remains_valid_after_household_formation() {
        let (mut world, first, second, place, guild) = workshop(0);
        world.guilds.get_mut(&guild).unwrap().hourly_fee = FEE_HOUR_MS as Coins;
        for id in [first, second] {
            let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&id).unwrap().kind;
            citizen.guilds.get_mut(&guild).unwrap().hourly_fee = FEE_HOUR_MS as Coins;
        }
        world = start(world, first, place);
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        let duration = citizen.active_action().unwrap().remaining_ms();
        world = world.advance(duration).unwrap();
        let boundary = crate::marketplace::UPDATE_TIME_MS;
        world = world.advance(boundary - world.current_time_ms).unwrap();
        assert_eq!(
            world
                .guild_history(guild)
                .unwrap()
                .completed
                .back()
                .unwrap()
                .closing_outstanding[&first]
                .unpaid,
            duration as Coins
        );
        world.set_account_coins(first, duration as Coins).unwrap();
        world = world.advance(1).unwrap();
        assert_eq!(
            world.workshop_fees[&(first, guild)],
            WorkshopFeeAccount::default()
        );
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        assert!(world.market.production_trial(first, Good::Cloth).is_none());
        let cloth = citizen.resource_units(Good::Cloth).unwrap();
        let (world, account) = world
            .with_household("Family", citizen.home(), &[first, second])
            .unwrap();
        assert!(world.market.production_trial(first, Good::Cloth).is_none());
        assert_eq!(
            world
                .storage
                .owned_units(GoodsOwner::Household(account), Good::Cloth),
            cloth
        );
        assert_eq!(
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
            world
        );
    }

    #[test]
    fn guild_history_records_only_partial_work_when_advancement_is_interrupted() {
        let (world, first, _, place, guild) = workshop(10);
        let world = start(world, first, place).advance(1234).unwrap();
        let history = world.guild_history(guild).unwrap().clone();
        assert_eq!(
            history.current.contributions[&(first, place)].production_ms,
            1234
        );
        assert_eq!(
            history.current.contributions[&(first, place)].produced,
            [0; Good::COUNT]
        );
        let mut calls = 0;
        let result = world
            .advance_internal(FEE_HOUR_MS, None, &mut || {
                calls += 1;
                calls > 2
            })
            .unwrap();
        assert!(result.is_none());
        assert_eq!(world.guild_history(guild), Some(&history));
    }

    #[test]
    fn caravan_repayment_after_four_closes_old_debt_and_records_payment_in_new_day() {
        let (world, first, _, place, guild) = workshop(0);
        let mut world = world.with_prices(
            crate::marketplace::Prices::default()
                .with_price(Good::Cloth, Good::Cloth.export_threshold() * 0.5)
                .unwrap()
                .with_price(Good::Thread, 1.0)
                .unwrap(),
        );
        for _ in 0..3 {
            world = start(world, first, place);
            let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
            world = world
                .advance(citizen.active_action().unwrap().remaining_ms())
                .unwrap();
        }
        let unpaid = world.workshop_fees[&(first, guild)].unpaid;
        assert!(unpaid > 0);
        let world = world
            .start_action(first, CitizenAction::List(Good::Cloth, 200))
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert!(
            world
                .market
                .orders()
                .filter(|order| order.seller.account() == Some(first) && order.good == Good::Cloth)
                .all(|order| order.quoted_price
                    < world
                        .market
                        .effective_export_threshold(Good::Cloth)
                        .unwrap())
        );
        let boundary = crate::marketplace::UPDATE_TIME_MS;
        let before = world.advance(boundary - world.current_time_ms - 1).unwrap();
        let after = before.advance(1).unwrap();
        let history = after.guild_history(guild).unwrap();
        assert_eq!(history.completed.back().unwrap().payments_received, 0);
        assert_eq!(history.current.opening_outstanding[&first].unpaid, unpaid);
        assert_eq!(history.current.payments_received, unpaid);
        assert_eq!(history.current.closing_outstanding[&first].unpaid, 0);
        assert_eq!(history.current.fees_accrued, WorkshopFeeAccount::default());
        assert_eq!(
            Universe::from_save_str(&after.to_save_string().unwrap()).unwrap(),
            after
        );
    }

    #[test]
    fn guild_history_records_weekly_property_expense_on_closing_day() {
        use crate::taxation::{TaxKind, TaxPayer, TaxScope};
        let (mut world, _, _, _, guild) = workshop(10);
        world.guilds.get_mut(&guild).unwrap().coins = 10;
        world
            .guild_histories
            .insert(guild, crate::guild_history::GuildHistory::new(0, 10));
        let (world, _) = world
            .with_scoped_tax_rule(
                TaxScope::LocationType(Location::Weavery),
                "Rent",
                TaxKind::FlatFee {
                    payer: TaxPayer::LocationOwner,
                    coins: 2,
                },
            )
            .unwrap();
        let settlement = crate::calendar::next_weekly_settlement(0).unwrap();
        let world = world.advance(settlement).unwrap();
        let history = world.guild_history(guild).unwrap();
        let paid: Coins = history
            .completed
            .iter()
            .map(|day| day.expenses_paid)
            .sum::<Coins>()
            + history.current.expenses_paid;
        assert_eq!(paid, 2);
        assert_eq!(history.completed.back().unwrap().expenses_paid, 2);
        assert_eq!(history.current.expenses_paid, 0);
        assert_eq!(history.current.closing_coins, 8);
        assert_eq!(
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
            world
        );
    }

    #[test]
    fn guild_history_rejects_corrupted_accounting_and_activity() {
        let (world, first, _, place, guild) = workshop(10);
        let started = start(world, first, place);
        let AgentKind::Citizen(citizen) = &started.agents[&first].kind;
        let world = started
            .advance(citizen.active_action().unwrap().remaining_ms())
            .unwrap();
        for corrupt in 0..5 {
            let mut broken = world.clone();
            let day = &mut broken.guild_histories.get_mut(&guild).unwrap().current;
            match corrupt {
                0 => day.payments_received += 1,
                1 => day.fees_accrued.remainder += 1,
                2 => day.closing_outstanding.clear(),
                3 => {
                    day.contributions
                        .get_mut(&(first, place))
                        .unwrap()
                        .production_ms += 1
                }
                _ => day.end_ms += 1,
            }
            assert!(Universe::from_save_str(&broken.to_save_string().unwrap()).is_err());
        }
    }

    #[test]
    fn guild_history_splits_work_at_four_and_retains_completed_days() {
        let (world, first, _, place, guild) = workshop(10);
        let boundary = crate::production::period_start(1);
        let world = world.advance(boundary - 1000).unwrap();
        let started = start(world, first, place);
        let AgentKind::Citizen(citizen) = &started.agents[&first].kind;
        let duration = citizen.active_action().unwrap().remaining_ms();
        let whole = started.advance(duration).unwrap();
        let split = started
            .advance(1000)
            .unwrap()
            .advance(duration - 1000)
            .unwrap();
        assert_eq!(whole.guild_history(guild), split.guild_history(guild));
        let history = whole.guild_history(guild).unwrap();
        let closed = history.completed.back().unwrap();
        assert_eq!(closed.contributions[&(first, place)].production_ms, 1000);
        assert_eq!(
            closed.contributions[&(first, place)].produced,
            [0; Good::COUNT]
        );
        let current = &history.current.contributions[&(first, place)];
        assert_eq!(current.production_ms, duration - 1000);
        for &(good, units) in Recipe::WeaveCloth.outputs() {
            assert_eq!(current.produced[good as usize], units);
        }
        assert_eq!(
            Universe::from_save_str(&whole.to_save_string().unwrap()).unwrap(),
            whole
        );
        let mut retained = whole;
        for period in 2..=368 {
            retained.current_time_ms = crate::production::period_start(period);
            retained.refresh_guild_history(retained.current_time_ms, true);
        }
        assert_eq!(retained.guild_history(guild).unwrap().completed.len(), 365);
        assert_eq!(
            retained
                .guild_history(guild)
                .unwrap()
                .completed
                .front()
                .unwrap()
                .start_ms,
            crate::production::period_start(3)
        );
    }

    #[test]
    fn household_formation_rejects_outstanding_fees_and_cleans_zero_accounts() {
        let (world, first, second, place, guild) = workshop(10);
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        let home = citizen.home();
        let duration = citizen
            .action_duration_ms(CitizenAction::ProduceAt {
                place,
                recipe: Recipe::WeaveCloth,
            })
            .unwrap();
        let charged = start(world.clone(), first, place)
            .advance(duration)
            .unwrap();
        assert_eq!(
            charged.with_household("Family", home, &[first, second]),
            Err(SimulationError::InvalidOwnership)
        );
        let mut world = world;
        world
            .workshop_fees
            .insert((first, guild), WorkshopFeeAccount::default());
        world.refresh_market();
        let (world, _) = world
            .with_household("Family", home, &[first, second])
            .unwrap();
        assert!(world.workshop_fees.is_empty());
        assert_eq!(
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap(),
            world
        );
    }
    #[test]
    fn guild_creation_and_saves_reject_crop_and_brewing_workshops() {
        for kind in [Location::Field, Location::Brewery, Location::Home] {
            let (world, worker) = Universe::with_map(Map::default())
                .with_citizen("Member", Citizen::new(0.0).unwrap())
                .unwrap();
            let (world, site) = world.with_property(worker, kind).unwrap();
            assert_eq!(
                world.with_guild("Guild", &[worker], &[site], 1),
                Err(SimulationError::InvalidOwnership)
            );
        }
        let (mut world, _, _, place, _) = workshop(0);
        world.map.places_mut().get_mut(&place).unwrap().kind = Location::Brewery;
        world.map.places_mut().get_mut(&place).unwrap().batches =
            vec![None; crate::brewing::BREWERY_SLOTS];
        world.refresh_market();
        assert!(Universe::from_save_str(&world.to_save_string().unwrap()).is_err());
    }
}
