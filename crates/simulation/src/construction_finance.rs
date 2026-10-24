//! Household cash protection and carpenter pricing. Estimates value consumption,
//! not purchases: production inputs and capital purchases are not lifestyle costs.
//! Reference prices do not promise that the market can supply the quoted volume.
use crate::{
    AgentId, AgentKind, Coins, Quantity, SimulationError, Universe,
    marketplace::{DAY_MS, Good, MarketParty},
};

pub const RESERVE_DAYS: f64 = 7.0;
pub const REPLENISHMENT_DAYS: f64 = 7.0;
pub const FALLBACK_EARNING_HOURS: f64 = 8.0;
pub const MIN_EARNING_HOURS: f64 = 1.0;
const HOUR_MS: f64 = 3_600_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct HouseholdBudget {
    pub account: AgentId,
    pub cash: Coins,
    pub liabilities: Coins,
    pub commitments: Coins,
    pub history_days: usize,
    pub essential_daily_cost: f64,
    pub consumption_daily_cost: f64,
    pub taxes_daily_cost: f64,
    pub fees_daily_cost: f64,
    pub recurring_daily_cost: f64,
    pub essential_reserve: Coins,
    pub earning_hours_per_day: f64,
    pub replenishment_daily_cost: f64,
    pub hourly_earnings_target: f64,
    pub maximum_offer: Coins,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CarpenterQuote {
    pub material_value: f64,
    pub procurement_hours: f64,
    pub construction_hours: f64,
    pub labour_value: f64,
    pub payment: Coins,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructionFinancing {
    pub procurement_cash: Coins,
    pub living_cash: Coins,
    pub required_cash: Coins,
    pub available_cash: Coins,
    pub can_finance: bool,
}

fn ceil_coins(value: f64) -> Result<Coins, SimulationError> {
    if !value.is_finite() || value < 0.0 || value.ceil() >= i64::MAX as f64 {
        return Err(SimulationError::WealthOverflow);
    }
    Ok(value.ceil() as Coins)
}

impl Universe {
    /// Whole-project market estimate, including procurement for every work portion.
    pub fn construction_estimate(
        &self,
        client: AgentId,
        place: crate::locations::PlaceId,
        base_ms: u64,
        wood: Quantity,
    ) -> Result<Coins, SimulationError> {
        use crate::production::{Skill, skill_duration_reduction};
        if base_ms == 0 {
            return if wood == 0 {
                Ok(0)
            } else {
                Err(SimulationError::InvalidQuantity)
            };
        }
        let mut workers = Vec::new();
        for agent in self.agents.values() {
            let AgentKind::Citizen(worker) = &agent.kind;
            if let Some(practice) = worker.skill_practice_ms(Skill::Carpentry) {
                let budget = self.household_budget(worker.economic_id(), 0)?;
                let travel = worker.position().distance(self.map.position(place))
                    * crate::locations::WALK_MS_PER_METRE
                    / HOUR_MS;
                workers.push((
                    budget.account,
                    budget.hourly_earnings_target,
                    1.0 - skill_duration_reduction(practice),
                    travel,
                ));
            }
        }
        if workers.is_empty() {
            // A missing profession must not make labour appear free to clients.
            let food = Good::FOOD
                .into_iter()
                .map(|good| {
                    self.construction_estimated_unit_price(client, good)
                        / good.nutrition_per_unit().unwrap()
                })
                .min_by(f64::total_cmp)
                .unwrap();
            let daily = crate::HUNGER_PER_HOUR * 24.0 * food
                + 400.0 * self.construction_estimated_unit_price(client, Good::Water);
            workers.push((client, daily / FALLBACK_EARNING_HOURS, 1.0, 0.0));
        }
        let portion_ms = crate::construction::MILESTONE_BASE_MS;
        let mut elapsed = 0;
        let mut estimate = 0.0;
        while elapsed < base_ms {
            let end = elapsed.saturating_add(portion_ms).min(base_ms);
            let units_through =
                |ms| (u128::from(wood) * u128::from(ms)).div_ceil(u128::from(base_ms)) as Quantity;
            let portion_wood = units_through(end) - units_through(elapsed);
            let hours = (end - elapsed) as f64 / HOUR_MS;
            let mut quotes: Vec<_> = workers
                .iter()
                .map(|(account, hourly, factor, travel)| {
                    portion_wood as f64
                        * self.construction_estimated_unit_price(*account, Good::Wood)
                        + (hours * factor + travel + if portion_wood > 0 { 2.0 } else { 0.0 })
                            * hourly
                })
                .collect();
            quotes.sort_by(f64::total_cmp);
            let middle = quotes.len() / 2;
            estimate += if quotes.len() % 2 == 0 {
                (quotes[middle - 1] + quotes[middle]) / 2.0
            } else {
                quotes[middle]
            };
            elapsed = end;
        }
        ceil_coins(estimate * 1.2)
    }

    pub fn construction_tender_offer(
        &self,
        client: AgentId,
        place: crate::locations::PlaceId,
        base_ms: u64,
        wood: Quantity,
    ) -> Result<Coins, SimulationError> {
        Ok(self
            .construction_estimate(client, place, base_ms, wood)?
            .min(self.construction_budget(client)?.maximum_offer))
    }

    /// Lowest obtainable offer, otherwise the world's current reference price.
    pub fn construction_estimated_unit_price(&self, account: AgentId, good: Good) -> f64 {
        let account = self.economic_id(account).unwrap_or(account);
        let party = if self.households.contains_key(&account) {
            MarketParty::Household(account)
        } else {
            MarketParty::Citizen(account)
        };
        self.market
            .lowest_purchase_quote(party, good, &self.map)
            .unwrap_or_else(|| {
                self.prices()
                    .price(good)
                    .unwrap_or_else(|| good.core_price())
            })
            / good.units_per_price_unit() as f64
    }

    /// Commitments exclude existing tax and workshop liabilities, counted here.
    pub fn household_budget(
        &self,
        member_or_account: AgentId,
        commitments: Coins,
    ) -> Result<HouseholdBudget, SimulationError> {
        if commitments < 0 {
            return Err(SimulationError::InvalidQuantity);
        }
        let account = self.economic_id(member_or_account)?;
        let members = if let Some(household) = self.households.get(&account) {
            household.members.clone()
        } else if self.agents.contains_key(&account) {
            vec![account]
        } else {
            return Err(SimulationError::AgentNotFound);
        };
        let cash = self.account_coins(account)?;
        let scale = i128::from(crate::guilds::FEE_HOUR_MS);
        let tax_liabilities = self
            .tax_rules()
            .keys()
            .map(|rule| i128::from(self.tax_arrears(*rule, account)))
            .sum::<i128>();
        let fee_liabilities = self
            .workshop_fee_accounts()
            .iter()
            .filter(|((payer, _), _)| *payer == account)
            .map(|(_, fee)| i128::from(fee.unpaid) * scale + i128::from(fee.remainder))
            .sum::<i128>();
        let liabilities = Coins::try_from(tax_liabilities + (fee_liabilities + scale - 1) / scale)
            .map_err(|_| SimulationError::WealthOverflow)?;
        let price = |good| self.construction_estimated_unit_price(account, good);
        let available_food: Vec<_> = Good::FOOD
            .into_iter()
            .filter(|good| {
                let party = if self.households.contains_key(&account) {
                    MarketParty::Household(account)
                } else {
                    MarketParty::Citizen(account)
                };
                self.market
                    .lowest_purchase_quote(party, *good, &self.map)
                    .is_some()
            })
            .collect();
        let food = if available_food.is_empty() {
            Good::FOOD.to_vec()
        } else {
            available_food
        };
        let food_per_nutrition = food
            .into_iter()
            .map(|good| price(good) / good.nutrition_per_unit().unwrap())
            .min_by(f64::total_cmp)
            .unwrap();
        let mut essential_daily_cost = 0.0;
        let mut clothing_daily_cost = 0.0;
        for member in &members {
            let AgentKind::Citizen(citizen) = &self.agents[member].kind;
            essential_daily_cost +=
                citizen.hunger_per_hour() * 24.0 * food_per_nutrition + 400.0 * price(Good::Water);
            if citizen.garment_condition().is_some() {
                clothing_daily_cost +=
                    price(Good::FlaxGarment) * DAY_MS as f64 / crate::GARMENT_LIFETIME_MS as f64;
            }
        }
        let days: Vec<_> = self
            .household_history(account)
            .into_iter()
            .flat_map(|history| history.completed.iter().rev())
            .filter(|day| day.end_ms - day.start_ms == DAY_MS)
            .take(7)
            .collect();
        let history_days = days.len();
        let mut consumed = 0.0;
        let mut taxes = 0.0;
        let mut fees = 0.0;
        for day in &days {
            taxes += day.taxes_paid as f64;
            fees += day.workshop_fees_paid as f64;
        }
        for member in &members {
            let personal_days: Vec<_> = self
                .citizen_history(*member)
                .into_iter()
                .flat_map(|history| history.completed.iter().rev())
                .filter(|day| day.elapsed_ms == DAY_MS)
                .take(7)
                .collect();
            let personal_cost = personal_days
                .iter()
                .map(|day| {
                    Good::FOOD
                        .into_iter()
                        .chain([Good::Water, Good::Beer])
                        .map(|good| day.personal_consumed[good as usize] as f64 * price(good))
                        .sum::<f64>()
                })
                .sum::<f64>();
            consumed += personal_cost / personal_days.len().max(1) as f64;
        }
        let singleton_days: Vec<_> = if self.households.contains_key(&account) {
            Vec::new()
        } else {
            self.citizen_history(account)
                .into_iter()
                .flat_map(|history| history.completed.iter().rev())
                .filter(|day| day.elapsed_ms == DAY_MS)
                .take(7)
                .collect()
        };
        let history_days = history_days.max(singleton_days.len());
        if let (Some(latest), Some(earliest)) = (singleton_days.first(), singleton_days.last()) {
            taxes = self
                .tax_history()
                .iter()
                .filter(|receipt| {
                    receipt.payer == account
                        && (receipt.time_ms > earliest.start_ms || earliest.start_ms == 0)
                        && receipt.time_ms <= latest.end_ms
                })
                .map(|receipt| match receipt.amount {
                    crate::taxation::TaxAmount::Coins { paid, .. } => paid as f64,
                    _ => 0.0,
                })
                .sum();
            fees = singleton_days
                .iter()
                .map(|day| day.workshop_fees_paid as f64)
                .sum();
        }
        let divisor = history_days.max(1) as f64;
        let consumption_daily_cost = consumed.max(essential_daily_cost) + clothing_daily_cost;
        let taxes_daily_cost = taxes / divisor;
        let fees_daily_cost = fees / divisor;
        let recurring_daily_cost = consumption_daily_cost + taxes_daily_cost + fees_daily_cost;
        let mut earning_hours_per_day = 0.0;
        for member in &members {
            let history: Vec<_> = self
                .citizen_history(*member)
                .into_iter()
                .flat_map(|history| history.completed.iter().rev())
                .filter(|day| day.elapsed_ms == DAY_MS)
                .take(7)
                .collect();
            // Food goals also contain meals: subtraction conservatively excludes
            // some earning time because existing history cannot separate overlaps.
            let hours = if history.is_empty() {
                FALLBACK_EARNING_HOURS
            } else {
                history
                    .iter()
                    .map(|day| {
                        (day.activity.production_ms
                            + day.activity.travel_ms
                            + day.activity.trade_ms)
                            .saturating_sub(day.food_goal_ms) as f64
                            / HOUR_MS
                    })
                    .sum::<f64>()
                    / history.len() as f64
            };
            earning_hours_per_day += hours.clamp(MIN_EARNING_HOURS, 24.0);
        }
        let essential_reserve = ceil_coins(essential_daily_cost * RESERVE_DAYS)?;
        let free_cash = cash
            .saturating_sub(commitments)
            .saturating_sub(liabilities)
            .max(0);
        let replenishment_daily_cost =
            essential_reserve.saturating_sub(free_cash).max(0) as f64 / REPLENISHMENT_DAYS;
        let hourly_earnings_target =
            (recurring_daily_cost + replenishment_daily_cost) / earning_hours_per_day;
        let maximum_offer = free_cash.saturating_sub(essential_reserve).max(0);
        Ok(HouseholdBudget {
            account,
            cash,
            liabilities,
            commitments,
            history_days,
            essential_daily_cost,
            consumption_daily_cost,
            taxes_daily_cost,
            fees_daily_cost,
            recurring_daily_cost,
            essential_reserve,
            earning_hours_per_day,
            replenishment_daily_cost,
            hourly_earnings_target,
            maximum_offer,
        })
    }

    /// Expected construction hours already include the worker's skill reduction.
    pub fn carpenter_quote(
        &self,
        budget: &HouseholdBudget,
        materials: &[(Good, Quantity)],
        construction_hours: f64,
        procurement_hours: f64,
    ) -> Result<CarpenterQuote, SimulationError> {
        if [construction_hours, procurement_hours]
            .iter()
            .any(|hours| !hours.is_finite() || *hours < 0.0)
        {
            return Err(SimulationError::InvalidQuantity);
        }
        let material_value = materials
            .iter()
            .map(|(good, units)| {
                *units as f64 * self.construction_estimated_unit_price(budget.account, *good)
            })
            .sum();
        let labour_value = (construction_hours + procurement_hours) * budget.hourly_earnings_target;
        let payment = ceil_coins(material_value + labour_value)?;
        Ok(CarpenterQuote {
            material_value,
            procurement_hours,
            construction_hours,
            labour_value,
            payment,
        })
    }
}

impl HouseholdBudget {
    /// Procurement cash is only missing materials; owned materials still enter quotes.
    /// Days until payment include needs breaks, travel and supply delays. No future
    /// income is presumed available before payment.
    pub fn construction_financing(
        &self,
        procurement_cash: Coins,
        days_until_payment: f64,
    ) -> Result<ConstructionFinancing, SimulationError> {
        if procurement_cash < 0 || !days_until_payment.is_finite() || days_until_payment < 0.0 {
            return Err(SimulationError::InvalidQuantity);
        }
        let living_cash = ceil_coins(self.recurring_daily_cost * days_until_payment)?;
        let required_cash = procurement_cash
            .checked_add(living_cash)
            .ok_or(SimulationError::WealthOverflow)?;
        let available_cash = self
            .cash
            .saturating_sub(self.commitments)
            .saturating_sub(self.liabilities)
            .max(0);
        Ok(ConstructionFinancing {
            procurement_cash,
            living_cash,
            required_cash,
            available_cash,
            can_finance: required_cash <= available_cash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Citizen, locations::Map};

    #[test]
    fn project_estimate_prices_each_portion_and_adds_twenty_percent() {
        let (world, worker) = Universe::with_map(Map::default())
            .with_citizen(
                "Carpenter",
                Citizen::new(0.0)
                    .unwrap()
                    .with_coins(100_000)
                    .unwrap()
                    .with_skill_practice_ms(crate::production::Skill::Carpentry, Some(0))
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
        let place = citizen.home();
        let target = world
            .household_budget(worker, 0)
            .unwrap()
            .hourly_earnings_target;
        let material = 4_800.0 * world.construction_estimated_unit_price(worker, Good::Wood);
        assert_eq!(
            world
                .construction_estimate(worker, place, 24 * 3_600_000, 4_800)
                .unwrap(),
            ((material + 28.0 * target) * 1.2).ceil() as Coins
        );
        let busy = world
            .start_action(worker, crate::CitizenAction::Sleep)
            .unwrap();
        assert_eq!(
            busy.construction_estimate(worker, place, 24 * 3_600_000, 4_800)
                .unwrap(),
            world
                .construction_estimate(worker, place, 24 * 3_600_000, 4_800)
                .unwrap()
        );
    }

    #[test]
    fn estimates_use_median_skill_and_cap_offers_at_available_cash() {
        let (mut world, client, _, _) = family(100_000);
        let AgentKind::Citizen(client_citizen) = &world.agents[&client].kind;
        let place = client_citizen.home();
        for practice in [0, crate::production::MAX_SKILL_PRACTICE_MS] {
            (world, _) = world
                .with_citizen(
                    "Carpenter",
                    Citizen::new(0.0)
                        .unwrap()
                        .with_coins(100_000)
                        .unwrap()
                        .with_skill_practice_ms(crate::production::Skill::Carpentry, Some(practice))
                        .unwrap(),
                )
                .unwrap();
        }
        let workers: Vec<_> = world
            .agents
            .iter()
            .filter_map(|(id, agent)| {
                let AgentKind::Citizen(worker) = &agent.kind;
                worker
                    .skill_practice_ms(crate::production::Skill::Carpentry)
                    .map(|_| *id)
            })
            .collect();
        let hourly = world
            .household_budget(workers[0], 0)
            .unwrap()
            .hourly_earnings_target;
        let material = 200.0 * world.construction_estimated_unit_price(client, Good::Wood);
        let travel = workers
            .iter()
            .map(|id| {
                let AgentKind::Citizen(worker) = &world.agents[id].kind;
                worker.position().distance(world.map.position(place))
                    * crate::locations::WALK_MS_PER_METRE
                    / HOUR_MS
            })
            .sum::<f64>()
            / workers.len() as f64;
        let estimate = world
            .construction_estimate(client, place, 3_600_000, 200)
            .unwrap();
        assert_eq!(
            estimate,
            ((material + (2.75 + travel) * hourly) * 1.2).ceil() as Coins
        );
        assert_eq!(
            world
                .construction_tender_offer(client, place, 3_600_000, 200)
                .unwrap(),
            estimate
        );
        let account = world.economic_id(client).unwrap();
        let reserve = world.household_budget(client, 0).unwrap().essential_reserve;
        world.set_account_coins(account, reserve + 1).unwrap();
        assert_eq!(
            world
                .construction_tender_offer(client, place, 3_600_000, 200)
                .unwrap(),
            1
        );
    }

    #[test]
    fn no_carpenter_estimate_keeps_unskilled_labour_and_remaining_material_cost() {
        let (world, client, _, _) = family(100_000);
        let AgentKind::Citizen(client_citizen) = &world.agents[&client].kind;
        let place = client_citizen.home();
        let materials = 200.0 * world.construction_estimated_unit_price(client, Good::Wood);
        let full = world
            .construction_estimate(client, place, 3_600_000, 200)
            .unwrap();
        let installed = world
            .construction_estimate(client, place, 3_600_000, 0)
            .unwrap();
        assert!(full > (materials * 1.2).ceil() as Coins);
        assert!(installed > 0 && installed < full);
        assert_eq!(world.construction_estimate(client, place, 0, 0).unwrap(), 0);
    }

    fn family(coins: Coins) -> (Universe, AgentId, AgentId, AgentId) {
        let (world, first) = Universe::with_map(Map::default())
            .with_citizen(
                "First",
                Citizen::new(0.0).unwrap().with_coins(coins).unwrap(),
            )
            .unwrap();
        let (world, second) = world
            .with_citizen("Second", Citizen::new(0.0).unwrap())
            .unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&first].kind;
        let home = citizen.home();
        let (world, account) = world
            .with_household("Family", home, &[first, second])
            .unwrap();
        (world, first, second, account)
    }

    #[test]
    fn shared_reserve_and_commitments_are_counted_once() {
        let (world, first, second, account) = family(100_000);
        let first_budget = world.household_budget(first, 500).unwrap();
        assert_eq!(first_budget, world.household_budget(second, 500).unwrap());
        assert_eq!(first_budget, world.household_budget(account, 500).unwrap());
        assert_eq!(first_budget.earning_hours_per_day, 16.0);
        assert_eq!(
            first_budget.maximum_offer,
            100_000 - 500 - first_budget.essential_reserve
        );
        assert!(first_budget.essential_reserve > 0);
    }

    #[test]
    fn self_provisioning_and_deprivation_do_not_remove_essential_reserve() {
        let (mut world, first, _, account) = family(0);
        world = world
            .advance(DAY_MS + crate::marketplace::UPDATE_TIME_MS)
            .unwrap();
        let budget = world.household_budget(first, 0).unwrap();
        assert_eq!(budget.maximum_offer, 0);
        assert!(budget.hourly_earnings_target.is_finite());
        assert!(budget.earning_hours_per_day >= 2.0);
        assert!(budget.replenishment_daily_cost > 0.0);
        assert!(budget.consumption_daily_cost >= budget.essential_daily_cost);
        let home = world.households[&account].home;
        let world = world
            .with_stored_good(
                home,
                crate::storage::GoodsOwner::Household(account),
                Good::Berries,
                100_000,
            )
            .unwrap();
        assert_eq!(
            world
                .household_budget(account, 0)
                .unwrap()
                .essential_reserve,
            budget.essential_reserve
        );
    }

    #[test]
    fn production_consumption_and_material_purchases_do_not_become_lifestyle() {
        let (world, first, _, account) = family(100_000);
        let mut world = world
            .advance(DAY_MS + crate::marketplace::UPDATE_TIME_MS)
            .unwrap();
        let baseline = world.household_budget(account, 0).unwrap();
        let day = world
            .citizen_histories
            .get_mut(&first)
            .unwrap()
            .completed
            .back_mut()
            .unwrap();
        day.consumed[Good::Water as usize] += 1_000_000;
        day.consumed[Good::Berries as usize] += 1_000_000;
        let household_day = world
            .household_histories
            .get_mut(&account)
            .unwrap()
            .completed
            .back_mut()
            .unwrap();
        household_day.purchases += 10_000;
        household_day.bought[Good::Wood as usize] += 1_000_000;
        assert_eq!(world.household_budget(account, 0).unwrap(), baseline);
        world
            .citizen_histories
            .get_mut(&first)
            .unwrap()
            .completed
            .back_mut()
            .unwrap()
            .personal_consumed[Good::Beer as usize] = 1_000_000;
        assert!(
            world
                .household_budget(account, 0)
                .unwrap()
                .recurring_daily_cost
                > baseline.recurring_daily_cost
        );
    }

    #[test]
    fn large_balances_preserve_single_coin_commitments_and_reserve() {
        let (world, first, _, _) = family(i64::MAX - 100);
        let free = world.household_budget(first, 0).unwrap();
        let committed = world.household_budget(first, 1).unwrap();
        assert_eq!(committed.maximum_offer, free.maximum_offer - 1);
        assert_eq!(
            committed.maximum_offer,
            committed.cash - 1 - committed.essential_reserve
        );
        assert_eq!(
            committed
                .construction_financing(0, 0.0)
                .unwrap()
                .available_cash,
            committed.cash - 1
        );
        let mut world = world;
        let liability = (1_i64 << 53) + 1;
        world.workshop_fees.insert(
            (committed.account, first),
            crate::guilds::WorkshopFeeAccount {
                unpaid: liability,
                remainder: 1,
            },
        );
        let indebted = world.household_budget(first, 1).unwrap();
        assert_eq!(indebted.liabilities, liability + 1);
        assert_eq!(
            indebted.maximum_offer,
            indebted.cash - 1 - liability - 1 - indebted.essential_reserve
        );
    }

    #[test]
    fn standalone_citizens_use_personal_consumption_and_fee_history() {
        let (world, member) = Universe::with_map(Map::default())
            .with_citizen(
                "Worker",
                Citizen::new(0.0).unwrap().with_coins(100_000).unwrap(),
            )
            .unwrap();
        let mut world = world
            .advance(DAY_MS + crate::marketplace::UPDATE_TIME_MS)
            .unwrap();
        let day = world
            .citizen_histories
            .get_mut(&member)
            .unwrap()
            .completed
            .back_mut()
            .unwrap();
        day.workshop_fees_paid = 100;
        day.personal_consumed[Good::Beer as usize] = 10_000;
        world
            .taxation
            .history
            .push_back(crate::taxation::TaxReceipt {
                time_ms: DAY_MS,
                rule: crate::taxation::TaxRuleId(99),
                place: world.map.public_place(crate::locations::Location::Market),
                payer: member,
                good: None,
                amount: crate::taxation::TaxAmount::Coins {
                    assessed: 50,
                    paid: 50,
                    arrears: 0,
                },
            });
        let budget = world.household_budget(member, 0).unwrap();
        assert_eq!(budget.history_days, 1);
        assert_eq!(budget.taxes_daily_cost, 50.0);
        assert_eq!(budget.fees_daily_cost, 100.0);
        assert!(budget.consumption_daily_cost > budget.essential_daily_cost);
    }

    #[test]
    fn observed_earning_time_and_existing_liabilities_reduce_available_funds() {
        let (world, first, second, account) = family(100_000);
        let mut world = world
            .advance(DAY_MS + crate::marketplace::UPDATE_TIME_MS)
            .unwrap();
        for member in [first, second] {
            let day = world
                .citizen_histories
                .get_mut(&member)
                .unwrap()
                .completed
                .back_mut()
                .unwrap();
            day.activity.production_ms = 6 * 3_600_000;
            day.activity.travel_ms = 2 * 3_600_000;
            day.activity.trade_ms = 3_600_000;
            day.food_goal_ms = 3 * 3_600_000;
        }
        world.workshop_fees.insert(
            (account, first),
            crate::guilds::WorkshopFeeAccount {
                unpaid: 400,
                remainder: 0,
            },
        );
        let budget = world.household_budget(account, 200).unwrap();
        assert_eq!(budget.earning_hours_per_day, 12.0);
        assert_eq!(budget.liabilities, 400);
        assert_eq!(
            budget.maximum_offer,
            100_000 - 600 - budget.essential_reserve
        );
        let financing = budget.construction_financing(0, 0.0).unwrap();
        assert_eq!(financing.available_cash, 99_400);
    }

    #[test]
    fn owned_materials_have_quote_value_but_do_not_require_procurement_cash() {
        let (world, first, _, _) = family(100_000);
        let budget = world.household_budget(first, 0).unwrap();
        let quote = world
            .carpenter_quote(&budget, &[(Good::Wood, 4_000)], 20.0, 2.0)
            .unwrap();
        assert_eq!(
            quote.material_value,
            4_000.0 * world.construction_estimated_unit_price(budget.account, Good::Wood)
        );
        assert_eq!(quote.labour_value, 22.0 * budget.hourly_earnings_target);
        let short = budget.construction_financing(0, 1.0).unwrap();
        let delayed = budget.construction_financing(0, 1_000_000.0).unwrap();
        assert!(short.can_finance);
        assert!(!delayed.can_finance);
        assert!(delayed.living_cash > short.living_cash);
        assert!(world.carpenter_quote(&budget, &[], f64::NAN, 0.0).is_err());
    }
}
