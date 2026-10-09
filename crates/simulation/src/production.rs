use crate::{Citizen, Coins, Quantity, SimulationError, locations::Location, marketplace::Good};

/// Practice is capped after 52 weeks of twelve-hour working days.
pub const MAX_SKILL_PRACTICE_MS: u64 = 4_368 * 3_600_000;

pub fn skill_duration_reduction(practice_ms: u64) -> f64 {
    let progress = practice_ms.min(MAX_SKILL_PRACTICE_MS) as f64 / MAX_SKILL_PRACTICE_MS as f64;
    0.5 * (2.0 * progress - progress * progress)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Skill {
    Farming,
    Milling,
    Woodcutting,
    Baking,
    Weaving,
    Tailoring,
    Fishing,
}

impl Skill {
    pub const COUNT: usize = 7;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Farming,
        Self::Milling,
        Self::Woodcutting,
        Self::Baking,
        Self::Weaving,
        Self::Tailoring,
        Self::Fishing,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Farming => "Farming",
            Self::Milling => "Milling",
            Self::Woodcutting => "Woodcutting",
            Self::Baking => "Baking",
            Self::Weaving => "Weaving",
            Self::Tailoring => "Tailoring",
            Self::Fishing => "Fishing",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Recipe {
    Forage,
    GrowWheat,
    MillFlour,
    ChopWood,
    FetchWater,
    BakeRolls,
    BakeBerryPie,
    GrowFlax,
    SpinThread,
    WeaveCloth,
    MakeClothingBlock,
    AssembleGarment,
    CatchFish,
    GrowCabbage,
}

impl Recipe {
    pub const COUNT: usize = 14;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Forage,
        Self::GrowWheat,
        Self::MillFlour,
        Self::ChopWood,
        Self::FetchWater,
        Self::BakeRolls,
        Self::BakeBerryPie,
        Self::GrowFlax,
        Self::SpinThread,
        Self::WeaveCloth,
        Self::MakeClothingBlock,
        Self::AssembleGarment,
        Self::CatchFish,
        Self::GrowCabbage,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Forage => "Forage",
            Self::GrowWheat => "Grow wheat",
            Self::MillFlour => "Mill flour",
            Self::ChopWood => "Chop wood",
            Self::FetchWater => "Fetch water",
            Self::BakeRolls => "Bake rolls",
            Self::BakeBerryPie => "Bake berry pie",
            Self::GrowFlax => "Grow flax",
            Self::SpinThread => "Spin thread",
            Self::WeaveCloth => "Weave cloth",
            Self::MakeClothingBlock => "Make clothing block",
            Self::AssembleGarment => "Assemble garment",
            Self::CatchFish => "Catch fish",
            Self::GrowCabbage => "Grow cabbage",
        }
    }
    pub fn skill(self) -> Option<Skill> {
        match self {
            Self::GrowWheat | Self::GrowFlax | Self::GrowCabbage => Some(Skill::Farming),
            Self::MillFlour => Some(Skill::Milling),
            Self::ChopWood => Some(Skill::Woodcutting),
            Self::FetchWater | Self::Forage => None,
            Self::CatchFish => Some(Skill::Fishing),
            Self::BakeRolls | Self::BakeBerryPie => Some(Skill::Baking),
            Self::SpinThread | Self::WeaveCloth => Some(Skill::Weaving),
            Self::MakeClothingBlock | Self::AssembleGarment => Some(Skill::Tailoring),
        }
    }
    pub fn location(self) -> Location {
        match self {
            Self::GrowWheat | Self::GrowFlax | Self::GrowCabbage => Location::Field,
            Self::MillFlour => Location::Mill,
            Self::ChopWood | Self::Forage => Location::Forest,
            Self::FetchWater | Self::CatchFish => Location::River,
            Self::BakeRolls | Self::BakeBerryPie => Location::Bakery,
            Self::SpinThread | Self::WeaveCloth => Location::Weavery,
            Self::MakeClothingBlock | Self::AssembleGarment => Location::Tailory,
        }
    }
    pub fn inputs(self) -> &'static [(Good, Quantity)] {
        match self {
            Self::GrowWheat
            | Self::GrowFlax
            | Self::GrowCabbage
            | Self::ChopWood
            | Self::FetchWater
            | Self::Forage
            | Self::CatchFish => &[],
            Self::MillFlour => &[(Good::Wheat, 300)],
            Self::SpinThread => &[(Good::Flax, 200)],
            Self::WeaveCloth => &[(Good::Thread, 200)],
            Self::MakeClothingBlock => &[(Good::Cloth, 25)],
            Self::AssembleGarment => &[(Good::FlaxBlock, 8)],
            Self::BakeRolls => &[(Good::Flour, 100), (Good::Wood, 25), (Good::Water, 100)],
            Self::BakeBerryPie => &[
                (Good::Flour, 100),
                (Good::Wood, 25),
                (Good::Water, 50),
                (Good::Berries, 100),
            ],
        }
    }
    pub fn outputs(self) -> &'static [(Good, Quantity)] {
        match self {
            Self::Forage => &[(Good::Berries, crate::FORAGE_AVERAGE_GRAMS)],
            Self::GrowWheat => &[(Good::Wheat, 600)],
            Self::GrowFlax => &[(Good::Flax, 600)],
            Self::GrowCabbage => &[(Good::Cabbage, 8)],
            Self::CatchFish => &[(Good::Fish, 1)],
            Self::SpinThread => &[(Good::Thread, 200)],
            Self::WeaveCloth => &[(Good::Cloth, 200)],
            Self::MakeClothingBlock => &[(Good::FlaxBlock, 1)],
            Self::AssembleGarment => &[(Good::FlaxGarment, 1)],
            Self::MillFlour => &[(Good::Flour, 240)],
            Self::ChopWood => &[(Good::Wood, 200)],
            Self::FetchWater => &[(Good::Water, 200)],
            Self::BakeRolls => &[(Good::BreadRoll, 10)],
            Self::BakeBerryPie => &[(Good::BerryPie, 2)],
        }
    }
    pub fn base_duration_ms(self) -> u64 {
        match self {
            Self::Forage
            | Self::CatchFish
            | Self::FetchWater
            | Self::SpinThread
            | Self::WeaveCloth
            | Self::MakeClothingBlock
            | Self::AssembleGarment => 30 * 60_000,
            Self::GrowWheat | Self::GrowFlax | Self::GrowCabbage => 3 * 60 * 60_000,
            Self::BakeBerryPie => 90 * 60_000,
            _ => 60 * 60_000,
        }
    }
    pub fn duration_ms(self, citizen: &Citizen) -> Result<u64, SimulationError> {
        let reduction = if let Some(skill) = self.skill() {
            let practice = citizen
                .skill_practice_ms(skill)
                .ok_or(SimulationError::MissingSkill)?;
            skill_duration_reduction(practice)
        } else {
            0.0
        };
        Ok((self.base_duration_ms() as f64 * (1.0 - reduction)).ceil() as u64)
    }
}

