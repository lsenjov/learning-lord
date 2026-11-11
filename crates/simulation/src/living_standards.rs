use crate::{AgentId, AgentKind, Citizen, SimulationError, Universe, marketplace::DAY_MS};

pub(crate) const fn base_tier() -> u8 {
    1
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivingStandardResult {
    Promoted,
    Demoted,
    Stayed,
    PromotionCooldown,
    AwaitingFreshDays,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct LivingStandardCheck {
    pub time_ms: u64,
    pub income: i128,
    pub expenditure: i128,
    pub result: LivingStandardResult,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct LivingStandard {
    pub tier: u8,
    pub since_ms: u64,
    pub promotion_allowed_ms: u64,
    pub last_check: Option<LivingStandardCheck>,
}

impl Default for LivingStandard {
    fn default() -> Self {
        Self::new(u64::MAX)
    }
}
impl LivingStandard {
    pub fn new(now: u64) -> Self {
        Self {
            tier: 1,
            since_ms: now,
            promotion_allowed_ms: now,
            last_check: None,
        }
    }
    pub fn next_check_ms(&self, now: u64) -> u64 {
        crate::production::period_start(crate::production::work_period(now).saturating_add(1))
    }

    fn change(&mut self, tier: u8, now: u64) {
        if tier < self.tier {
            self.promotion_allowed_ms = now.saturating_add(14 * DAY_MS);
        }
        self.tier = tier;
        self.since_ms = now;
    }
}

impl Citizen {
    pub fn living_tier(&self) -> u8 {
        self.household
            .as_ref()
            .map_or(5, |context| context.living_tier)
    }
    pub fn diet_category_active(&self, category: crate::diet::DietCategory) -> bool {
        self.living_tier()
            >= if category == crate::diet::DietCategory::Sweets {
                5
            } else {
                3
            }
    }
    pub(crate) fn active_diet_intake(&self, good: crate::marketplace::Good) -> [f64; 4] {
        let mut intake = good.diet_per_unit();
        for category in crate::diet::DietCategory::ALL {
            if !self.diet_category_active(category) {
                intake[category as usize] = 0.0;
            }
        }
        intake
    }
    pub fn alcohol(&self) -> f64 {
        self.alcohol
    }
    pub fn alcohol_wellbeing(&self) -> f64 {
        if self.living_tier() >= 4 {
            crate::diet::category_bonus(self.alcohol)
        } else {
            0.0
        }
    }
    pub(crate) fn alcohol_rate(&self, active: Option<crate::ActiveAction>) -> f64 {
        let mut rate = -crate::diet::DIET_DECAY_PER_DAY / DAY_MS as f64;
        if let Some(action) = active
            && self.living_tier() >= 4
            && action.drink_good == crate::marketplace::Good::Beer
            && action.drink_duration_ms() > action.duration_ms - action.remaining_ms
        {
            rate += action.drink_units as f64 * 0.05 / action.drink_duration_ms() as f64;
        }
        rate
    }
}

impl Universe {
    pub fn set_household_living_tier(
        &self,
        id: AgentId,
        tier: u8,
    ) -> Result<Self, SimulationError> {
        let mut world = self.clone();
        let state = &mut world
            .households
            .get_mut(&id)
            .ok_or(SimulationError::AgentNotFound)?
            .living_standard;
        state.change(tier.clamp(1, 5), world.current_time_ms);
        state.last_check = None;
        world.invalidate_household_living_plans(id)?;
        world.refresh_living_tiers();
        Ok(world)
    }
    pub(crate) fn invalidate_household_living_plans(
        &mut self,
        id: AgentId,
    ) -> Result<(), SimulationError> {
        let members = self.households[&id].members.clone();
        for member in members {
            self.cancel_maintenance(member)?;
            let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&member).unwrap().kind;
            citizen.active_plan = None;
            if citizen.active_action.is_some_and(|action| {
                matches!(
                    action.action,
                    crate::CitizenAction::Buy(_)
                        | crate::CitizenAction::BuyAt { .. }
                        | crate::CitizenAction::BuyFood(_)
                        | crate::CitizenAction::EquipClothing
                )
            }) {
                citizen.abandon_action()?;
            }
        }
        Ok(())
    }

    pub(crate) fn assess_household_living_standards(
        &mut self,
        now: u64,
    ) -> Result<Vec<AgentId>, SimulationError> {
        let mut changed = Vec::new();
        let ids: Vec<_> = self.households.keys().copied().collect();
        for id in ids {
            let state = &self.households[&id].living_standard;
            let history = &self.household_histories[&id];
            if !history
                .completed
                .back()
                .is_some_and(|day| day.end_ms == now)
                || state
                    .last_check
                    .as_ref()
                    .is_some_and(|check| check.time_ms == now)
            {
                continue;
            }
            let days: Vec<_> = history.completed.iter().rev().take(7).collect();
            let fresh = days.len() == 7
                && days.iter().all(|day| {
                    day.end_ms - day.start_ms == DAY_MS && day.start_ms >= state.since_ms
                });
            let income: i128 = days
                .iter()
                .map(|day| {
                    i128::from(day.sales_receipts)
                        + i128::from(day.inn_receipts)
                        + i128::from(day.construction.payments_received)
                        - i128::from(day.internal_construction_receipts)
                })
                .sum();
            let expenditure: i128 = days
                .iter()
                .map(|day| {
                    i128::from(day.purchases)
                        + i128::from(day.inn_payments)
                        + i128::from(day.taxes_paid)
                        + i128::from(day.workshop_fees_paid)
                })
                .sum();
            let surplus = income - expenditure;
            let result = if !fresh {
                LivingStandardResult::AwaitingFreshDays
            } else if surplus < 0 && state.tier > 1 {
                LivingStandardResult::Demoted
            } else if surplus > 0 && surplus * 10 >= expenditure && state.tier < 5 {
                if now < state.promotion_allowed_ms {
                    LivingStandardResult::PromotionCooldown
                } else {
                    LivingStandardResult::Promoted
                }
            } else {
                LivingStandardResult::Stayed
            };
            let state = &mut self.households.get_mut(&id).unwrap().living_standard;
            if result == LivingStandardResult::Promoted {
                state.change(state.tier + 1, now);
                changed.push(id);
            }
            if result == LivingStandardResult::Demoted {
                state.change(state.tier - 1, now);
                changed.push(id);
            }
            state.last_check = Some(LivingStandardCheck {
                time_ms: now,
                income,
                expenditure,
                result,
            });
        }
        for id in &changed {
            self.invalidate_household_living_plans(*id)?;
        }
        if !changed.is_empty() {
            self.refresh_living_tiers();
        }
        Ok(changed)
    }

    pub(crate) fn refresh_living_tiers(&mut self) {
        for (_, household) in self.households.iter_mut() {
            if household.living_standard.since_ms == u64::MAX {
                household.living_standard = LivingStandard::new(self.current_time_ms);
            }
        }
        for (_, agent) in self.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            if let Some(context) = &mut citizen.household {
                if let Some(household) = self.households.get(&context.id) {
                    context.living_tier = household.living_standard.tier;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diet::{DietCategory, category_bonus},
        marketplace::Good,
    };

    fn household_citizen() -> (Universe, AgentId, AgentId) {
        let (world, member) = Universe::default()
            .with_citizen("Ada", Citizen::new(100.0).unwrap())
            .unwrap();
        let household = world.economic_id(member).unwrap();
        (world, member, household)
    }

    #[test]
    fn each_tier_activates_only_its_needs_for_every_household_member() {
        let (world, member, household) = household_citizen();
        for tier in 1..=5 {
            let world = world.set_household_living_tier(household, tier).unwrap();
            let AgentKind::Citizen(source) = &world.agents[&member].kind;
            let mut citizen = source.clone();
            citizen.diet = [100.0; 4];
            citizen.alcohol = 100.0;
            citizen.garment_condition = None;
            assert_eq!(citizen.living_tier(), tier);
            assert_eq!(citizen.clothing_need() > 0.0, tier >= 2);
            assert_eq!(citizen.diet_category_active(DietCategory::Meat), tier >= 3);
            assert_eq!(
                citizen.diet_category_active(DietCategory::Sweets),
                tier >= 5
            );
            assert_eq!(
                citizen.alcohol_wellbeing(),
                if tier >= 4 {
                    category_bonus(100.0)
                } else {
                    0.0
                }
            );
            assert_eq!(
                citizen.dietary_wellbeing(),
                category_bonus(100.0)
                    * if tier == 5 {
                        4.0
                    } else if tier >= 3 {
                        3.0
                    } else {
                        0.0
                    }
            );
            assert_eq!(
                citizen.active_diet_intake(Good::Beer)[1],
                if tier >= 3 { 0.05 } else { 0.0 }
            );
        }
    }

    #[test]
    fn manual_changes_reset_fresh_days_and_downward_changes_extend_promotion_cooldown() {
        let (world, _, household) = household_citizen();
        let world = world.set_household_living_tier(household, 255).unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 5);
        let mut world = world;
        world.current_time_ms += 3 * DAY_MS;
        let world = world.set_household_living_tier(household, 2).unwrap();
        let state = &world.households[&household].living_standard;
        assert_eq!(state.since_ms, world.current_time_ms);
        assert_eq!(
            state.promotion_allowed_ms,
            world.current_time_ms + 14 * DAY_MS
        );
        assert!(state.next_check_ms(world.current_time_ms) > world.current_time_ms);
        assert_eq!(
            world
                .set_household_living_tier(household, 0)
                .unwrap()
                .households[&household]
                .living_standard
                .tier,
            1
        );
    }

    #[test]
    fn saves_round_trip_tier_and_alcohol_and_legacy_households_start_at_base() {
        let (world, member, household) = household_citizen();
        let mut world = world.set_household_living_tier(household, 4).unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&member).unwrap().kind;
        citizen.alcohol = 25.0;
        let saved = world.to_save_string().unwrap();
        let loaded = Universe::from_save_str(&saved).unwrap();
        assert_eq!(loaded.households[&household].living_standard.tier, 4);
        let AgentKind::Citizen(citizen) = &loaded.agents[&member].kind;
        assert_eq!(citizen.alcohol, 25.0);
        let start = saved.find("living_standard:").unwrap();
        let mut depth = 0;
        let mut end = start;
        for (offset, ch) in saved[start..].char_indices() {
            if ch == '(' {
                depth += 1;
            }
            if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    end = start + offset + 2;
                    break;
                }
            }
        }
        let legacy = format!("{}{}", &saved[..start], &saved[end..]);
        let loaded = Universe::from_save_str(&legacy).unwrap();
        assert_eq!(loaded.households[&household].living_standard.tier, 1);
        assert_eq!(
            loaded.households[&household].living_standard.since_ms,
            loaded.current_time_ms
        );
    }

    #[test]
    fn real_daily_boundary_closes_history_then_assesses_and_round_trips() {
        let (world, member, household) = household_citizen();
        let mut world = world.advance(crate::marketplace::UPDATE_TIME_MS).unwrap();
        world.households.get_mut(&household).unwrap().coins += 100;
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&member).unwrap().kind;
        citizen.coins += 100;
        world
            .household_histories
            .get_mut(&household)
            .unwrap()
            .current
            .sales_receipts = 100;
        world
            .refresh_household_history(world.current_time_ms, false)
            .unwrap();
        let world = world.advance(7 * DAY_MS).unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 2);
        assert_eq!(
            world.households[&household]
                .living_standard
                .last_check
                .as_ref()
                .unwrap()
                .result,
            LivingStandardResult::Promoted
        );
        Universe::from_save_str(&world.to_save_string().unwrap()).unwrap();
    }

    #[test]
    fn malformed_missing_household_fails_load_and_legacy_pending_purchases_are_cancelled() {
        let (mut world, member, household) = household_citizen();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&member).unwrap().kind;
        citizen.household.as_mut().unwrap().id = AgentId(uuid::Uuid::new_v4());
        assert!(Universe::from_save_str(&world.to_save_string().unwrap()).is_err());
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&member).unwrap().kind;
        citizen.household.as_mut().unwrap().id = household;
        world
            .households
            .get_mut(&household)
            .unwrap()
            .living_standard
            .since_ms = u64::MAX;
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&member).unwrap().kind;
        *citizen = citizen.start_planning().unwrap();
        assert!(citizen.active_plan.is_some());
        let loaded = Universe::from_save_str(&world.to_save_string().unwrap()).unwrap();
        let AgentKind::Citizen(citizen) = &loaded.agents[&member].kind;
        assert!(citizen.active_plan.is_none());
    }

    fn append_days(world: &mut Universe, household: AgentId, days: u64, income: i64, expense: i64) {
        for _ in 0..days {
            let start = world.current_time_ms;
            world.current_time_ms += DAY_MS;
            let history = world.household_histories.get_mut(&household).unwrap();
            let mut day = history.current.clone();
            day.start_ms = start;
            day.end_ms = world.current_time_ms;
            day.sales_receipts = income;
            day.purchases = expense;
            history.completed.push_back(day);
        }
    }

    #[test]
    fn cash_flow_requires_seven_fresh_days_and_positive_ten_percent_buffer() {
        let (mut world, _, household) = household_citizen();
        append_days(&mut world, household, 6, 110, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 1);
        append_days(&mut world, household, 1, 110, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 2);
        append_days(&mut world, household, 6, 1000, 0);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 2);
        append_days(&mut world, household, 1, 1000, 0);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 3);
    }

    #[test]
    fn demotion_blocks_promotion_for_fourteen_days_but_further_demotion_only_seven() {
        let (world, _, household) = household_citizen();
        let mut world = world.set_household_living_tier(household, 4).unwrap();
        append_days(&mut world, household, 7, 0, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 3);
        append_days(&mut world, household, 7, 200, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(
            world.households[&household]
                .living_standard
                .last_check
                .as_ref()
                .unwrap()
                .result,
            LivingStandardResult::PromotionCooldown
        );
        append_days(&mut world, household, 7, 200, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 4);
        append_days(&mut world, household, 7, 0, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        append_days(&mut world, household, 7, 0, 100);
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 2);
    }

    #[test]
    fn operating_cash_excludes_savings_and_client_capital_and_counts_carpenter_receipts() {
        let (mut world, _, household) = household_citizen();
        append_days(&mut world, household, 7, 0, 0);
        for day in world
            .household_histories
            .get_mut(&household)
            .unwrap()
            .completed
            .iter_mut()
        {
            day.construction.escrow_deposited = 100000;
            day.construction.escrow_refunded = 100000;
            day.introduced_coins = 100000;
        }
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 1);
        for day in world
            .household_histories
            .get_mut(&household)
            .unwrap()
            .completed
            .iter_mut()
        {
            day.construction.payments_received = 110;
            day.internal_construction_receipts = 110;
            day.purchases = 100;
        }
        world
            .households
            .get_mut(&household)
            .unwrap()
            .living_standard
            .last_check = None;
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 1);
        for day in world
            .household_histories
            .get_mut(&household)
            .unwrap()
            .completed
            .iter_mut()
        {
            day.internal_construction_receipts = 0;
        }
        world
            .households
            .get_mut(&household)
            .unwrap()
            .living_standard
            .last_check = None;
        world
            .assess_household_living_standards(world.current_time_ms)
            .unwrap();
        assert_eq!(world.households[&household].living_standard.tier, 2);
    }

    #[test]
    fn beer_does_not_preload_inactive_alcohol_and_demotion_cancels_pending_clothing() {
        let (world, member, household) = household_citizen();
        let mut world = world.set_household_living_tier(household, 3).unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&member).unwrap().kind;
        let mut action = citizen.consumption_action(crate::CitizenAction::Drink);
        action.drink_good = Good::Beer;
        action.drink_units = 400;
        action.duration_ms = crate::DRINK_DURATION_MS;
        action.remaining_ms = action.duration_ms;
        assert!(citizen.alcohol_rate(Some(action)) < 0.0);
        assert_eq!(citizen.active_diet_intake(Good::Beer)[1], 0.05);
        citizen.garment_condition = None;
        citizen.inventory.insert(Good::FlaxGarment, 1);
        *citizen = citizen
            .start_action(crate::CitizenAction::EquipClothing)
            .unwrap();
        let world = world.set_household_living_tier(household, 1).unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&member].kind;
        assert!(citizen.active_action.is_none());
        assert!(citizen.active_plan.is_none());
        assert_eq!(
            citizen
                .advance(crate::EQUIP_CLOTHING_DURATION_MS)
                .unwrap()
                .units(Good::FlaxGarment),
            1
        );
    }

    #[test]
    fn inactive_clothing_is_not_replaced_or_worn() {
        let (world, member, _) = household_citizen();
        let AgentKind::Citizen(citizen) = &world.agents[&member].kind;
        let citizen = citizen.with_garment_condition(Some(0.1)).unwrap();
        assert_eq!(citizen.spare_clothing_target(), 0);
        assert_eq!(
            citizen.advance(DAY_MS).unwrap().garment_condition(),
            Some(0.1)
        );
        assert!(!crate::planning::goals::need_eligible(
            &citizen,
            crate::planning::goals::Effect::ReduceClothingNeed
        ));
    }
}
