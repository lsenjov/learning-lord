use crate::{Citizen, SimulationError, locations::Location, marketplace::Good};

pub const SKILL_GAIN_PER_BATCH: f64 = 0.1;
pub const SPEED_GAIN_PER_LEVEL: f64 = 0.05;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Skill {
    Farming,
    Milling,
    Woodcutting,
    Baking,
}

impl Skill {
    pub const COUNT: usize = 4;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Farming,
        Self::Milling,
        Self::Woodcutting,
        Self::Baking,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Farming => "Farming",
            Self::Milling => "Milling",
            Self::Woodcutting => "Woodcutting",
            Self::Baking => "Baking",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Recipe {
    GrowWheat,
    MillFlour,
    ChopWood,
    FetchWater,
    BakeBread,
    BakeBerryPie,
}

impl Recipe {
    pub const COUNT: usize = 6;
    pub const ALL: [Self; Self::COUNT] = [
        Self::GrowWheat,
        Self::MillFlour,
        Self::ChopWood,
        Self::FetchWater,
        Self::BakeBread,
        Self::BakeBerryPie,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::GrowWheat => "Grow wheat",
            Self::MillFlour => "Mill flour",
            Self::ChopWood => "Chop wood",
            Self::FetchWater => "Fetch water",
            Self::BakeBread => "Bake bread",
            Self::BakeBerryPie => "Bake berry pie",
        }
    }
    pub fn skill(self) -> Option<Skill> {
        match self {
            Self::GrowWheat => Some(Skill::Farming),
            Self::MillFlour => Some(Skill::Milling),
            Self::ChopWood => Some(Skill::Woodcutting),
            Self::FetchWater => None,
            Self::BakeBread | Self::BakeBerryPie => Some(Skill::Baking),
        }
    }
    pub fn location(self) -> Location {
        match self {
            Self::GrowWheat => Location::Field,
            Self::MillFlour => Location::Mill,
            Self::ChopWood => Location::Forest,
            Self::FetchWater => Location::River,
            Self::BakeBread | Self::BakeBerryPie => Location::Bakery,
        }
    }
    pub fn inputs(self) -> &'static [(Good, f64)] {
        match self {
            Self::GrowWheat | Self::ChopWood | Self::FetchWater => &[],
            Self::MillFlour => &[(Good::Wheat, 300.0)],
            Self::BakeBread => &[
                (Good::Flour, 100.0),
                (Good::Wood, 25.0),
                (Good::Water, 100.0),
            ],
            Self::BakeBerryPie => &[
                (Good::Flour, 100.0),
                (Good::Wood, 25.0),
                (Good::Water, 50.0),
                (Good::Berries, 100.0),
            ],
        }
    }
    pub fn outputs(self) -> &'static [(Good, f64)] {
        match self {
            Self::GrowWheat => &[(Good::Wheat, 200.0)],
            Self::MillFlour => &[(Good::Flour, 240.0)],
            Self::ChopWood => &[(Good::Wood, 200.0)],
            Self::FetchWater => &[(Good::Water, 200.0)],
            Self::BakeBread => &[(Good::Bread, 200.0)],
            Self::BakeBerryPie => &[(Good::BerryPie, 250.0)],
        }
    }
    pub fn base_duration_ms(self) -> u64 {
        match self {
            Self::FetchWater => 30 * 60_000,
            Self::BakeBerryPie => 90 * 60_000,
            _ => 60 * 60_000,
        }
    }
    pub fn duration_ms(self, citizen: &Citizen) -> Result<u64, SimulationError> {
        let speed = if let Some(skill) = self.skill() {
            let level = citizen.skill_level(skill);
            if level <= 0.0 {
                return Err(SimulationError::MissingSkill);
            }
            1.0 + SPEED_GAIN_PER_LEVEL * (level - 1.0).max(0.0)
        } else {
            1.0
        };
        Ok((self.base_duration_ms() as f64 / speed).ceil().max(1.0) as u64)
    }
}

pub fn starting_inputs(role: crate::StartingRole) -> crate::marketplace::ShoppingList {
    let recipe = match role {
        crate::StartingRole::Miller => Recipe::MillFlour,
        crate::StartingRole::Baker => Recipe::BakeBread,
        _ => return crate::marketplace::ShoppingList::default(),
    };
    let batches = (4 * 60 * 60_000) as f64 / recipe.base_duration_ms() as f64;
    crate::marketplace::ShoppingList::new(
        recipe
            .inputs()
            .iter()
            .map(|&(good, grams)| (good, grams * batches)),
    )
    .expect("prototype recipe quantities are finite")
}

pub fn starting_coins(_role: crate::StartingRole) -> f64 {
    let base = 1.0;
    let purchase_allowance = 2.0;
    base + purchase_allowance
}