pub fn starting_inputs(role: crate::StartingRole) -> crate::marketplace::ShoppingList {
    let recipe = match role {
        crate::StartingRole::Miller => Recipe::MillFlour,
        crate::StartingRole::Baker => Recipe::BakeRolls,
        crate::StartingRole::Weaver => Recipe::SpinThread,
        crate::StartingRole::Tailor => Recipe::MakeClothingBlock,
        _ => return crate::marketplace::ShoppingList::default(),
    };
    let batches = (4 * 60 * 60_000) / recipe.base_duration_ms();
    crate::marketplace::ShoppingList::new(
        recipe
            .inputs()
            .iter()
            .map(|&(good, units)| (good, units * batches)),
    )
    .expect("prototype recipe quantities are finite")
}

pub fn starting_coins(_role: crate::StartingRole) -> Coins {
    let base = 1000;
    let purchase_allowance = 2000;
    base + purchase_allowance
}

pub const DAILY_CAPACITY_MS: u64 = 12 * 60 * 60_000;
pub(crate) const PRODUCTION_BLOCK_MS: u64 = 2 * 60 * 60_000;
pub const PERSONAL_FOOD_RESERVE: f64 = crate::FOOD_RESERVE_CAP_NUTRITION;
pub const MIN_LISTING_VALUE: f64 = 2.0;
pub const URGENT_HUNGER: f64 = 20.0;
pub const URGENT_TIREDNESS: f64 = 50.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionTargets {
    pub calculated_at_ms: u64,
    remaining_batches: [Quantity; Recipe::COUNT],
    pub(crate) projected_order: Vec<Recipe>,
}

impl ProductionTargets {
    pub fn remaining_batches(&self, recipe: Recipe) -> Quantity {
        self.remaining_batches[recipe as usize]
    }
    pub fn batches(&self) -> impl Iterator<Item = (Recipe, Quantity)> + '_ {
        Recipe::ALL
            .into_iter()
            .map(|recipe| (recipe, self.remaining_batches(recipe)))
            .filter(|(_, batches)| *batches > 0)
    }
    pub fn is_empty(&self) -> bool {
        self.batches().next().is_none()
    }
    pub(crate) fn complete(&mut self, recipe: Recipe) {
        self.remaining_batches[recipe as usize] = self.remaining_batches(recipe).saturating_sub(1);
        if let Some(index) = self
            .projected_order
            .iter()
            .position(|&planned| planned == recipe)
        {
            self.projected_order.remove(index);
        }
    }

    pub fn calculate(citizen: &Citizen, now_ms: u64) -> Result<Self, SimulationError> {
        let mut target = Self {
            calculated_at_ms: now_ms,
            remaining_batches: [0; Recipe::COUNT],
            projected_order: Vec::new(),
        };
        let active = citizen
            .active_action()
            .and_then(|active| active.action().production_recipe());
        target.projected_order = projected_recipes(citizen, DAILY_CAPACITY_MS)?;
        for &recipe in &target.projected_order {
            target.remaining_batches[recipe as usize] += 1;
        }
        if let Some(recipe) = active {
            target.remaining_batches[recipe as usize] += 1;
            target.projected_order.insert(0, recipe);
        }
        Ok(target)
    }
}

