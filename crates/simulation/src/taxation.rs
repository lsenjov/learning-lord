use crate::{AgentId, Coins, Quantity, SimulationError, locations::PlaceId, marketplace::Good};
use imbl::{HashMap, OrdMap, Vector};

pub const TAX_HISTORY_LIMIT: usize = 4096;
const RATE_SCALE: u128 = 10_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaxRuleId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaxRate(u16);

impl TaxRate {
    pub fn new(basis_points: u16) -> Result<Self, SimulationError> {
        if basis_points > RATE_SCALE as u16 {
            return Err(SimulationError::InvalidTaxRate);
        }
        Ok(Self(basis_points))
    }

    pub fn basis_points(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaxKind {
    Socage { rates: HashMap<Good, TaxRate> },
    Asset { rates: HashMap<Good, TaxRate> },
    Income { rates: HashMap<Good, TaxRate> },
    FlatFee { payer: AgentId, coins: Coins },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaxRule {
    pub id: TaxRuleId,
    pub place: PlaceId,
    pub name: String,
    pub kind: TaxKind,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaxAmount {
    Goods {
        assessed: Quantity,
        collected: Quantity,
        outstanding: Quantity,
    },
    Coins {
        assessed: Coins,
        paid: Coins,
        arrears: Coins,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaxReceipt {
    pub time_ms: u64,
    pub rule: TaxRuleId,
    pub place: PlaceId,
    pub payer: AgentId,
    pub good: Option<Good>,
    pub amount: TaxAmount,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct AccountKey {
    pub rule: TaxRuleId,
    pub payer: AgentId,
    pub good: Option<Good>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TaxAccount {
    pub goods_due_scaled: u128,
    pub income_remainder: u128,
    pub asset_remainder: f64,
    pub arrears: Coins,
}

impl TaxAccount {
    pub fn assess_goods(
        &mut self,
        gross: Quantity,
        rate: TaxRate,
    ) -> Result<Quantity, SimulationError> {
        let amount = u128::from(gross) * u128::from(rate.0);
        let previous = self.goods_due_scaled / RATE_SCALE;
        self.goods_due_scaled = self
            .goods_due_scaled
            .checked_add(amount)
            .ok_or(SimulationError::InventoryOverflow)?;
        Quantity::try_from(self.goods_due_scaled / RATE_SCALE - previous)
            .map_err(|_| SimulationError::InventoryOverflow)
    }

    pub fn goods_owed(&self) -> Result<Quantity, SimulationError> {
        Quantity::try_from(self.goods_due_scaled / RATE_SCALE)
            .map_err(|_| SimulationError::InventoryOverflow)
    }

    pub fn collect_goods(&mut self, units: Quantity) {
        self.goods_due_scaled -= u128::from(units) * RATE_SCALE;
    }

    pub fn assess_income(&mut self, coins: Coins, rate: TaxRate) -> Result<Coins, SimulationError> {
        let scaled = u128::try_from(coins).map_err(|_| SimulationError::InvalidCoins)?
            * u128::from(rate.0)
            + self.income_remainder;
        let due =
            Coins::try_from(scaled / RATE_SCALE).map_err(|_| SimulationError::WealthOverflow)?;
        self.income_remainder = scaled % RATE_SCALE;
        Ok(due)
    }

    pub fn assess_asset(&mut self, value: f64, rate: TaxRate) -> Result<Coins, SimulationError> {
        let obligation = value * (f64::from(rate.0) / RATE_SCALE as f64) + self.asset_remainder;
        if !obligation.is_finite() || obligation < 0.0 || obligation >= 9_223_372_036_854_775_808.0
        {
            return Err(SimulationError::WealthOverflow);
        }
        let due = obligation.floor() as Coins;
        self.asset_remainder = obligation - obligation.floor();
        Ok(due)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Taxation {
    pub rules: OrdMap<TaxRuleId, TaxRule>,
    pub accounts: HashMap<AccountKey, TaxAccount>,
    pub history: Vector<TaxReceipt>,
    pub next_rule_id: u64,
}

impl Taxation {
    pub fn record(&mut self, receipt: TaxReceipt) {
        self.history.push_back(receipt);
        if self.history.len() > TAX_HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    pub fn arrears(&self, rule: TaxRuleId, payer: AgentId) -> Result<Coins, SimulationError> {
        self.accounts
            .iter()
            .filter(|(key, _)| key.rule == rule && key.payer == payer)
            .try_fold(0_i64, |total, (_, account)| {
                total.checked_add(account.arrears)
            })
            .ok_or(SimulationError::WealthOverflow)
    }
}

use crate::{
    AgentKind, Universe,
    storage::{GoodsOwner, StockKey},
};

impl Universe {
    pub fn tax_rules(&self) -> &OrdMap<TaxRuleId, TaxRule> {
        &self.taxation.rules
    }

    pub fn tax_history(&self) -> &Vector<TaxReceipt> {
        &self.taxation.history
    }

    pub fn tax_arrears(&self, rule: TaxRuleId, payer: AgentId) -> Coins {
        self.taxation
            .arrears(rule, payer)
            .expect("validated tax account total")
    }

    pub fn with_tax_rule(
        &self,
        place: PlaceId,
        name: impl Into<String>,
        kind: TaxKind,
    ) -> Result<(Self, TaxRuleId), SimulationError> {
        let id = TaxRuleId(self.taxation.next_rule_id);
        let mut universe = self.clone();
        universe.taxation.next_rule_id =
            id.0.checked_add(1).ok_or(SimulationError::TimeOverflow)?;
        universe.taxation.rules.insert(
            id,
            TaxRule {
                id,
                place,
                name: name.into(),
                kind,
                active: true,
            },
        );
        universe.validate_tax_rule(id)?;
        Ok((universe, id))
    }

    pub fn edit_tax_rule(
        &self,
        id: TaxRuleId,
        name: impl Into<String>,
        kind: TaxKind,
    ) -> Result<Self, SimulationError> {
        let mut universe = self.clone();
        let rule = universe
            .taxation
            .rules
            .get_mut(&id)
            .filter(|rule| rule.active)
            .ok_or(SimulationError::TaxRuleNotFound)?;
        rule.name = name.into();
        rule.kind = kind;
        universe.validate_tax_rule(id)?;
        Ok(universe)
    }

    pub fn without_tax_rule(&self, id: TaxRuleId) -> Result<Self, SimulationError> {
        let mut universe = self.clone();
        universe
            .taxation
            .rules
            .get_mut(&id)
            .filter(|rule| rule.active)
            .ok_or(SimulationError::TaxRuleNotFound)?
            .active = false;
        Ok(universe)
    }

    fn validate_tax_rule(&self, id: TaxRuleId) -> Result<(), SimulationError> {
        let rule = &self.taxation.rules[&id];
        let place = self.map.place(rule.place)?;
        if place.kind.is_public() || place.kind.is_town_owned() || rule.name.trim().is_empty() {
            return Err(SimulationError::InvalidTaxRule);
        }
        if let TaxKind::FlatFee { payer, coins } = rule.kind {
            if !self.agents.contains_key(&payer) {
                return Err(SimulationError::AgentNotFound);
            }
            if coins < 0 {
                return Err(SimulationError::InvalidCoins);
            }
        }
        for good in Good::ALL {
            let total: u64 = self
                .taxation
                .rules
                .values()
                .filter(|other| other.active && other.place == rule.place)
                .filter_map(|other| match &other.kind {
                    TaxKind::Socage { rates } => rates.get(&good),
                    _ => None,
                })
                .map(|rate| u64::from(rate.0))
                .try_fold(0_u64, u64::checked_add)
                .ok_or(SimulationError::InvalidTaxRule)?;
            if total > RATE_SCALE as u64 {
                return Err(SimulationError::InvalidTaxRule);
            }
        }
        Ok(())
    }

    fn collect_coin_account(
        &mut self,
        key: AccountKey,
        assessed: Coins,
        time_ms: u64,
    ) -> Result<bool, SimulationError> {
        let total = self.taxation.arrears(key.rule, key.payer)?;
        total
            .checked_add(assessed)
            .ok_or(SimulationError::WealthOverflow)?;
        let account = self.taxation.accounts.entry(key).or_default();
        let owed = account
            .arrears
            .checked_add(assessed)
            .ok_or(SimulationError::WealthOverflow)?;
        let AgentKind::Citizen(citizen) = &mut self
            .agents
            .get_mut(&key.payer)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        let paid = owed.min(citizen.coins.max(0));
        self.town_treasury = self
            .town_treasury
            .checked_add(paid)
            .ok_or(SimulationError::WealthOverflow)?;
        citizen.coins -= paid;
        account.arrears = owed - paid;
        let arrears = account.arrears;
        if assessed > 0 || owed > 0 {
            self.taxation.record(TaxReceipt {
                time_ms,
                rule: key.rule,
                place: self.taxation.rules[&key.rule].place,
                payer: key.payer,
                good: key.good,
                amount: TaxAmount::Coins {
                    assessed,
                    paid,
                    arrears,
                },
            });
        }
        Ok(paid > 0)
    }

    pub(crate) fn tax_production(
        &mut self,
        payer: AgentId,
        place: PlaceId,
        good: Good,
        gross: Quantity,
        time_ms: u64,
    ) -> Result<bool, SimulationError> {
        let rules: Vec<_> = self
            .taxation
            .rules
            .values()
            .filter(|rule| rule.place == place)
            .cloned()
            .collect();
        let mut remaining = gross;
        let mut changed = false;
        for rule in rules {
            let key = AccountKey {
                rule: rule.id,
                payer,
                good: Some(good),
            };
            let rate = if rule.active {
                match &rule.kind {
                    TaxKind::Socage { rates } => rates.get(&good).copied(),
                    _ => None,
                }
            } else {
                None
            };
            if rate.is_none() && !self.taxation.accounts.contains_key(&key) {
                continue;
            }
            let account = self.taxation.accounts.entry(key).or_default();
            let assessed = rate
                .map(|rate| account.assess_goods(gross, rate))
                .transpose()?
                .unwrap_or(0);
            let owed = account.goods_owed()?;
            let collected = owed.min(remaining);
            account.collect_goods(collected);
            let outstanding = account.goods_owed()?;
            remaining -= collected;
            if collected > 0 {
                let stored = self
                    .storage
                    .units(place, GoodsOwner::Town, good)
                    .checked_add(collected)
                    .ok_or(SimulationError::InventoryOverflow)?;
                self.storage.set_units(
                    StockKey {
                        place,
                        owner: GoodsOwner::Town,
                        good,
                    },
                    stored,
                )?;
                let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&payer).unwrap().kind;
                let retained = citizen
                    .units(good)
                    .checked_sub(collected)
                    .ok_or(SimulationError::InventoryOverflow)?;
                citizen.inventory.insert(good, retained);
                changed = true;
            }
            if rate.is_some() || owed > 0 {
                self.taxation.record(TaxReceipt {
                    time_ms,
                    rule: rule.id,
                    place,
                    payer,
                    good: Some(good),
                    amount: TaxAmount::Goods {
                        assessed,
                        collected,
                        outstanding,
                    },
                });
            }
        }
        Ok(changed)
    }

    pub(crate) fn tax_sales(
        &mut self,
        trades: &[crate::marketplace::Trade],
        time_ms: u64,
    ) -> Result<Vec<AgentId>, SimulationError> {
        let mut groups =
            std::collections::BTreeMap::<(uuid::Uuid, uuid::Uuid, usize), Coins>::new();
        for trade in trades {
            let revenue = groups
                .entry((trade.seller.0, trade.place.0, trade.good as usize))
                .or_default();
            *revenue = revenue
                .checked_add(trade.coins)
                .ok_or(SimulationError::WealthOverflow)?;
        }
        let rules: Vec<_> = self
            .taxation
            .rules
            .values()
            .filter(|rule| rule.active)
            .cloned()
            .collect();
        let mut changed = Vec::new();
        for ((seller, place, good), revenue) in groups {
            let payer = AgentId(seller);
            let good = Good::ALL[good];
            for rule in &rules {
                if rule.place.0 != place {
                    continue;
                }
                let TaxKind::Income { rates } = &rule.kind else {
                    continue;
                };
                let Some(&rate) = rates.get(&good) else {
                    continue;
                };
                let key = AccountKey {
                    rule: rule.id,
                    payer,
                    good: Some(good),
                };
                let assessed = self
                    .taxation
                    .accounts
                    .entry(key)
                    .or_default()
                    .assess_income(revenue, rate)?;
                if self.collect_coin_account(key, assessed, time_ms)? {
                    changed.push(payer);
                }
            }
        }
        Ok(changed)
    }

    pub(crate) fn tax_week(&mut self, time_ms: u64) -> Result<Vec<AgentId>, SimulationError> {
        let mut changed = Vec::new();
        let rules: Vec<_> = self
            .taxation
            .rules
            .values()
            .filter(|rule| rule.active)
            .cloned()
            .collect();
        let mut assessed_keys = std::collections::HashSet::new();
        for rule in rules {
            match rule.kind {
                TaxKind::FlatFee { payer, coins } => {
                    let key = AccountKey {
                        rule: rule.id,
                        payer,
                        good: None,
                    };
                    assessed_keys.insert(key);
                    if self.collect_coin_account(key, coins, time_ms)? {
                        changed.push(payer);
                    }
                }
                TaxKind::Asset { rates } => {
                    let mut stocks =
                        std::collections::BTreeMap::<(uuid::Uuid, usize), Quantity>::new();
                    for (key, units) in self.storage.stock() {
                        if key.place != rule.place {
                            continue;
                        }
                        let GoodsOwner::Agent(payer) = key.owner else {
                            continue;
                        };
                        let total = stocks.entry((payer.0, key.good as usize)).or_default();
                        *total = total
                            .checked_add(*units)
                            .ok_or(SimulationError::InventoryOverflow)?;
                    }
                    for order in self
                        .market
                        .orders()
                        .filter(|order| order.place == rule.place)
                    {
                        let total = stocks
                            .entry((order.seller.0, order.good as usize))
                            .or_default();
                        *total = total
                            .checked_add(order.units)
                            .ok_or(SimulationError::InventoryOverflow)?;
                    }
                    for ((payer, good), units) in stocks {
                        let good = Good::ALL[good];
                        let Some(&rate) = rates.get(&good) else {
                            continue;
                        };
                        let key = AccountKey {
                            rule: rule.id,
                            payer: AgentId(payer),
                            good: Some(good),
                        };
                        let value = self
                            .prices()
                            .value(good, units)
                            .ok_or(SimulationError::InvalidPrices)?;
                        let assessed = self
                            .taxation
                            .accounts
                            .entry(key)
                            .or_default()
                            .assess_asset(value, rate)?;
                        assessed_keys.insert(key);
                        if self.collect_coin_account(key, assessed, time_ms)? {
                            changed.push(key.payer);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut debts: Vec<_> = self
            .taxation
            .accounts
            .iter()
            .filter(|(_, account)| account.arrears > 0)
            .map(|(key, _)| *key)
            .collect();
        debts.sort_by_key(|key| (key.rule, key.payer.0, key.good.map(|good| good as usize)));
        for key in debts {
            if !assessed_keys.contains(&key) && self.collect_coin_account(key, 0, time_ms)? {
                changed.push(key.payer);
            }
        }
        Ok(changed)
    }
}
