use imbl::HashMap;

use crate::{
    AgentId, Citizen, CitizenAction, Coins, Quantity, SimulationError,
    locations::PlaceId,
    marketplace::{Good, Market, OrderId},
    production::Recipe,
};

const OBSERVATION_MS: u64 = 48 * 3_600_000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrialPhase {
    Preparing,
    AwaitingOutput,
    Ready,
    Offered,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionTrial {
    pub worker: AgentId,
    pub recipe: Recipe,
    pub place: PlaceId,
    pub phase: TrialPhase,
    pub quantity: Quantity,
    pub remaining: Quantity,
    pub asking_price: f64,
    pub estimated_cost: f64,
    pub cost_quantity: Quantity,
    pub net_receipts: Coins,
    pub accepted: bool,
    pub order: Option<OrderId>,
    pub committed_at_ms: u64,
    pub observed_at_ms: u64,
    pub local_sales: Quantity,
    pub sold_units: Quantity,
    pub observed_local_sales: Quantity,
    pub committed_cost: f64,
    pub committed_viable_price: f64,
    pub committed_selling_price: f64,
    pub source: Option<CitizenAction>,
    pub stock_before: Quantity,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct TrialResult {
    pub trial: ProductionTrial,
    pub profitable: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct ProductionTrials {
    active: HashMap<(AgentId, Good), ProductionTrial>,
    results: HashMap<(AgentId, Good), TrialResult>,
}

impl Market {
    pub(crate) fn account_has_committed_production_trial(&self, account: AgentId) -> bool {
        self.production_trials
            .active
            .iter()
            .any(|((owner, _), trial)| {
                *owner == account
                    && matches!(
                        trial.phase,
                        TrialPhase::AwaitingOutput | TrialPhase::Offered
                    )
            })
    }

    pub(crate) fn cancel_unoffered_production_trials(&mut self, account: AgentId) {
        self.production_trials.active.retain(|(owner, _), trial| {
            *owner != account || !matches!(trial.phase, TrialPhase::Preparing | TrialPhase::Ready)
        });
    }

    pub fn production_trial(&self, account: AgentId, good: Good) -> Option<&ProductionTrial> {
        self.production_trials.active.get(&(account, good))
    }

    pub fn production_trial_result(&self, account: AgentId, good: Good) -> Option<&TrialResult> {
        self.production_trials.results.get(&(account, good))
    }

    pub(crate) fn replenishment_eligible(
        &self,
        account: AgentId,
        good: Good,
        viable_price: f64,
        selling_price: f64,
    ) -> bool {
        self.production_trials
            .results
            .get(&(account, good))
            .is_none_or(|result| {
                result.profitable
                    || viable_price + 1e-9 < result.trial.committed_viable_price
                    || selling_price > result.trial.committed_selling_price + 1e-9
            })
    }

    pub(crate) fn trial_eligible(
        &self,
        account: AgentId,
        good: Good,
        viable_price: f64,
        selling_price: f64,
    ) -> bool {
        !self.production_trials.active.contains_key(&(account, good))
            && self.replenishment_eligible(account, good, viable_price, selling_price)
    }

    pub(crate) fn record_trial_sale(
        &mut self,
        order: OrderId,
        units: Quantity,
        receipts: Coins,
        local: bool,
        _now_ms: u64,
    ) {
        let key = self
            .production_trials
            .active
            .iter()
            .find_map(|(key, trial)| (trial.order == Some(order)).then_some(*key));
        if let Some(trial) = key.and_then(|key| self.production_trials.active.get_mut(&key)) {
            trial.remaining = trial.remaining.saturating_sub(units);
            trial.sold_units = trial.sold_units.saturating_add(units);
            trial.net_receipts = trial.net_receipts.saturating_add(receipts);
            if local {
                trial.local_sales = trial.local_sales.saturating_add(units);
            }
        }
    }

    pub(crate) fn charge_trial_sale_tax(&mut self, order: OrderId, tax: Coins) {
        let key = self
            .production_trials
            .active
            .iter()
            .find_map(|(key, trial)| (trial.order == Some(order)).then_some(*key));
        if let Some(trial) = key.and_then(|key| self.production_trials.active.get_mut(&key)) {
            trial.net_receipts = trial.net_receipts.saturating_sub(tax);
        } else {
            let key = self
                .production_trials
                .results
                .iter()
                .find_map(|(key, result)| (result.trial.order == Some(order)).then_some(*key));
            if let Some(result) = key.and_then(|key| self.production_trials.results.get_mut(&key)) {
                result.trial.net_receipts = result.trial.net_receipts.saturating_sub(tax);
                result.profitable =
                    result.trial.net_receipts as f64 + 1e-9 >= result.trial.estimated_cost;
            }
        }
    }

    fn finish_trial(&mut self, key: (AgentId, Good)) {
        if let Some(mut trial) = self.production_trials.active.remove(&key) {
            trial.committed_selling_price = trial
                .committed_selling_price
                .max(self.known_trial_price(key.0, trial.place, key.1));
            if let Some(order) = trial.order {
                self.mark_trial_order(order, false);
            }
            if trial.sold_units == 0 {
                return;
            }
            let profitable = trial.net_receipts as f64 + 1e-9 >= trial.estimated_cost;
            self.production_trials
                .results
                .insert(key, TrialResult { trial, profitable });
        }
    }

    pub(crate) fn update_production_trials(&mut self, now_ms: u64) {
        let keys: Vec<_> = self.production_trials.active.keys().copied().collect();
        for key in keys {
            let trial = self.production_trials.active[&key].clone();
            if trial.phase != TrialPhase::Offered {
                continue;
            }
            let remaining = trial
                .order
                .and_then(|id| self.order(id))
                .map_or(0, |order| order.units)
                .min(trial.remaining);
            if remaining < trial.remaining {
                self.production_trials.active.remove(&key);
                if let Some(order) = trial.order {
                    self.mark_trial_order(order, false);
                }
                continue;
            }
            self.production_trials
                .active
                .get_mut(&key)
                .unwrap()
                .remaining = remaining;
            if now_ms.saturating_sub(trial.observed_at_ms) >= OBSERVATION_MS {
                let local = trial.local_sales.saturating_sub(trial.observed_local_sales);
                if local.saturating_mul(2) >= trial.quantity {
                    self.production_trials
                        .active
                        .get_mut(&key)
                        .unwrap()
                        .accepted = true;
                    // Acceptance belongs to the tested quote, not a partial fill.
                    self.establish_trial_price(key.0, trial.place, key.1, trial.asking_price);
                } else {
                    let known = self.known_trial_price(key.0, trial.place, key.1);
                    let price = (trial.asking_price + known) / 2.0;
                    if price < trial.asking_price {
                        if let Some(order) = trial.order {
                            let _ = self.set_order_price(order, price);
                        }
                        self.production_trials
                            .active
                            .get_mut(&key)
                            .unwrap()
                            .asking_price = price;
                    }
                }
                let current = self.production_trials.active.get_mut(&key).unwrap();
                current.observed_at_ms = now_ms;
                current.observed_local_sales = current.local_sales;
            }
            if remaining == 0
                && (trial.local_sales == 0
                    || now_ms.saturating_sub(trial.observed_at_ms) >= OBSERVATION_MS)
            {
                self.finish_trial(key);
            }
        }
    }
}

impl Citizen {
    fn minimum_viable_trial_price(
        &self,
        recipe: Recipe,
        cost: f64,
        site: Option<PlaceId>,
    ) -> Option<f64> {
        let (good, gross) = recipe.outputs()[0];
        let place = site
            .or_else(|| self.production_place(recipe).ok())
            .unwrap_or(self.selling_place());
        let socage = self
            .production_socage_rate_at(place, recipe, good)
            .basis_points();
        let units = gross
            .saturating_sub((u128::from(gross) * u128::from(socage)).div_ceil(10_000) as Quantity);
        let income = self
            .sale_income_basis_points
            .get(&(self.selling_place(), good))
            .copied()
            .unwrap_or(0);
        if units == 0 || income >= 10_000 {
            return None;
        }
        let price = cost * good.units_per_price_unit() as f64
            / (units as f64 * (1.0 - f64::from(income) / 10_000.0));
        price.is_finite().then_some(price)
    }

    pub(crate) fn eligible_sale_replenishment(
        &self,
        recipe: Recipe,
        site: Option<PlaceId>,
    ) -> bool {
        let good = recipe.outputs()[0].0;
        let Some(cost) = crate::production::estimated_batch_cost(recipe, self, site) else {
            return false;
        };
        self.market.replenishment_eligible(
            self.economic_id(),
            good,
            self.minimum_viable_trial_price(recipe, cost, site)
                .unwrap_or(f64::MAX),
            self.market
                .known_trial_price(self.economic_id(), self.selling_place(), good),
        )
    }

    pub(crate) fn eligible_production_trial(&self, recipe: Recipe, site: Option<PlaceId>) -> bool {
        let (good, _) = recipe.outputs()[0];
        if self
            .excess_goods()
            .is_ok_and(|excess| excess.units(good) > 0)
        {
            return false;
        }
        if let Some(trial) = self.market.production_trial(self.economic_id(), good) {
            return trial.worker == self.id && trial.phase == TrialPhase::Preparing;
        }
        let Some(cost) = crate::production::estimated_batch_cost(recipe, self, site) else {
            return false;
        };
        let Some(viable) = self.minimum_viable_trial_price(recipe, cost, site) else {
            return false;
        };
        let selling = self
            .market
            .known_trial_price(self.economic_id(), self.selling_place(), good);
        self.market
            .trial_eligible(self.economic_id(), good, viable, selling)
    }

    pub(crate) fn claim_production_trial(
        &mut self,
        recipe: Recipe,
        site: Option<PlaceId>,
    ) -> Result<(), SimulationError> {
        let (good, quantity) = recipe.outputs()[0];
        let key = (self.economic_id(), good);
        if let Some(trial) = self.market.production_trial(key.0, good) {
            let speculative = crate::production::recipe_profit_for_site(recipe, self, site)
                .is_none_or(|profit| profit <= 0.0);
            if speculative && (trial.worker != self.id || trial.phase != TrialPhase::Preparing) {
                return Err(SimulationError::MissingInputs);
            }
            return Ok(());
        }
        let existing = self.excess_goods()?.units(good).min(quantity);
        let Some(cost) =
            crate::production::estimated_batch_cost(recipe, self, site).or_else(|| {
                if existing == 0 {
                    return None;
                }
                let estimate = recipe
                    .inputs()
                    .iter()
                    .map(|&(input, units)| self.prices.value(input, units).unwrap_or(f64::NAN))
                    .sum::<f64>()
                    + self.workshop_fee_cost(
                        site.or_else(|| self.production_place(recipe).ok()),
                        recipe.duration_ms(self).ok()?,
                    );
                estimate.is_finite().then_some(estimate)
            })
        else {
            return Ok(());
        };
        let place = site
            .or_else(|| self.production_place(recipe).ok())
            .unwrap_or(self.selling_place());
        let known = self
            .market
            .known_trial_price(key.0, self.selling_place(), good);
        let viable = self
            .minimum_viable_trial_price(recipe, cost, site)
            .unwrap_or(f64::MAX);
        if !self.market.trial_eligible(key.0, good, viable, known) {
            return Ok(());
        }
        let rate = f64::from(self.market.ensure_seller_adjustment_rate(key.0)) / 100.0;
        let speculative = crate::production::recipe_profit_for_site(recipe, self, site)
            .is_none_or(|profit| profit <= 0.0);
        if existing == 0 && viable == f64::MAX {
            return Ok(());
        }
        let ask = if speculative && existing == 0 {
            known.max(viable)
        } else {
            known
        } * (1.0 + rate);
        if !ask.is_finite() {
            return Ok(());
        }
        let total_stock = self.economic_owned_units(good);
        let trial = ProductionTrial {
            worker: self.id,
            recipe,
            place,
            phase: if existing > 0 {
                TrialPhase::Ready
            } else {
                TrialPhase::Preparing
            },
            quantity,
            remaining: quantity,
            asking_price: if existing > 0 {
                known * (1.0 + rate)
            } else {
                ask
            },
            estimated_cost: cost,
            cost_quantity: quantity
                .saturating_sub(
                    (quantity as u128
                        * u128::from(
                            self.production_socage_rate_at(place, recipe, good)
                                .basis_points(),
                        ))
                    .div_ceil(10_000) as Quantity,
                )
                .max(1),
            net_receipts: 0,
            accepted: false,
            order: None,
            committed_at_ms: self.market_time_ms,
            observed_at_ms: self.market_time_ms,
            local_sales: 0,
            sold_units: 0,
            observed_local_sales: 0,
            committed_cost: cost,
            committed_viable_price: viable,
            committed_selling_price: known,
            source: None,
            stock_before: total_stock.saturating_sub(existing),
        };
        self.market.production_trials.active.insert(key, trial);
        Ok(())
    }

    pub(crate) fn start_trial_production(
        &mut self,
        action: CitizenAction,
    ) -> Result<(), SimulationError> {
        let recipe = match action {
            CitizenAction::Plant { crop, .. } => crop.recipe(),
            action => match action.production_recipe() {
                Some(recipe) => recipe,
                None => return Ok(()),
            },
        };
        let good = recipe.outputs()[0].0;
        let key = (self.economic_id(), good);
        if self.market.production_trial(key.0, good).is_none() {
            self.claim_production_trial(recipe, action.required_place(self))?;
        }
        let source = if recipe == Recipe::BrewBeer {
            action.required_place(self).and_then(|brewery| {
                self.map
                    .place(brewery)
                    .ok()
                    .and_then(|place| place.batches.iter().position(Option::is_none))
                    .map(|slot| CitizenAction::CollectBeer { brewery, slot })
            })
        } else {
            Some(action)
        };
        if let Some(trial) = self.market.production_trials.active.get_mut(&key)
            && trial.phase == TrialPhase::Preparing
        {
            if trial.worker != self.id {
                return Ok(());
            }
            trial.phase = TrialPhase::AwaitingOutput;
            trial.source = source;
        }
        Ok(())
    }

    pub(crate) fn complete_trial_output(&mut self, action: CitizenAction) {
        let account = self.economic_id();
        let owned: [Quantity; Good::COUNT] =
            std::array::from_fn(|index| self.economic_owned_units(Good::ALL[index]));
        let socage = self.site_socage_rates.clone();
        let keys: Vec<_> = self
            .market
            .production_trials
            .active
            .keys()
            .filter(|(owner, _)| *owner == account)
            .copied()
            .collect();
        for key in keys {
            let trial = self.market.production_trials.active.get_mut(&key).unwrap();
            if trial.phase != TrialPhase::AwaitingOutput {
                continue;
            }
            let completed = match (trial.source, action) {
                (
                    Some(CitizenAction::Plant {
                        field: a, plot: b, ..
                    }),
                    CitizenAction::Harvest { field, plot },
                ) => a == field && b == plot,
                (
                    Some(CitizenAction::CollectBeer {
                        brewery: a,
                        slot: b,
                    }),
                    CitizenAction::CollectBeer { brewery, slot },
                ) => a == brewery && b == slot,
                (Some(source), action) => {
                    source == action
                        && trial.worker == self.id
                        && trial.recipe != Recipe::BrewBeer
                        && !matches!(action, CitizenAction::Plant { .. })
                }
                _ => false,
            };
            if completed {
                let good = trial.recipe.outputs()[0].0;
                let gross = owned[good as usize]
                    .saturating_sub(trial.stock_before)
                    .min(trial.quantity);
                let basis_points = socage
                    .get(&(trial.place, good))
                    .map_or(0, |rate| rate.basis_points());
                trial.cost_quantity = gross
                    .saturating_sub(
                        (u128::from(gross) * u128::from(basis_points)).div_ceil(10_000) as Quantity,
                    )
                    .max(1);
                trial.phase = TrialPhase::Ready;
            }
        }
    }

    pub(crate) fn list_trial_stock(
        &mut self,
        good: Good,
        units: Quantity,
    ) -> Result<(), SimulationError> {
        let key = (self.economic_id(), good);
        let saleable = self.excess_goods()?.units(good);
        let recipe = self
            .available_production_recipes()
            .find(|recipe| recipe.outputs()[0].0 == good);
        if saleable > 0
            && self.market.production_trial(key.0, good).is_none()
            && let Some(recipe) = recipe
        {
            self.claim_production_trial(recipe, None)?;
        }
        let eligible = self
            .market
            .production_trials
            .active
            .get(&key)
            .filter(|trial| trial.phase == TrialPhase::Ready)
            .cloned();
        let trial_units = eligible.as_ref().map_or(0, |trial| {
            units
                .min(saleable)
                .min(trial.quantity)
                .min(trial.cost_quantity)
        });
        if let Some(trial) = eligible
            && trial_units > 0
        {
            let selling_place = self.selling_place();
            let order = self.market.list_at_quote(
                self.market_party(),
                self.selling_place(),
                good,
                trial_units,
                trial.asking_price,
            )?;
            self.market.mark_trial_order(order, true);
            let current = self.market.production_trials.active.get_mut(&key).unwrap();
            current.estimated_cost *= trial_units as f64 / current.cost_quantity as f64;
            current.quantity = trial_units;
            current.remaining = trial_units;
            current.order = Some(order);
            current.place = selling_place;
            current.phase = TrialPhase::Offered;
            current.observed_at_ms = self.market_time_ms;
        }
        if units > trial_units {
            self.market.list(
                self.market_party(),
                self.selling_place(),
                good,
                units - trial_units,
            )?;
        }
        self.consume_goods(good, units)?;
        Ok(())
    }
}

impl Citizen {
    pub(crate) fn cancel_preparing_trials(&mut self) {
        let worker = self.id;
        self.market
            .production_trials
            .active
            .retain(|_, trial| trial.worker != worker || trial.phase != TrialPhase::Preparing);
    }

    pub(crate) fn reconcile_production_trials(&mut self) {
        let account = self.economic_id();
        let keys: Vec<_> = self
            .market
            .production_trials
            .active
            .keys()
            .filter(|(owner, _)| *owner == account)
            .copied()
            .collect();
        for key in keys {
            let trial = self.market.production_trials.active[&key].clone();
            if trial.phase == TrialPhase::Ready
                && self.economic_owned_units(key.1) <= trial.stock_before
            {
                self.market.finish_trial(key);
            }
            if trial.phase == TrialPhase::AwaitingOutput {
                let gone = match trial.source {
                    Some(CitizenAction::Plant { field, plot, .. }) => {
                        self.crop(field, plot).is_ok_and(|crop| {
                            crop.is_none_or(|crop| {
                                crop.status(self.market_time_ms) == crate::farming::CropStatus::Dead
                            })
                        }) && !self
                            .active_action
                            .is_some_and(|active| active.action == trial.source.unwrap())
                    }
                    _ => false,
                };
                if gone {
                    self.market.finish_trial(key);
                }
            }
        }
        self.market.update_production_trials(self.market_time_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        StartingRole,
        locations::{Location, Position},
        marketplace::{MarketParty, Prices},
    };

    fn weaver() -> Citizen {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Weaver);
        let (map, _) = citizen
            .map()
            .with_place(
                Location::Weavery,
                Position::default(),
                Some(citizen.id()),
                "Weavery",
            )
            .unwrap();
        citizen
            .with_map(map)
            .unwrap()
            .with_good(Good::Flax, 200)
            .unwrap()
            .with_prices(
                Prices::default()
                    .with_price(Good::Flax, 1000.0)
                    .unwrap()
                    .with_price(Good::Thread, 1.0)
                    .unwrap(),
            )
    }

    #[test]
    fn unprofitable_recipe_is_a_bounded_candidate_and_cost_is_frozen() {
        let mut citizen = weaver();
        assert!(crate::production::recipe_profit(Recipe::SpinThread, &citizen).unwrap() < 0.0);
        assert!(crate::production::ranked_recipes_at(&citizen, None).contains(&Recipe::SpinThread));
        citizen
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        let trial = citizen
            .market
            .production_trial(citizen.economic_id(), Good::Thread)
            .unwrap()
            .clone();
        assert_eq!(trial.committed_cost, 200.0);
        assert_eq!(
            trial.asking_price,
            1000.0
                * (1.0
                    + f64::from(
                        citizen
                            .market
                            .seller_adjustment_rate(citizen.economic_id())
                            .unwrap()
                    ) / 100.0)
        );
        citizen.prices = citizen.prices.with_price(Good::Flax, 1.0).unwrap();
        assert_eq!(
            citizen
                .market
                .production_trial(citizen.economic_id(), Good::Thread)
                .unwrap()
                .estimated_cost,
            200.0
        );
        citizen
            .start_trial_production(CitizenAction::Produce(Recipe::SpinThread))
            .unwrap();
        assert!(!citizen.eligible_production_trial(Recipe::SpinThread, None));
    }

    #[test]
    fn shared_worker_claim_is_atomic_and_cancelled_preparation_releases_it() {
        let mut first = weaver();
        let mut second = weaver();
        let account = AgentId(uuid::Uuid::new_v4());
        let members = vec![first.id(), second.id()];
        for citizen in [&mut first, &mut second] {
            citizen.household = Some(crate::households::HouseholdContext {
                id: account,
                members: members.clone(),
                allegiance: 1.0,
                other_carried: [0; Good::COUNT],
                other_consumption_value: 0.0,
            });
        }
        first
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        second.market = first.market.clone();
        assert_eq!(
            second.claim_production_trial(Recipe::SpinThread, None),
            Err(SimulationError::MissingInputs)
        );
        first.cancel_preparing_trials();
        second.market = first.market.clone();
        second
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        assert_eq!(
            second
                .market
                .production_trial(account, Good::Thread)
                .unwrap()
                .worker,
            second.id()
        );
    }

    fn offered() -> (Market, AgentId, PlaceId, OrderId) {
        let mut citizen = weaver().with_good(Good::Thread, 200).unwrap();
        citizen.market.prices = citizen
            .market
            .prices
            .with_price(Good::Thread, 100.0)
            .unwrap();
        citizen.prices = citizen.market.prices;
        citizen
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        citizen.list_trial_stock(Good::Thread, 200).unwrap();
        let account = citizen.economic_id();
        let trial = citizen
            .market
            .production_trials
            .active
            .get_mut(&(account, Good::Thread))
            .unwrap();
        trial.asking_price = 200.0;
        trial.estimated_cost = 20.0;
        trial.committed_cost = 20.0;
        trial.committed_viable_price = 100.0;
        trial.committed_selling_price = 100.0;
        let order = trial.order.unwrap();
        let place = trial.place;
        citizen.market.set_order_price(order, 200.0).unwrap();
        (citizen.market, account, place, order)
    }

    #[test]
    fn fixed_trial_ignores_restocking_and_partial_acceptance_waits_48_hours() {
        let (mut market, account, place, order) = offered();
        market.list(account, place, Good::Thread, 1000).unwrap();
        let ordinary = market
            .orders()
            .find(|listed| listed.id != order)
            .unwrap()
            .id;
        market.set_order_price(ordinary, 300.0).unwrap();
        market
            .purchase(
                AgentId(uuid::Uuid::new_v4()),
                place,
                (Good::Thread, 100),
                1000,
                0,
                true,
            )
            .unwrap();
        assert_eq!(
            market
                .production_trial(account, Good::Thread)
                .unwrap()
                .quantity,
            200
        );
        market.update_production_trials(OBSERVATION_MS - 1);
        assert!(
            !market
                .production_trial(account, Good::Thread)
                .unwrap()
                .accepted
        );
        market.update_production_trials(OBSERVATION_MS);
        assert!(
            market
                .production_trial(account, Good::Thread)
                .unwrap()
                .accepted
        );
        assert_eq!(
            market.known_trial_price(account, place, Good::Thread),
            200.0
        );
        assert!(!market.trial_eligible(account, Good::Thread, 100.0, 200.0));
    }

    #[test]
    fn weak_trial_moves_halfway_and_full_sale_waits_for_price_acceptance() {
        let (mut market, account, place, order) = offered();
        market.update_production_trials(OBSERVATION_MS);
        assert_eq!(market.order(order).unwrap().quoted_price, 150.0);
        assert_eq!(
            market
                .production_trial(account, Good::Thread)
                .unwrap()
                .asking_price,
            150.0
        );
        market
            .purchase(
                AgentId(uuid::Uuid::new_v4()),
                place,
                (Good::Thread, 200),
                1000,
                OBSERVATION_MS + 1,
                true,
            )
            .unwrap();
        market.update_production_trials(OBSERVATION_MS + 1);
        assert!(market.production_trial(account, Good::Thread).is_some());
        market.update_production_trials(2 * OBSERVATION_MS);
        assert!(market.production_trial(account, Good::Thread).is_none());
        assert_eq!(
            market.known_trial_price(account, place, Good::Thread),
            150.0
        );
        assert!(
            market
                .production_trial_result(account, Good::Thread)
                .unwrap()
                .profitable
        );
    }

    #[test]
    fn actual_net_loss_blocks_same_conditions_and_improvement_unlocks() {
        let (mut market, account, place, order) = offered();
        market
            .purchase(
                MarketParty::Citizen(AgentId(uuid::Uuid::new_v4())),
                place,
                (Good::Thread, 200),
                1000,
                0,
                true,
            )
            .unwrap();
        market.charge_trial_sale_tax(order, 30);
        market.update_production_trials(OBSERVATION_MS);
        let result = market
            .production_trial_result(account, Good::Thread)
            .unwrap();
        assert_eq!(result.trial.net_receipts, 10);
        assert!(!result.profitable);
        assert!(!market.trial_eligible(account, Good::Thread, 100.0, 200.0));
        assert!(market.trial_eligible(account, Good::Thread, 95.0, 200.0));
        assert!(market.trial_eligible(account, Good::Thread, 100.0, 201.0));
    }
    #[test]
    fn nonsale_withdrawal_after_partial_sale_does_not_record_a_failed_experiment() {
        let (mut market, account, place, _) = offered();
        market
            .purchase(
                AgentId(uuid::Uuid::new_v4()),
                place,
                (Good::Thread, 50),
                1000,
                0,
                true,
            )
            .unwrap();
        market.withdraw(account, place, Good::Thread, 150).unwrap();
        market.update_production_trials(1);
        assert!(market.production_trial(account, Good::Thread).is_none());
        assert!(
            market
                .production_trial_result(account, Good::Thread)
                .is_none()
        );
        assert!(market.trial_eligible(account, Good::Thread, 100.0, 100.0));
    }
}

impl Market {
    pub(crate) fn validate_production_trial_state(
        &self,
    ) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        for ((account, good), trial) in &self.production_trials.active {
            if trial.recipe.outputs()[0].0 != *good
                || trial.quantity == 0
                || trial.cost_quantity == 0
                || trial.remaining > trial.quantity
                || trial.sold_units > trial.quantity
                || trial.local_sales > trial.sold_units
                || trial.observed_local_sales > trial.local_sales
                || trial.observed_at_ms < trial.committed_at_ms
                || !trial.asking_price.is_finite()
                || trial.asking_price <= 0.0
                || !trial.estimated_cost.is_finite()
                || trial.estimated_cost < 0.0
                || !trial.committed_viable_price.is_finite()
                || trial.committed_viable_price < 0.0
                || !trial.committed_cost.is_finite()
                || trial.committed_cost < 0.0
                || !trial.committed_selling_price.is_finite()
                || trial.committed_selling_price <= 0.0
                || self.seller_adjustment_rate(*account).is_none()
                || (trial.phase == TrialPhase::Offered) != trial.order.is_some()
                || trial.phase == TrialPhase::AwaitingOutput && trial.source.is_none()
            {
                return Err(SaveError::InvalidState("invalid production trial"));
            }
            if let Some(order) = trial.order.and_then(|id| self.order(id))
                && (order.seller.account() != Some(*account)
                    || order.good != *good
                    || order.place != trial.place
                    || order.units != trial.remaining
                    || !self.order_is_trial(order.id))
            {
                return Err(SaveError::InvalidState("production trial listing mismatch"));
            }
        }
        for ((_, good), result) in &self.production_trials.results {
            let trial = &result.trial;
            if trial.recipe.outputs()[0].0 != *good
                || !trial.estimated_cost.is_finite()
                || trial.estimated_cost < 0.0
                || result.profitable != (trial.net_receipts as f64 + 1e-9 >= trial.estimated_cost)
            {
                return Err(SaveError::InvalidState("invalid production trial result"));
            }
        }
        Ok(())
    }
}

impl crate::Universe {
    pub(crate) fn validate_production_trials(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        self.market.validate_production_trial_state()?;
        for ((account, _), trial) in &self.market.production_trials.active {
            let Some(agent) = self.agents.get(&trial.worker) else {
                return Err(SaveError::InvalidState("unknown trial worker"));
            };
            let crate::AgentKind::Citizen(citizen) = &agent.kind;
            if citizen.economic_id() != *account
                || self.map.place(trial.place).is_err()
                || trial.committed_at_ms > self.current_time_ms
                || trial.observed_at_ms > self.current_time_ms
            {
                return Err(SaveError::InvalidState(
                    "production trial account or time mismatch",
                ));
            }
            if trial.phase == TrialPhase::AwaitingOutput {
                let present = match trial.source {
                    Some(CitizenAction::Plant { field, plot, .. }) => {
                        self.map
                            .place(field)
                            .is_ok_and(|place| place.plots.get(plot).is_some_and(Option::is_some))
                            || citizen
                                .active_action
                                .is_some_and(|active| Some(active.action) == trial.source)
                    }
                    Some(CitizenAction::CollectBeer { brewery, slot }) => self
                        .map
                        .place(brewery)
                        .is_ok_and(|place| place.batches.get(slot).is_some_and(Option::is_some)),
                    Some(source) => citizen
                        .active_action
                        .is_some_and(|active| active.action == source),
                    None => false,
                };
                if !present {
                    return Err(SaveError::InvalidState("orphaned production trial"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod depletion_tests {
    use super::*;
    use crate::{
        StartingRole,
        locations::{Location, Position},
        marketplace::Prices,
    };

    fn weaver() -> Citizen {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Weaver);
        let (map, _) = citizen
            .map()
            .with_place(
                Location::Weavery,
                Position::default(),
                Some(citizen.id()),
                "Weavery",
            )
            .unwrap();
        citizen
            .with_map(map)
            .unwrap()
            .with_good(Good::Flax, 200)
            .unwrap()
            .with_prices(
                Prices::default()
                    .with_price(Good::Flax, 1000.0)
                    .unwrap()
                    .with_price(Good::Thread, 1.0)
                    .unwrap(),
            )
    }

    #[test]
    fn existing_output_is_tested_before_speculative_manufacturing() {
        let mut citizen = weaver().with_good(Good::Thread, 200).unwrap();
        assert!(!citizen.eligible_production_trial(Recipe::SpinThread, None));
        assert!(
            !crate::production::ranked_recipes_at(&citizen, None).contains(&Recipe::SpinThread)
        );
        citizen.list_trial_stock(Good::Thread, 200).unwrap();
        assert_eq!(
            citizen
                .market
                .production_trial(citizen.economic_id(), Good::Thread)
                .unwrap()
                .phase,
            TrialPhase::Offered
        );
    }

    #[test]
    fn input_consumption_releases_unoffered_trial_without_poisoning_retry() {
        let mut citizen = weaver();
        citizen
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        citizen
            .start_trial_production(CitizenAction::Produce(Recipe::SpinThread))
            .unwrap();
        citizen.set_inventory_units(Good::Thread, 200);
        citizen.complete_trial_output(CitizenAction::Produce(Recipe::SpinThread));
        citizen.consume_goods(Good::Thread, 200).unwrap();
        citizen.reconcile_production_trials();
        assert!(
            citizen
                .market
                .production_trial(citizen.economic_id(), Good::Thread)
                .is_none()
        );
        assert!(
            citizen
                .market
                .production_trial_result(citizen.economic_id(), Good::Thread)
                .is_none()
        );
        assert!(citizen.eligible_production_trial(Recipe::SpinThread, None));
    }

    #[test]
    fn complete_income_or_output_tax_excludes_speculative_candidates() {
        let mut citizen = weaver();
        citizen
            .sale_income_basis_points
            .insert((citizen.selling_place(), Good::Thread), 10_000);
        assert!(!citizen.eligible_production_trial(Recipe::SpinThread, None));
        citizen
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        assert!(
            citizen
                .market
                .production_trial(citizen.economic_id(), Good::Thread)
                .is_none()
        );
        citizen.sale_income_basis_points.clear();
        let place = citizen.production_place(Recipe::SpinThread).unwrap();
        citizen.site_socage_rates.insert(
            (place, Good::Thread),
            crate::taxation::TaxRate::new(10_000).unwrap(),
        );
        assert!(!citizen.eligible_production_trial(Recipe::SpinThread, Some(place)));
        citizen
            .claim_production_trial(Recipe::SpinThread, Some(place))
            .unwrap();
        assert!(
            citizen
                .market
                .production_trial(citizen.economic_id(), Good::Thread)
                .is_none()
        );
    }

    #[test]
    fn profitable_second_worker_does_not_steal_preparing_trial() {
        let mut owner = weaver();
        owner
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        let account = owner.economic_id();
        let mut worker = owner.clone();
        worker.id = AgentId(uuid::Uuid::new_v4());
        worker.household = Some(crate::households::HouseholdContext {
            id: account,
            members: vec![owner.id(), worker.id()],
            allegiance: 1.0,
            other_carried: [0; Good::COUNT],
            other_consumption_value: 0.0,
        });
        worker
            .start_trial_production(CitizenAction::Produce(Recipe::SpinThread))
            .unwrap();
        assert_eq!(
            worker
                .market
                .production_trial(account, Good::Thread)
                .unwrap()
                .worker,
            owner.id()
        );
        assert_eq!(
            worker
                .market
                .production_trial(account, Good::Thread)
                .unwrap()
                .phase,
            TrialPhase::Preparing
        );
    }
    #[test]
    fn post_socage_output_carries_full_batch_cost() {
        let mut citizen = weaver().with_good(Good::Thread, 100).unwrap();
        let place = citizen.production_place(Recipe::SpinThread).unwrap();
        citizen.site_socage_rates.insert(
            (place, Good::Thread),
            crate::taxation::TaxRate::new(5000).unwrap(),
        );
        citizen
            .claim_production_trial(Recipe::SpinThread, Some(place))
            .unwrap();
        citizen.list_trial_stock(Good::Thread, 100).unwrap();
        let account = citizen.economic_id();
        let trial = citizen
            .market
            .production_trial(account, Good::Thread)
            .unwrap();
        assert_eq!(trial.estimated_cost, 200.0);
        let order = trial.order.unwrap();
        let place = trial.place;
        citizen.market.set_order_price(order, 1500.0).unwrap();
        citizen
            .market
            .purchase(
                AgentId(uuid::Uuid::new_v4()),
                place,
                (Good::Thread, 100),
                1000,
                0,
                true,
            )
            .unwrap();
        citizen.market.update_production_trials(OBSERVATION_MS);
        assert!(
            !citizen
                .market
                .production_trial_result(account, Good::Thread)
                .unwrap()
                .profitable
        );
        assert!(
            !citizen
                .market
                .trial_eligible(account, Good::Thread, 2000.0, 1.0)
        );
    }

    #[test]
    fn brewing_claim_survives_delay_and_save_then_matches_exact_slot() {
        let mut citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Brewer);
        for &(good, units) in Recipe::BrewBeer.inputs() {
            citizen = citizen.with_good(good, units * 2).unwrap();
        }
        let (mut universe, id) = crate::Universe::with_map(crate::locations::Map::default())
            .with_citizen("Brewer", citizen)
            .unwrap();
        let (world, companion) = universe
            .with_citizen("Companion", Citizen::new(0.0).unwrap())
            .unwrap();
        universe = world;
        let (world, brewery) = universe.with_property(id, Location::Brewery).unwrap();
        universe = world;
        universe
            .map
            .set_batch(
                brewery,
                0,
                Some(crate::brewing::Batch::Fermenting {
                    ready_at_ms: crate::brewing::FERMENTATION_MS,
                }),
            )
            .unwrap();
        universe.refresh_market();
        let action = CitizenAction::ProduceAt {
            place: brewery,
            recipe: Recipe::BrewBeer,
        };
        let started = universe.start_action(id, action).unwrap();
        assert_eq!(
            started
                .market
                .production_trial(id, Good::Beer)
                .unwrap()
                .source,
            Some(CitizenAction::CollectBeer { brewery, slot: 1 })
        );
        let done = started.advance(crate::brewing::PREPARATION_MS).unwrap();
        let loaded = crate::Universe::from_save_str(&done.to_save_string().unwrap()).unwrap();
        assert_eq!(loaded, done);
        let crate::AgentKind::Citizen(brewer) = &loaded.agents[&id].kind;
        assert_eq!(
            loaded.with_household("Family", brewer.home(), &[id, companion]),
            Err(SimulationError::InvalidOwnership)
        );
        let mut standalone = match &loaded.agents[&id].kind {
            crate::AgentKind::Citizen(citizen) => citizen.clone(),
        };
        standalone.complete_trial_output(CitizenAction::CollectBeer { brewery, slot: 0 });
        assert_eq!(
            standalone
                .market
                .production_trial(id, Good::Beer)
                .unwrap()
                .phase,
            TrialPhase::AwaitingOutput
        );
        standalone.complete_trial_output(CitizenAction::CollectBeer { brewery, slot: 1 });
        assert_eq!(
            standalone
                .market
                .production_trial(id, Good::Beer)
                .unwrap()
                .phase,
            TrialPhase::Ready
        );
    }
    #[test]
    fn lower_income_tax_unlocks_a_failed_trial_without_higher_sale_quote() {
        let mut citizen = weaver().with_good(Good::Thread, 200).unwrap();
        let account = citizen.economic_id();
        let selling_place = citizen.selling_place();
        citizen
            .sale_income_basis_points
            .insert((selling_place, Good::Thread), 5000);
        citizen
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        citizen.list_trial_stock(Good::Thread, 200).unwrap();
        let trial = citizen
            .market
            .production_trials
            .active
            .get_mut(&(account, Good::Thread))
            .unwrap();
        assert_eq!(trial.committed_viable_price, 2000.0);
        trial.asking_price = 1500.0;
        let order = trial.order.unwrap();
        citizen.market.set_order_price(order, 1500.0).unwrap();
        citizen
            .market
            .purchase(
                AgentId(uuid::Uuid::new_v4()),
                selling_place,
                (Good::Thread, 200),
                1000,
                0,
                true,
            )
            .unwrap();
        citizen.market.charge_trial_sale_tax(order, 150);
        citizen.market.update_production_trials(OBSERVATION_MS);
        let result = citizen
            .market
            .production_trial_result(account, Good::Thread)
            .unwrap();
        assert!(!result.profitable);
        assert_eq!(result.trial.net_receipts, 150);
        assert_eq!(
            citizen
                .market
                .known_trial_price(account, selling_place, Good::Thread),
            1500.0
        );
        assert!(!citizen.eligible_production_trial(Recipe::SpinThread, None));
        citizen
            .sale_income_basis_points
            .insert((selling_place, Good::Thread), 0);
        assert!(citizen.eligible_production_trial(Recipe::SpinThread, None));
        assert!(citizen.eligible_sale_replenishment(Recipe::SpinThread, None));
    }

    #[test]
    fn another_household_workers_matching_output_does_not_complete_claim() {
        let mut owner = weaver();
        let action = CitizenAction::Produce(Recipe::SpinThread);
        owner
            .claim_production_trial(Recipe::SpinThread, None)
            .unwrap();
        owner.start_trial_production(action).unwrap();
        let account = owner.economic_id();
        let mut worker = owner.clone();
        worker.id = AgentId(uuid::Uuid::new_v4());
        worker.household = Some(crate::households::HouseholdContext {
            id: account,
            members: vec![owner.id(), worker.id()],
            allegiance: 1.0,
            other_carried: [0; Good::COUNT],
            other_consumption_value: 0.0,
        });
        worker.set_inventory_units(Good::Thread, 200);
        worker.complete_trial_output(action);
        assert_eq!(
            worker
                .market
                .production_trial(account, Good::Thread)
                .unwrap()
                .phase,
            TrialPhase::AwaitingOutput
        );
        owner.market = worker.market.clone();
        owner.set_inventory_units(Good::Thread, 200);
        owner.complete_trial_output(action);
        assert_eq!(
            owner
                .market
                .production_trial(account, Good::Thread)
                .unwrap()
                .phase,
            TrialPhase::Ready
        );
    }
}