pub(crate) fn crop_kind(recipe: Recipe) -> Option<crate::farming::CropKind> {
    crate::farming::CropKind::ALL
        .into_iter()
        .find(|kind| kind.recipe() == recipe)
}

pub(crate) fn stock_cap_batches(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    let days = if crop_kind(recipe).is_some() {
        crate::farming::GROWTH_MS as f64 / crate::marketplace::DAY_MS as f64
    } else {
        1.0
    };
    Some(DAILY_CAPACITY_MS as f64 * days / recipe.duration_ms(citizen).ok()? as f64)
}

pub(crate) fn production_stock(good: Good, citizen: &Citizen) -> f64 {
    let mut stock =
        citizen.units(good) as f64 + citizen.market().listed_units(citizen.id(), good) as f64;
    if let Some(active) = citizen.active_action()
        && let Some(recipe) = active.action().production_recipe()
    {
        stock = (stock
            - recipe
                .inputs()
                .iter()
                .filter(|(input, _)| *input == good)
                .map(|(_, units)| *units as f64)
                .sum::<f64>())
        .max(0.0);
        stock += recipe
            .outputs()
            .iter()
            .filter(|(output, _)| *output == good)
            .map(|(_, units)| *units as f64)
            .sum::<f64>();
    }
    if let Some(active) = citizen.active_action()
        && let crate::CitizenAction::Plant { crop, .. } = active.action()
        && crop.good() == good
    {
        stock += crop.recipe().outputs()[0].1 as f64;
    }
    for place in citizen.owned_properties() {
        for crop in place
            .plots
            .iter()
            .flatten()
            .filter(|crop| crop.kind.good() == good)
        {
            let travel_ms = (citizen.position().distance(place.position)
                * crate::locations::WALK_MS_PER_METRE)
                .ceil() as u64;
            let tending_ms = citizen
                .skill_practice_ms(Skill::Farming)
                .map(|practice| {
                    (crate::farming::HOUR_MS as f64 / 2.0
                        * (1.0 - skill_duration_reduction(practice)))
                    .ceil() as u64
                })
                .unwrap_or(u64::MAX);
            let finish = citizen
                .current_time_ms()
                .saturating_add(travel_ms)
                .saturating_add(tending_ms);
            let mut tending_finish = [finish; 2];
            if let Some(active) = citizen.active_action()
                && let crate::CitizenAction::Tend { field, plot, stage } = active.action()
                && field == place.id
                && place
                    .plots
                    .get(plot)
                    .is_some_and(|existing| existing.as_ref() == Some(crop))
            {
                tending_finish[stage] = citizen
                    .current_time_ms()
                    .saturating_add(active.remaining_ms());
            }
            stock += crop.planned_yield(citizen.current_time_ms(), tending_finish) as f64;
        }
    }
    stock
}

fn stock_batches(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    recipe
        .outputs()
        .iter()
        .map(|&(good, units)| production_stock(good, citizen) / units as f64)
        .max_by(f64::total_cmp)
}

pub(crate) fn stock_fraction(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    Some(stock_batches(recipe, citizen)? / stock_cap_batches(recipe, citizen)?)
}

pub fn recipe_profit(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    recipe_profit_at(recipe, citizen, stock_fraction(recipe, citizen)? >= 1.0)
}

fn recipe_profit_at(recipe: Recipe, citizen: &Citizen, export: bool) -> Option<f64> {
    let owned = std::array::from_fn(|index| citizen.usable_owned_units(Good::ALL[index]));
    let profit = output_value(recipe, citizen, export)? - input_value(recipe, citizen, &owned)?;
    profit.is_finite().then_some(profit)
}

fn output_value(recipe: Recipe, citizen: &Citizen, export: bool) -> Option<f64> {
    recipe
        .outputs()
        .iter()
        .map(|&(good, units)| {
            let price = if export {
                if !citizen.market().caravan_policy().exports[good as usize].allowed {
                    return None;
                }
                citizen.market().effective_export_threshold(good)?
            } else {
                citizen.prices().price(good)?
            };
            Some(
                price * units as f64 / good.units_per_price_unit() as f64
                    * (1.0
                        - f64::from(citizen.production_socage_rate(recipe, good).basis_points())
                            / 10_000.0),
            )
        })
        .try_fold(0.0, |sum, value| Some(sum + value?))
}
fn input_value(recipe: Recipe, citizen: &Citizen, owned: &[Quantity; Good::COUNT]) -> Option<f64> {
    let mut value = 0.0;
    let missing =
        crate::marketplace::ShoppingList::new(recipe.inputs().iter().map(|&(good, units)| {
            let held = units.min(owned[good as usize]);
            value += citizen.prices().value(good, held).unwrap_or(f64::NAN);
            (good, units - held)
        }))
        .ok()?;
    value += citizen.market().estimated_purchase_cost_from(
        citizen.id(),
        &citizen.map(),
        citizen.position(),
        missing,
    )? as f64;
    value.is_finite().then_some(value)
}