pub const DAILY_CAPACITY_MS: u64 = 12 * 60 * 60_000;
pub const SALES_HISTORY_DAYS: u64 = 7;
pub const PERSONAL_FOOD_RESERVE: f64 = crate::FOOD_RESERVE_CAP_NUTRITION;
pub const MIN_LISTING_VALUE: f64 = 0.02;
pub const URGENT_HUNGER: f64 = 20.0;
pub const URGENT_TIREDNESS: f64 = 50.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionTargets {
    pub calculated_at_ms: u64,
    remaining_batches: [f64; Recipe::COUNT],
}

impl ProductionTargets {
    pub fn remaining_batches(&self, recipe: Recipe) -> f64 {
        self.remaining_batches[recipe as usize]
    }
    pub fn batches(&self) -> impl Iterator<Item = (Recipe, f64)> + '_ {
        Recipe::ALL
            .into_iter()
            .map(|recipe| (recipe, self.remaining_batches(recipe)))
            .filter(|(_, batches)| *batches > 0.0)
    }
    pub fn is_empty(&self) -> bool {
        self.batches().next().is_none()
    }
    pub(crate) fn complete(&mut self, recipe: Recipe) {
        self.remaining_batches[recipe as usize] = (self.remaining_batches(recipe) - 1.0).max(0.0);
    }

    pub fn calculate(citizen: &Citizen, now_ms: u64) -> Result<Self, SimulationError> {
        use crate::marketplace::DAY_MS;
        let mut target = Self {
            calculated_at_ms: now_ms,
            remaining_batches: [0.0; Recipe::COUNT],
        };
        let active = citizen.active_action().and_then(|active| {
            if let crate::CitizenAction::Produce(recipe) = active.action() {
                Some(recipe)
            } else {
                None
            }
        });
        let mut stocks: [f64; Good::COUNT] = std::array::from_fn(|i| {
            citizen.grams(Good::ALL[i]) + citizen.market().listed_grams(citizen.id(), Good::ALL[i])
        });
        let coins = citizen.coins().max(0.0);
        if let Some(recipe) = active {
            for &(good, grams) in recipe.inputs() {
                stocks[good as usize] = (stocks[good as usize] - grams).max(0.0);
            }
        }
        let mut food_left = PERSONAL_FOOD_RESERVE;
        let mut personal_reserve = [0.0; Good::COUNT];
        for good in Good::FOOD {
            let nutrition = good.nutrition_per_gram().unwrap();
            let kept = stocks[good as usize].min(food_left / nutrition);
            personal_reserve[good as usize] = kept;
            food_left = (food_left - kept * nutrition).max(0.0);
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
            for &(good, yield_grams) in recipe.outputs() {
                let sales: f64 = citizen
                    .market()
                    .trades()
                    .iter()
                    .filter(|trade| {
                        trade.seller == citizen.id()
                            && trade.good == good
                            && trade.time_ms > start
                            && trade.time_ms <= now_ms
                    })
                    .map(|trade| trade.grams)
                    .sum();
                let unmet = citizen.market().unmet_grams(good);
                let desired = (sales / observed_days + unmet).max(yield_grams * daily_batches);
                let owned = citizen.grams(good) + citizen.market().listed_grams(citizen.id(), good);
                let saleable = (owned - personal_reserve[good as usize]).max(0.0);
                let progress = active.map_or(0.0, |active| {
                    active
                        .outputs()
                        .iter()
                        .filter(|(g, _)| *g == good)
                        .map(|(_, grams)| *grams)
                        .sum()
                });
                wanted =
                    wanted.max(((desired - saleable - progress).max(0.0) / yield_grams).ceil());
            }
            let mut count = wanted.min(daily_batches);
            if count <= 0.0 {
                continue;
            }
            let cost = |count: f64| -> Option<f64> {
                recipe
                    .inputs()
                    .iter()
                    .map(|&(good, grams)| {
                        citizen.market().estimated_purchase_cost(
                            citizen.id(),
                            good,
                            (grams * count - stocks[good as usize]).max(0.0),
                        )
                    })
                    .try_fold(0.0, |sum, cost| Some(sum + cost?))
                    .filter(|cost| cost.is_finite())
            };
            if cost(count).is_none_or(|cost| cost > coins) {
                let mut low = 0u64;
                let mut high = count as u64;
                while low < high {
                    let middle = low + (high - low).div_ceil(2);
                    if cost(middle as f64).is_some_and(|cost| cost <= coins) {
                        low = middle;
                    } else {
                        high = middle - 1;
                    }
                }
                count = low as f64;
            }
            if count <= 0.0 {
                continue;
            }
            target.remaining_batches[recipe as usize] = count;
        }
        if let Some(recipe) = active {
            target.remaining_batches[recipe as usize] += 1.0;
        }
        Ok(target)
    }
}

pub fn recipe_profit(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    let outputs: f64 = recipe
        .outputs()
        .iter()
        .map(|&(good, grams)| citizen.prices().value(good, grams))
        .try_fold(0.0, |sum, value| Some(sum + value?))?;
    let inputs: f64 = recipe
        .inputs()
        .iter()
        .map(|&(good, grams)| {
            citizen
                .market()
                .estimated_purchase_cost(citizen.id(), good, grams)
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
