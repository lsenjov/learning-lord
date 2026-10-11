use crate::{
    AgentId, Coins, Quantity, SimulationError, Universe,
    guilds::{FEE_HOUR_MS, WorkshopFeeAccount},
    locations::PlaceId,
    marketplace::Good,
    taxation::TaxAmount,
};
use imbl::{HashMap, Vector};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionContribution {
    pub production_ms: u64,
    pub produced: [Quantity; Good::COUNT],
}
impl Default for ProductionContribution {
    fn default() -> Self {
        Self {
            production_ms: 0,
            produced: [0; Good::COUNT],
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct DailyGuildHistory {
    pub start_ms: u64,
    pub end_ms: u64,
    pub opening_coins: Coins,
    pub closing_coins: Coins,
    pub payments_received: Coins,
    pub expenses_paid: Coins,
    /// Exact accrued amount; `unpaid` holds whole coins, regardless of settlement.
    pub fees_accrued: WorkshopFeeAccount,
    pub opening_outstanding: HashMap<AgentId, WorkshopFeeAccount>,
    pub closing_outstanding: HashMap<AgentId, WorkshopFeeAccount>,
    pub contributions: HashMap<(AgentId, PlaceId), ProductionContribution>,
}
impl DailyGuildHistory {
    fn new(now: u64, coins: Coins, outstanding: HashMap<AgentId, WorkshopFeeAccount>) -> Self {
        Self {
            start_ms: now,
            end_ms: now,
            opening_coins: coins,
            closing_coins: coins,
            payments_received: 0,
            expenses_paid: 0,
            fees_accrued: WorkshopFeeAccount::default(),
            opening_outstanding: outstanding.clone(),
            closing_outstanding: outstanding,
            contributions: HashMap::new(),
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct GuildHistory {
    pub current: DailyGuildHistory,
    pub completed: Vector<DailyGuildHistory>,
}
impl GuildHistory {
    pub(crate) fn new(now: u64, coins: Coins) -> Self {
        Self {
            current: DailyGuildHistory::new(now, coins, HashMap::new()),
            completed: Vector::new(),
        }
    }
}
fn cash(total: &mut Coins, amount: Coins) -> Result<(), SimulationError> {
    *total = total
        .checked_add(amount)
        .ok_or(SimulationError::WealthOverflow)?;
    Ok(())
}
fn amount(fee: &WorkshopFeeAccount) -> i128 {
    i128::from(fee.unpaid) * i128::from(FEE_HOUR_MS) + i128::from(fee.remainder)
}
fn total(fees: &HashMap<AgentId, WorkshopFeeAccount>) -> i128 {
    fees.values().map(amount).sum()
}
impl Universe {
    pub fn guild_history(&self, id: AgentId) -> Option<&GuildHistory> {
        self.guild_histories.get(&id)
    }
    pub(crate) fn record_guild_work(
        &mut self,
        member: AgentId,
        place: PlaceId,
        elapsed: u64,
        produced: [Quantity; Good::COUNT],
    ) -> Result<(), SimulationError> {
        let Some(guild) = self
            .guilds
            .values()
            .find(|guild| guild.workshops.contains(&place) && guild.members.contains(&member))
            .map(|guild| guild.id)
        else {
            return Ok(());
        };
        let rate = self.guilds[&guild].hourly_fee;
        let day = &mut self.guild_histories.get_mut(&guild).unwrap().current;
        day.fees_accrued.accrue(rate, elapsed)?;
        {
            let contribution = day.contributions.entry((member, place)).or_default();
            contribution.production_ms = contribution
                .production_ms
                .checked_add(elapsed)
                .ok_or(SimulationError::TimeOverflow)?;
            for (sum, units) in contribution.produced.iter_mut().zip(produced) {
                *sum = sum
                    .checked_add(units)
                    .ok_or(SimulationError::InventoryOverflow)?;
            }
        }
        Ok(())
    }
    pub(crate) fn record_guild_payment(
        &mut self,
        guild: AgentId,
        paid: Coins,
    ) -> Result<(), SimulationError> {
        if let Some(history) = self.guild_histories.get_mut(&guild) {
            cash(&mut history.current.payments_received, paid)?;
        }
        Ok(())
    }
    pub(crate) fn record_guild_taxes(&mut self, from: usize) -> Result<(), SimulationError> {
        for receipt in self.taxation.history.iter().skip(from) {
            if let Some(history) = self.guild_histories.get_mut(&receipt.payer)
                && let TaxAmount::Coins { paid, .. } = receipt.amount
            {
                cash(&mut history.current.expenses_paid, paid)?;
            }
        }
        Ok(())
    }
    pub(crate) fn refresh_guild_history(&mut self, now: u64, close: bool) {
        for (id, guild) in &self.guilds {
            let outstanding = self
                .workshop_fees
                .iter()
                .filter_map(|((payer, payee), fee)| {
                    (*payee == *id && *fee != WorkshopFeeAccount::default())
                        .then_some((*payer, *fee))
                })
                .collect();
            let history = self.guild_histories.get_mut(id).unwrap();
            history.current.end_ms = now;
            history.current.closing_coins = guild.coins;
            history.current.closing_outstanding = outstanding;
            if close
                && crate::production::work_period(now)
                    != crate::production::work_period(history.current.start_ms)
            {
                let next = DailyGuildHistory::new(
                    now,
                    guild.coins,
                    history.current.closing_outstanding.clone(),
                );
                history
                    .completed
                    .push_back(std::mem::replace(&mut history.current, next));
                if history.completed.len() > crate::household_history::COMPLETED_DAYS {
                    history.completed.pop_front();
                }
            }
        }
    }
    pub(crate) fn validate_guild_histories(&self) -> Result<(), crate::persistence::SaveError> {
        let invalid = || crate::persistence::SaveError::InvalidState("invalid guild history");
        if self.guild_histories.len() != self.guilds.len() {
            return Err(invalid());
        }
        for (id, guild) in &self.guilds {
            let history = self.guild_histories.get(id).ok_or_else(invalid)?;
            let outstanding: HashMap<_, _> = self
                .workshop_fees
                .iter()
                .filter_map(|((payer, payee), fee)| {
                    (*payee == *id && *fee != WorkshopFeeAccount::default())
                        .then_some((*payer, *fee))
                })
                .collect();
            if history.completed.len() > crate::household_history::COMPLETED_DAYS
                || history.current.end_ms != self.current_time_ms
                || history.current.closing_coins != guild.coins
                || history.current.closing_outstanding != outstanding
                || crate::production::work_period(history.current.start_ms)
                    != crate::production::work_period(self.current_time_ms)
            {
                return Err(invalid());
            }
            let mut previous: Option<&DailyGuildHistory> = None;
            for day in history
                .completed
                .iter()
                .chain(std::iter::once(&history.current))
            {
                if day.start_ms > day.end_ms
                    || day.end_ms > self.current_time_ms
                    || day.end_ms - day.start_ms > crate::marketplace::DAY_MS
                    || [
                        day.opening_coins,
                        day.closing_coins,
                        day.payments_received,
                        day.expenses_paid,
                        day.fees_accrued.unpaid,
                    ]
                    .iter()
                    .any(|coins| *coins < 0)
                    || day.fees_accrued.remainder >= FEE_HOUR_MS
                    || i128::from(day.opening_coins) + i128::from(day.payments_received)
                        - i128::from(day.expenses_paid)
                        != i128::from(day.closing_coins)
                    || total(&day.opening_outstanding) + amount(&day.fees_accrued)
                        - i128::from(day.payments_received) * i128::from(FEE_HOUR_MS)
                        != total(&day.closing_outstanding)
                {
                    return Err(invalid());
                }
                if let Some(old) = previous
                    && (old.end_ms != day.start_ms
                        || old.closing_coins != day.opening_coins
                        || old.closing_outstanding != day.opening_outstanding)
                {
                    return Err(invalid());
                }
                if day != &history.current
                    && (crate::production::work_period(day.start_ms) + 1
                        != crate::production::work_period(day.end_ms)
                        || crate::production::period_start(crate::production::work_period(
                            day.end_ms,
                        )) != day.end_ms)
                {
                    return Err(invalid());
                }
                for fees in [&day.opening_outstanding, &day.closing_outstanding] {
                    for (payer, fee) in fees {
                        if fee.unpaid < 0
                            || fee.remainder >= FEE_HOUR_MS
                            || *fee == WorkshopFeeAccount::default()
                            || (!self.agents.contains_key(payer)
                                && !self.households.contains_key(payer))
                            || !self
                                .account_workers(*payer)
                                .iter()
                                .any(|member| guild.members.contains(member))
                        {
                            return Err(invalid());
                        }
                    }
                }
                let mut member_ms: HashMap<AgentId, u128> = HashMap::new();
                let mut work_ms = 0u128;
                for ((member, place), c) in &day.contributions {
                    if !guild.members.contains(member) || !guild.workshops.contains(place) {
                        return Err(invalid());
                    }
                    let sum = member_ms.entry(*member).or_default();
                    *sum += u128::from(c.production_ms);
                    if *sum > u128::from(day.end_ms - day.start_ms) {
                        return Err(invalid());
                    }
                    work_ms += u128::from(c.production_ms);
                }
                if amount(&day.fees_accrued) != (work_ms * guild.hourly_fee as u128) as i128 {
                    return Err(invalid());
                }
                previous = Some(day);
            }
        }
        Ok(())
    }
}