pub(crate) fn recipe_requirements(
    citizen: &Citizen,
    recipes: &[Recipe],
) -> Result<crate::marketplace::ShoppingList, SimulationError> {
    let mut balance = [0_i128; Good::COUNT];
    let mut needed = [0_i128; Good::COUNT];
    // Unobserved tax carry cannot fund future dependencies; round cumulative claims upward.
    let mut tax_carry = [0_u64; Good::COUNT];
    for &recipe in recipes {
        for &(good, units) in recipe.inputs() {
            balance[good as usize] -= units as i128;
            needed[good as usize] = needed[good as usize].max(-balance[good as usize]);
        }
        // Growing crops increase expected stock but cannot supply this work block.
        if crop_kind(recipe).is_none() {
            for &(good, units) in recipe.outputs() {
                balance[good as usize] +=
                    usable_output(recipe, good, units, citizen, &mut tax_carry) as i128;
            }
        }
    }
    crate::marketplace::ShoppingList::new(Good::ALL.into_iter().map(|good| {
        (
            good,
            Quantity::try_from(needed[good as usize]).unwrap_or(Quantity::MAX),
        )
    }))
}

fn usable_output(
    recipe: Recipe,
    good: Good,
    units: Quantity,
    citizen: &Citizen,
    tax_carry: &mut [u64; Good::COUNT],
) -> Quantity {
    let rate = u64::from(citizen.production_socage_rate(recipe, good).basis_points());
    let claim = u128::from(units) * u128::from(rate);
    let previous = tax_carry[good as usize];
    let total = u128::from(previous) + claim;
    let reserved = total.div_ceil(10000) - u128::from(previous).div_ceil(10000);
    tax_carry[good as usize] = (total % 10000) as u64;
    units.saturating_sub(reserved as u64)
}

pub(crate) fn projected_recipes(
    citizen: &Citizen,
    horizon_ms: u64,
) -> Result<Vec<Recipe>, SimulationError> {
    let mut owned = std::array::from_fn(|index| citizen.usable_owned_units(Good::ALL[index]));
    let mut stock = std::array::from_fn::<_, { Good::COUNT }, _>(|index| {
        production_stock(Good::ALL[index], citizen)
    });
    if let Some(active) = citizen.active_action()
        && let Some(recipe) = active.action().production_recipe()
    {
        for &(good, units) in recipe.inputs() {
            owned[good as usize] = owned[good as usize].saturating_sub(units);
        }
    }
    // Start with no fractional tax credit so future supplies never rely on an unobserved carry.
    let mut tax_carry = [0_u64; Good::COUNT];
    let mut quoted = citizen.clone();
    let crop_choices: Vec<_> = citizen
        .farming_actions()
        .into_iter()
        .filter_map(|action| match action {
            crate::CitizenAction::Plant { crop, .. } => Some(crop),
            _ => None,
        })
        .collect();
    let mut empty_plots = citizen
        .owned_properties()
        .map(|place| place.plots.iter().filter(|plot| plot.is_none()).count())
        .sum::<usize>();
    let mut elapsed = 0;
    let mut recipes = Vec::new();
    while elapsed < horizon_ms {
        let next = citizen
            .available_production_recipes()
            .filter_map(|recipe| {
                if let Some(kind) = crop_kind(recipe)
                    && (empty_plots == 0 || !crop_choices.contains(&kind))
                {
                    return None;
                }
                let cap = stock_cap_batches(recipe, citizen)?;
                let batches = recipe
                    .outputs()
                    .iter()
                    .map(|&(good, units)| stock[good as usize] / units as f64)
                    .max_by(f64::total_cmp)?;
                let fraction = batches / cap;
                let profit = output_value(recipe, citizen, fraction >= 1.0)?
                    - input_value(recipe, &quoted, &owned)?;
                let duration = recipe.duration_ms(citizen).ok()?;
                if horizon_ms == DAILY_CAPACITY_MS && duration > horizon_ms.saturating_sub(elapsed)
                {
                    return None;
                }
                let rate = profit / duration as f64;
                (rate > 0.0).then_some((recipe, fraction / rate, rate, duration))
            })
            .min_by(|(a, af, ar, _), (b, bf, br, _)| {
                af.total_cmp(bf)
                    .then_with(|| br.total_cmp(ar))
                    .then_with(|| (*a as usize).cmp(&(*b as usize)))
            });
        let Some((recipe, _, _, duration)) = next else {
            break;
        };
        recipes.push(recipe);
        elapsed += duration;
        if crop_kind(recipe).is_some() {
            empty_plots = empty_plots.saturating_sub(1);
        }
        let mut missing = crate::marketplace::ShoppingList::new(
            recipe
                .inputs()
                .iter()
                .map(|&(good, units)| (good, units.saturating_sub(owned[good as usize]))),
        )?;
        let mut position = citizen.position();
        let mut visited = Vec::new();
        while let Some(place) = quoted.market().nearest_purchase_place(
            citizen.id(),
            &citizen.map(),
            position,
            missing,
            &visited,
        ) {
            let mut remaining = Vec::new();
            for (good, units) in missing.items() {
                let purchase = quoted.market.purchase(
                    citizen.id(),
                    place,
                    (good, units),
                    Coins::MAX,
                    citizen.current_time_ms(),
                    false,
                )?;
                remaining.push((good, units.saturating_sub(purchase.units)));
            }
            missing = crate::marketplace::ShoppingList::new(remaining)?;
            position = citizen.map().position(place);
            visited.push(place);
        }
        for &(good, units) in recipe.inputs() {
            let consumed = units.min(owned[good as usize]);
            owned[good as usize] -= consumed;
            stock[good as usize] = (stock[good as usize] - consumed as f64).max(0.0);
        }
        for &(good, units) in recipe.outputs() {
            stock[good as usize] += units as f64;
            if crop_kind(recipe).is_none() {
                owned[good as usize] = owned[good as usize]
                    .checked_add(usable_output(recipe, good, units, citizen, &mut tax_carry))
                    .ok_or(SimulationError::InventoryOverflow)?;
            }
        }
    }
    Ok(recipes)
}

