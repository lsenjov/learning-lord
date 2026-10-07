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
}

impl Skill {
    pub const COUNT: usize = 6;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Farming,
        Self::Milling,
        Self::Woodcutting,
        Self::Baking,
        Self::Weaving,
        Self::Tailoring,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Farming => "Farming",
            Self::Milling => "Milling",
            Self::Woodcutting => "Woodcutting",
            Self::Baking => "Baking",
            Self::Weaving => "Weaving",
            Self::Tailoring => "Tailoring",
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
    BakeBread,
    BakeBerryPie,
    GrowFlax,
    SpinThread,
    WeaveCloth,
    MakeClothingBlock,
    AssembleGarment,
}

impl Recipe {
    pub const COUNT: usize = 12;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Forage,
        Self::GrowWheat,
        Self::MillFlour,
        Self::ChopWood,
        Self::FetchWater,
        Self::BakeBread,
        Self::BakeBerryPie,
        Self::GrowFlax,
        Self::SpinThread,
        Self::WeaveCloth,
        Self::MakeClothingBlock,
        Self::AssembleGarment,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Forage => "Forage",
            Self::GrowWheat => "Grow wheat",
            Self::MillFlour => "Mill flour",
            Self::ChopWood => "Chop wood",
            Self::FetchWater => "Fetch water",
            Self::BakeBread => "Bake bread",
            Self::BakeBerryPie => "Bake berry pie",
            Self::GrowFlax => "Grow flax",
            Self::SpinThread => "Spin thread",
            Self::WeaveCloth => "Weave cloth",
            Self::MakeClothingBlock => "Make clothing block",
            Self::AssembleGarment => "Assemble garment",
        }
    }
    pub fn skill(self) -> Option<Skill> {
        match self {
            Self::GrowWheat | Self::GrowFlax => Some(Skill::Farming),
            Self::MillFlour => Some(Skill::Milling),
            Self::ChopWood => Some(Skill::Woodcutting),
            Self::FetchWater | Self::Forage => None,
            Self::BakeBread | Self::BakeBerryPie => Some(Skill::Baking),
            Self::SpinThread | Self::WeaveCloth => Some(Skill::Weaving),
            Self::MakeClothingBlock | Self::AssembleGarment => Some(Skill::Tailoring),
        }
    }
    pub fn location(self) -> Location {
        match self {
            Self::GrowWheat | Self::GrowFlax => Location::Field,
            Self::MillFlour => Location::Mill,
            Self::ChopWood | Self::Forage => Location::Forest,
            Self::FetchWater => Location::River,
            Self::BakeBread | Self::BakeBerryPie => Location::Bakery,
            Self::SpinThread | Self::WeaveCloth => Location::Weavery,
            Self::MakeClothingBlock | Self::AssembleGarment => Location::Tailory,
        }
    }
    pub fn inputs(self) -> &'static [(Good, Quantity)] {
        match self {
            Self::GrowWheat | Self::GrowFlax | Self::ChopWood | Self::FetchWater | Self::Forage => {
                &[]
            }
            Self::MillFlour => &[(Good::Wheat, 300)],
            Self::SpinThread => &[(Good::Flax, 200)],
            Self::WeaveCloth => &[(Good::Thread, 200)],
            Self::MakeClothingBlock => &[(Good::Cloth, 25)],
            Self::AssembleGarment => &[(Good::FlaxBlock, 8)],
            Self::BakeBread => &[(Good::Flour, 100), (Good::Wood, 25), (Good::Water, 100)],
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
            Self::GrowWheat => &[(Good::Wheat, 200)],
            Self::GrowFlax => &[(Good::Flax, 200)],
            Self::SpinThread => &[(Good::Thread, 200)],
            Self::WeaveCloth => &[(Good::Cloth, 200)],
            Self::MakeClothingBlock => &[(Good::FlaxBlock, 1)],
            Self::AssembleGarment => &[(Good::FlaxGarment, 1)],
            Self::MillFlour => &[(Good::Flour, 240)],
            Self::ChopWood => &[(Good::Wood, 200)],
            Self::FetchWater => &[(Good::Water, 200)],
            Self::BakeBread => &[(Good::Bread, 2)],
            Self::BakeBerryPie => &[(Good::BerryPie, 2)],
        }
    }
    pub fn base_duration_ms(self) -> u64 {
        match self {
            Self::Forage
            | Self::FetchWater
            | Self::SpinThread
            | Self::WeaveCloth
            | Self::MakeClothingBlock
            | Self::AssembleGarment => 30 * 60_000,
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
        crate::StartingRole::Baker => Recipe::BakeBread,
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
pub const SALES_HISTORY_DAYS: u64 = 7;
pub const PERSONAL_FOOD_RESERVE: f64 = crate::FOOD_RESERVE_CAP_NUTRITION;
pub const MIN_LISTING_VALUE: f64 = 2.0;
pub const URGENT_HUNGER: f64 = 20.0;
pub const URGENT_TIREDNESS: f64 = 50.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionTargets {
    pub calculated_at_ms: u64,
    remaining_batches: [Quantity; Recipe::COUNT],
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
    }

    pub fn calculate(citizen: &Citizen, now_ms: u64) -> Result<Self, SimulationError> {
        use crate::marketplace::DAY_MS;
        let mut target = Self {
            calculated_at_ms: now_ms,
            remaining_batches: [0; Recipe::COUNT],
        };
        let active = citizen.active_action().and_then(|active| {
            if let crate::CitizenAction::Produce(recipe) = active.action() {
                Some(recipe)
            } else {
                None
            }
        });
        let mut stocks = [0; Good::COUNT];
        let mut usable_stocks = [0; Good::COUNT];
        for good in Good::ALL {
            usable_stocks[good as usize] = citizen
                .available_units(good)
                .checked_add(citizen.market().listed_units(citizen.id(), good))
                .ok_or(SimulationError::InventoryOverflow)?;
            stocks[good as usize] = citizen
                .units(good)
                .checked_add(citizen.market().listed_units(citizen.id(), good))
                .ok_or(SimulationError::InventoryOverflow)?;
        }
        let coins = citizen.coins().max(0);
        if let Some(recipe) = active {
            for &(good, units) in recipe.inputs() {
                stocks[good as usize] = stocks[good as usize].saturating_sub(units);
                usable_stocks[good as usize] = usable_stocks[good as usize].saturating_sub(units);
            }
        }
        let mut food_left = PERSONAL_FOOD_RESERVE;
        let mut personal_reserve = [0; Good::COUNT];
        for good in Good::FOOD {
            let nutrition = good.nutrition_per_unit().unwrap();
            let kept = stocks[good as usize].min((food_left / nutrition).ceil() as Quantity);
            personal_reserve[good as usize] = kept;
            food_left = (food_left - kept as f64 * nutrition).max(0.0);
        }
        let mut recipes: Vec<_> = citizen
            .available_recipes()
            .filter_map(|recipe| {
                let profit = recipe_profit(recipe, citizen)?;
                (profit > 0.0)
                    .then_some((recipe, profit / recipe.duration_ms(citizen).ok()? as f64))
            })
            .collect();
        recipes.sort_by(|(a, ap), (b, bp)| {
            bp.total_cmp(ap)
                .then_with(|| (*a as usize).cmp(&(*b as usize)))
        });
        let window = SALES_HISTORY_DAYS * DAY_MS;
        let start = now_ms.saturating_sub(window);
        let observed_days = now_ms.min(window).max(DAY_MS) as f64 / DAY_MS as f64;
        for (recipe, _) in recipes {
            let duration = recipe.duration_ms(citizen)? as f64;
            let daily_batches = (DAILY_CAPACITY_MS as f64 / duration).floor();
            let mut wanted: f64 = 0.0;
            for &(good, yield_units) in recipe.outputs() {
                let sales: f64 = citizen
                    .market()
                    .trades()
                    .iter()
                    .filter(|trade| {
                        trade.seller.citizen() == Some(citizen.id())
                            && trade.good == good
                            && trade.time_ms > start
                            && trade.time_ms <= now_ms
                    })
                    .map(|trade| trade.units as f64)
                    .sum();
                let unmet = citizen.market().unmet_units(good) as f64;
                let desired =
                    (sales / observed_days + unmet).max(yield_units as f64 * daily_batches);
                let owned = stocks[good as usize];
                let saleable = owned.saturating_sub(personal_reserve[good as usize]) as f64;
                let progress = active.map_or(0.0, |active| {
                    active
                        .outputs()
                        .iter()
                        .filter(|(g, _)| *g == good)
                        .map(|(_, units)| *units as f64)
                        .sum()
                });
                wanted = wanted
                    .max(((desired - saleable - progress).max(0.0) / yield_units as f64).ceil());
            }
            let mut count = wanted.min(daily_batches) as Quantity;
            if count == 0 {
                continue;
            }
            let cost = |count: Quantity| -> Option<Coins> {
                recipe
                    .inputs()
                    .iter()
                    .map(|&(good, units)| {
                        citizen.market().estimated_purchase_cost(
                            citizen.id(),
                            good,
                            units
                                .checked_mul(count)?
                                .saturating_sub(usable_stocks[good as usize]),
                        )
                    })
                    .try_fold(0_i64, |sum, cost| sum.checked_add(cost?))
            };
            if cost(count).is_none_or(|cost| cost > coins) {
                let mut low = 0u64;
                let mut high = count;
                while low < high {
                    let middle = low + (high - low).div_ceil(2);
                    if cost(middle).is_some_and(|cost| cost <= coins) {
                        low = middle;
                    } else {
                        high = middle - 1;
                    }
                }
                count = low;
            }
            if count == 0 {
                continue;
            }
            target.remaining_batches[recipe as usize] = count;
        }
        if let Some(recipe) = active {
            target.remaining_batches[recipe as usize] += 1;
        }
        Ok(target)
    }
}

pub fn recipe_profit(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    let outputs: f64 = recipe
        .outputs()
        .iter()
        .map(|&(good, units)| {
            citizen.prices().value(good, units).map(|value| {
                value
                    * (1.0
                        - f64::from(citizen.production_socage_rate(recipe, good).basis_points())
                            / 10_000.0)
            })
        })
        .try_fold(0.0, |sum, value| Some(sum + value?))?;
    let inputs: f64 = recipe
        .inputs()
        .iter()
        .map(|&(good, units)| {
            citizen
                .market()
                .estimated_purchase_cost(citizen.id(), good, units)
                .map(|coins| coins as f64)
        })
        .try_fold(0.0, |sum, value| Some(sum + value?))?;
    let profit = outputs - inputs;
    profit.is_finite().then_some(profit)
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
        assert_eq!(Recipe::GrowFlax.outputs(), &[(Good::Flax, 200)]);
        assert_eq!(Recipe::GrowFlax.location(), Location::Field);
        assert_eq!(Recipe::GrowFlax.skill(), Some(Skill::Farming));
        assert_eq!(Recipe::GrowFlax.base_duration_ms(), 60 * 60_000);
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