pub(crate) fn ranked_recipes(citizen: &Citizen) -> Vec<Recipe> {
    let mut recipes: Vec<_> = citizen
        .available_production_recipes()
        .filter_map(|recipe| {
            let rate = recipe_profit(recipe, citizen)? / recipe.duration_ms(citizen).ok()? as f64;
            (rate > 0.0).then_some((recipe, stock_fraction(recipe, citizen)? / rate, rate))
        })
        .collect();
    recipes.sort_by(|(a, af, ar), (b, bf, br)| {
        af.total_cmp(bf)
            .then_with(|| br.total_cmp(ar))
            .then_with(|| (*a as usize).cmp(&(*b as usize)))
    });
    recipes.into_iter().map(|(recipe, _, _)| recipe).collect()
}

pub(crate) fn work_period(time_ms: u64) -> u64 {
    use crate::marketplace::{DAY_MS, UPDATE_TIME_MS};
    if time_ms < UPDATE_TIME_MS {
        0
    } else {
        1 + (time_ms - UPDATE_TIME_MS) / DAY_MS
    }
}

pub(crate) fn period_start(period: u64) -> u64 {
    if period == 0 {
        0
    } else {
        crate::marketplace::UPDATE_TIME_MS + (period - 1) * crate::marketplace::DAY_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universal_gathering_uses_local_stock_caps() {
        for recipe in [Recipe::Forage, Recipe::FetchWater] {
            let base = Citizen::new(0.0).unwrap();
            assert!(recipe_profit(recipe, &base).is_some_and(|profit| profit > 0.0));
            let cap = stock_cap_batches(recipe, &base).unwrap().ceil() as u64;
            let (good, units) = recipe.outputs()[0];
            let full = base.with_good(good, cap * units).unwrap();
            assert_eq!(recipe_profit(recipe, &full), None);
            assert_eq!(
                ProductionTargets::calculate(&full, 0)
                    .unwrap()
                    .remaining_batches(recipe),
                0
            );
            let direct = full
                .start_action(crate::CitizenAction::Produce(recipe))
                .unwrap();
            assert!(direct.active_action().is_some());
        }
    }

    #[test]
    fn stock_ratios_compare_capacity_fractions_across_whole_goods_and_grams() {
        let prices = Prices::default()
            .with_price(Good::Fish, 1.0)
            .unwrap()
            .with_price(Good::Water, 2.5)
            .unwrap();
        let fisher = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Fisher)
            .with_prices(prices)
            .with_good(Good::Fish, 7)
            .unwrap()
            .with_good(Good::Water, 960)
            .unwrap();
        let order = |citizen: &Citizen| {
            ranked_recipes(citizen)
                .into_iter()
                .filter(|recipe| matches!(recipe, Recipe::CatchFish | Recipe::FetchWater))
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&fisher)[0], Recipe::CatchFish);
        // Fish earn twice the hourly profit; ten fish exceed twice water's stock fraction.
        assert_eq!(
            order(&fisher.with_good(Good::Fish, 10).unwrap())[0],
            Recipe::FetchWater
        );
    }

    #[test]
    fn export_valuation_changes_at_the_cap_and_respects_tariffs_and_disabled_goods() {
        let fisher = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Fisher)
            .with_prices(Prices::default().with_price(Good::Fish, 100.0).unwrap())
            .with_good(Good::Fish, 23)
            .unwrap();
        assert_eq!(recipe_profit(Recipe::CatchFish, &fisher), Some(100.0));
        let fisher = fisher.with_good(Good::Fish, 24).unwrap();
        assert_eq!(
            recipe_profit(Recipe::CatchFish, &fisher),
            Some(Good::Fish.export_threshold())
        );
        let mut market = fisher.market().clone();
        let mut policy = market.caravan_policy();
        policy.export_tariff_basis_points = 5000;
        market.set_caravan_policy(policy).unwrap();
        assert_eq!(
            recipe_profit(Recipe::CatchFish, &fisher.with_market(market.clone())),
            Some(Good::Fish.export_threshold() / 2.0)
        );
        policy.exports[Good::Fish as usize].allowed = false;
        market.set_caravan_policy(policy).unwrap();
        assert_eq!(
            recipe_profit(Recipe::CatchFish, &fisher.with_market(market)),
            None
        );
    }

    #[test]
    fn owned_ingredients_keep_reference_value_and_local_budgets_stop_at_export_loss() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Miller);
        let (map, _) = citizen
            .map()
            .with_place(
                Location::Mill,
                citizen.position(),
                Some(citizen.id()),
                "Mill",
            )
            .unwrap();
        let citizen = citizen
            .with_map(map)
            .unwrap()
            .with_coins(10000)
            .unwrap()
            .with_prices(
                Prices::default()
                    .with_price(Good::Wheat, 100.0)
                    .unwrap()
                    .with_price(Good::Flour, 200.0)
                    .unwrap(),
            )
            .with_good(Good::Wheat, 10000)
            .unwrap()
            .with_good(Good::Flour, 240 * 11)
            .unwrap();
        assert!((recipe_profit(Recipe::MillFlour, &citizen).unwrap() - 18.0).abs() < 1e-10);
        assert!(
            ProductionTargets::calculate(&citizen, 0)
                .unwrap()
                .remaining_batches(Recipe::MillFlour)
                <= 1
        );
        let capped = citizen.with_good(Good::Flour, 240 * 12).unwrap();
        assert!(recipe_profit(Recipe::MillFlour, &capped).unwrap() < 0.0);
        assert_eq!(
            ProductionTargets::calculate(&capped, 0)
                .unwrap()
                .remaining_batches(Recipe::MillFlour),
            0
        );
    }

    fn quoted_weaver(owned: Quantity) -> Citizen {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Weaver);
        let (map, _) = citizen
            .map()
            .with_place(
                Location::Weavery,
                citizen.position(),
                Some(citizen.id()),
                "Weavery",
            )
            .unwrap();
        let citizen = citizen.with_map(map).unwrap();
        let mut market = citizen.market().clone();
        market.prices = market.prices.with_price(Good::Thread, 175.5).unwrap();
        market
            .list(
                crate::AgentId(uuid::Uuid::new_v4()),
                citizen.selling_place(),
                Good::Thread,
                10000,
            )
            .unwrap();
        market.prices = market
            .prices
            .with_price(Good::Thread, 22.34)
            .unwrap()
            .with_price(Good::Cloth, 175.62)
            .unwrap()
            .with_price(Good::Flax, 1000000.0)
            .unwrap();
        if owned > 0 {
            market
                .list(citizen.id(), citizen.selling_place(), Good::Thread, owned)
                .unwrap();
        }
        citizen.with_market(market)
    }

    #[test]
    fn listed_inputs_use_reference_value_and_only_shortfalls_pay_rounded_quotes() {
        let owned = quoted_weaver(1440);
        assert!(
            (recipe_profit(Recipe::WeaveCloth, &owned).unwrap() - (35.124 - 4.468)).abs() < 1e-10
        );
        let missing = quoted_weaver(0);
        assert!(
            (recipe_profit(Recipe::WeaveCloth, &missing).unwrap() - (35.124 - 36.0)).abs() < 1e-10
        );
        let partial = quoted_weaver(100);
        assert!(
            (recipe_profit(Recipe::WeaveCloth, &partial).unwrap() - (35.124 - 2.234 - 18.0)).abs()
                < 1e-10
        );
        let mut reserved = missing.with_good(Good::Thread, 200).unwrap();
        reserved.tax_reserved.insert(Good::Thread, 200);
        assert_eq!(
            recipe_profit(Recipe::WeaveCloth, &reserved),
            recipe_profit(Recipe::WeaveCloth, &missing)
        );
        let mut inaccessible = missing.clone();
        let place = inaccessible.map().public_place(Location::River);
        inaccessible
            .market
            .list(inaccessible.id(), place, Good::Thread, 200)
            .unwrap();
        assert_eq!(
            recipe_profit(Recipe::WeaveCloth, &inaccessible),
            recipe_profit(Recipe::WeaveCloth, &missing)
        );
    }

    #[test]
    fn ingredient_projection_uses_workshop_outputs_but_waits_for_future_crops() {
        let workshop = recipe_requirements(
            &Citizen::new(0.0).unwrap(),
            &[Recipe::SpinThread, Recipe::WeaveCloth],
        )
        .unwrap();
        assert_eq!(workshop.units(Good::Flax), 200);
        assert_eq!(workshop.units(Good::Thread), 0);
        let crop = recipe_requirements(
            &Citizen::new(0.0).unwrap(),
            &[Recipe::GrowFlax, Recipe::SpinThread],
        )
        .unwrap();
        assert_eq!(crop.units(Good::Flax), 200);
    }

    #[test]
    fn projected_dependencies_exclude_socage_and_carry_fractional_whole_item_claims() {
        let mut citizen = Citizen::new(0.0).unwrap();
        citizen.socage_rates.insert(
            (Recipe::SpinThread, Good::Thread),
            crate::taxation::TaxRate::new(5000).unwrap(),
        );
        let requirements =
            recipe_requirements(&citizen, &[Recipe::SpinThread, Recipe::WeaveCloth]).unwrap();
        assert_eq!(requirements.units(Good::Thread), 100);
        citizen.socage_rates.insert(
            (Recipe::MakeClothingBlock, Good::FlaxBlock),
            crate::taxation::TaxRate::new(5000).unwrap(),
        );
        let mut recipes = vec![Recipe::MakeClothingBlock; 16];
        recipes.push(Recipe::AssembleGarment);
        assert_eq!(
            recipe_requirements(&citizen, &recipes)
                .unwrap()
                .units(Good::FlaxBlock),
            0
        );
        let mut recipes = vec![Recipe::MakeClothingBlock; 15];
        recipes.push(Recipe::AssembleGarment);
        assert_eq!(
            recipe_requirements(&citizen, &recipes)
                .unwrap()
                .units(Good::FlaxBlock),
            1
        );
    }

    #[test]
    fn imported_ingredients_do_not_consume_tax_reserved_stock_for_recipe_ratios() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker);
        let (map, _) = citizen
            .map()
            .with_place(
                Location::Bakery,
                citizen.position(),
                Some(citizen.id()),
                "Bakery",
            )
            .unwrap();
        let mut citizen = citizen
            .with_map(map)
            .unwrap()
            .with_good(Good::Berries, 5520)
            .unwrap()
            .with_good(Good::Flour, 10000)
            .unwrap()
            .with_good(Good::Wood, 10000)
            .unwrap()
            .with_good(Good::Water, 10000)
            .unwrap();
        citizen.tax_reserved.insert(Good::Berries, 5520);
        let recipes = projected_recipes(&citizen, DAILY_CAPACITY_MS).unwrap();
        assert!(recipes.contains(&Recipe::BakeBerryPie));
        assert!(!recipes.contains(&Recipe::Forage));
    }

    #[test]
    fn owned_profit_does_not_subsidise_later_import_batches() {
        let citizen = quoted_weaver(1400)
            .with_good(Good::Berries, 10000)
            .unwrap()
            .with_good(Good::Water, 10000)
            .unwrap();
        let recipes = projected_recipes(&citizen, DAILY_CAPACITY_MS).unwrap();
        assert_eq!(recipes, vec![Recipe::WeaveCloth; 7]);
        let covered = quoted_weaver(800);
        assert!(
            covered
                .production_restock_requirements()
                .unwrap()
                .items()
                .next()
                .is_none()
        );
        let short = quoted_weaver(200);
        // The preferred mix can use one owned batch; further imported cloth work loses money.
        assert!(
            projected_recipes(&short, DAILY_CAPACITY_MS)
                .unwrap()
                .iter()
                .filter(|&&recipe| recipe == Recipe::WeaveCloth)
                .count()
                <= 1
        );
    }

    #[test]
    fn crop_caps_amortise_work_and_pending_plant_output_is_counted_once() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Farmer);
        let (map, field) = citizen
            .map()
            .with_place(
                Location::Field,
                citizen.position(),
                Some(citizen.id()),
                "Field",
            )
            .unwrap();
        let citizen = citizen.with_map(map).unwrap();
        assert_eq!(stock_cap_batches(Recipe::GrowWheat, &citizen), Some(16.0));
        let planted = citizen
            .start_action(crate::CitizenAction::Plant {
                field,
                plot: 0,
                crop: crate::farming::CropKind::Wheat,
            })
            .unwrap();
        assert_eq!(production_stock(Good::Wheat, &planted), 600.0);
        let completed = planted.advance(crate::farming::HOUR_MS).unwrap();
        assert_eq!(production_stock(Good::Wheat, &completed), 600.0);
    }

    #[test]
    fn promised_crop_care_requires_time_for_travel_and_work_but_uses_active_remaining_time() {
        use crate::farming::{Crop, CropKind, HOUR_MS};
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Farmer);
        let (map, field) = citizen
            .map()
            .with_place(
                Location::Field,
                citizen.position(),
                Some(citizen.id()),
                "Field",
            )
            .unwrap();
        let mut map = map;
        map.set_crop(
            field,
            0,
            Some(Crop {
                kind: CropKind::Wheat,
                planted_at_ms: 0,
                tended: [false; 2],
            }),
        )
        .unwrap();
        let mut citizen = citizen.with_map(map).unwrap();
        citizen.market_time_ms = 48 * HOUR_MS - HOUR_MS / 2;
        assert_eq!(production_stock(Good::Wheat, &citizen), 600.0);
        let remote = citizen
            .with_position(crate::locations::Position { x: 100.0, y: 0.0 })
            .unwrap();
        assert_eq!(production_stock(Good::Wheat, &remote), 375.0);
        let tending = citizen
            .start_action(crate::CitizenAction::Tend {
                field,
                plot: 0,
                stage: 0,
            })
            .unwrap()
            .advance(HOUR_MS / 4)
            .unwrap();
        assert_eq!(production_stock(Good::Wheat, &tending), 600.0);
        citizen.market_time_ms += HOUR_MS / 4;
        assert_eq!(production_stock(Good::Wheat, &citizen), 375.0);
        citizen.market_time_ms = 72 * HOUR_MS - HOUR_MS / 4;
        assert_eq!(production_stock(Good::Wheat, &citizen), 0.0);
    }

    #[test]
    fn projected_mix_keeps_profitable_work_with_fragmented_local_quotes() {
        use crate::locations::{Location, Map, Position};
        let map = Map::new(
            Position { x: 10.0, y: 0.0 },
            Position { x: -11.0, y: 0.0 },
            Position { x: 20.0, y: 0.0 },
        )
        .unwrap();
        let mut market = crate::marketplace::Market::default();
        market.prices = market
            .prices
            .with_price(Good::Flour, 1.0)
            .unwrap()
            .with_price(Good::Wood, 1.0)
            .unwrap()
            .with_price(Good::BreadRoll, 100.0)
            .unwrap();
        for _ in 0..12 {
            market
                .list(
                    crate::AgentId(uuid::Uuid::new_v4()),
                    map.public_place(Location::River),
                    Good::Flour,
                    50,
                )
                .unwrap();
        }
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        market
            .list(
                seller,
                map.public_place(Location::Market),
                Good::Flour,
                1000,
            )
            .unwrap();
        market
            .list(seller, map.public_place(Location::Forest), Good::Wood, 150)
            .unwrap();
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(crate::StartingRole::Baker)
            .with_map(map.clone())
            .unwrap()
            .with_coins(2)
            .unwrap()
            .with_good(Good::Wood, 150)
            .unwrap()
            .with_good(Good::Water, 1200)
            .unwrap();
        let (universe, buyer) = crate::Universe::with_map(map)
            .with_citizen("Baker", citizen)
            .unwrap();
        let (universe, _) = universe.with_property(buyer, Location::Bakery).unwrap();
        let crate::AgentKind::Citizen(citizen) = &universe.agents()[&buyer].kind;
        let citizen = citizen.with_market(market);
        let targets = ProductionTargets::calculate(&citizen, 0).unwrap();
        assert_eq!(targets.remaining_batches(Recipe::BakeRolls), 10);
    }

    use crate::{StartingRole, marketplace::Prices};

    #[test]
    fn clothing_chain_preserves_mass_and_has_positive_margins() {
        let prices = Prices::default();
        for recipe in [
            Recipe::SpinThread,
            Recipe::WeaveCloth,
            Recipe::MakeClothingBlock,
            Recipe::AssembleGarment,
        ] {
            let mass = |items: &[(Good, Quantity)]| {
                items
                    .iter()
                    .map(|(good, units)| good.weight_grams() * units)
                    .sum::<u64>()
            };
            let value = |items: &[(Good, Quantity)]| {
                items
                    .iter()
                    .map(|&(good, units)| prices.value(good, units).unwrap())
                    .sum::<f64>()
            };
            assert_eq!(mass(recipe.inputs()), mass(recipe.outputs()));
            assert!(value(recipe.outputs()) > value(recipe.inputs()));
            assert_eq!(recipe.base_duration_ms(), 30 * 60_000);
        }
        assert_eq!(Recipe::GrowFlax.outputs(), &[(Good::Flax, 600)]);
        assert_eq!(Recipe::GrowFlax.location(), Location::Field);
        assert_eq!(Recipe::GrowFlax.skill(), Some(Skill::Farming));
        assert_eq!(Recipe::GrowFlax.base_duration_ms(), 3 * 60 * 60_000);
        assert_eq!(
            starting_inputs(StartingRole::Weaver).units(Good::Flax),
            1600
        );
        assert_eq!(
            starting_inputs(StartingRole::Tailor).units(Good::Cloth),
            200
        );
    }

    #[test]
    fn clothing_goods_use_grams_or_whole_items_without_nutrition() {
        let prices = Prices::default();
        for (good, quoted, units, weight) in [
            (Good::Flax, Good::Flax.core_price(), 1000, 1),
            (Good::Thread, Good::Thread.core_price(), 1000, 1),
            (Good::Cloth, Good::Cloth.core_price(), 1000, 1),
            (Good::FlaxBlock, Good::FlaxBlock.core_price(), 1, 25),
            (Good::FlaxGarment, Good::FlaxGarment.core_price(), 1, 200),
        ] {
            assert_eq!(prices.price(good), Some(quoted));
            assert_eq!(good.units_per_price_unit(), units);
            assert_eq!(good.weight_grams(), weight);
            assert_eq!(good.nutrition_per_unit(), None);
            assert_eq!(good.eating_ms_per_unit(), None);
        }
    }
}
