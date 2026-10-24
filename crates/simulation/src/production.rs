mod routes;
pub(crate) use routes::production_routes;
pub use routes::{ProductionRouteQuote, ProductionRoutes};

use crate::{
    Citizen, Coins, Quantity, SimulationError,
    locations::{Location, PlaceId},
    marketplace::Good,
};

/// Practice is capped after 52 weeks of twelve-hour working days.
pub const MAX_SKILL_PRACTICE_MS: u64 = 4_368 * 3_600_000;

pub fn skill_duration_reduction(practice_ms: u64) -> f64 {
    let progress = practice_ms.min(MAX_SKILL_PRACTICE_MS) as f64 / MAX_SKILL_PRACTICE_MS as f64;
    0.5 * (2.0 * progress - progress * progress)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Skill {
    Farming,
    Milling,
    Woodcutting,
    Baking,
    Weaving,
    Tailoring,
    Fishing,
    Brewing,
    Carpentry,
}

impl Skill {
    pub const COUNT: usize = 9;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Farming,
        Self::Milling,
        Self::Woodcutting,
        Self::Baking,
        Self::Weaving,
        Self::Tailoring,
        Self::Fishing,
        Self::Brewing,
        Self::Carpentry,
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
            Self::Brewing => "Brewing",
            Self::Carpentry => "Carpentry",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
    GrowHops,
    BrewBeer,
}

impl Recipe {
    pub const COUNT: usize = 16;
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
        Self::GrowHops,
        Self::BrewBeer,
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
            Self::GrowHops => "Grow hops",
            Self::BrewBeer => "Brew beer",
        }
    }
    pub fn skill(self) -> Option<Skill> {
        match self {
            Self::GrowWheat | Self::GrowFlax | Self::GrowCabbage | Self::GrowHops => {
                Some(Skill::Farming)
            }
            Self::BrewBeer => Some(Skill::Brewing),
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
            Self::GrowWheat | Self::GrowFlax | Self::GrowCabbage | Self::GrowHops => {
                Location::Field
            }
            Self::BrewBeer => Location::Brewery,
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
            | Self::GrowHops
            | Self::ChopWood
            | Self::FetchWater
            | Self::Forage
            | Self::CatchFish => &[],
            Self::BrewBeer => &[
                (Good::Wheat, 200),
                (Good::Hops, 20),
                (Good::Wood, 25),
                (Good::Water, 1000),
            ],
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
            Self::GrowHops => &[(Good::Hops, 600)],
            Self::BrewBeer => &[(Good::Beer, 1000)],
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
            Self::GrowWheat | Self::GrowFlax | Self::GrowCabbage | Self::GrowHops => {
                3 * 60 * 60_000
            }
            Self::BrewBeer => 75 * 60_000,
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
        crate::StartingRole::Brewer => Recipe::BrewBeer,
        crate::StartingRole::Miller => Recipe::MillFlour,
        crate::StartingRole::Baker => Recipe::BakeRolls,
        crate::StartingRole::Weaver => Recipe::SpinThread,
        crate::StartingRole::Tailor => Recipe::MakeClothingBlock,
        _ => return crate::marketplace::ShoppingList::default(),
    };
    let batches = if recipe == Recipe::BrewBeer {
        4
    } else {
        (4 * 60 * 60_000) / recipe.base_duration_ms()
    };
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionTargets {
    pub calculated_at_ms: u64,
    remaining_batches: [Quantity; Recipe::COUNT],
    pub(crate) projected_order: Vec<Recipe>,
    pub(crate) site: Option<PlaceId>,
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
        let site = citizen
            .active_action()
            .and_then(|active| match active.action() {
                crate::CitizenAction::ProduceAt { place, .. } => Some(place),
                action => action.crop_plot().map(|(field, _)| field),
            })
            .or_else(|| {
                citizen
                    .production_targets()
                    .and_then(|targets| targets.site)
            });
        Self::calculate_at(citizen, now_ms, site)
    }

    pub(crate) fn calculate_at(
        citizen: &Citizen,
        now_ms: u64,
        site: Option<PlaceId>,
    ) -> Result<Self, SimulationError> {
        Self::calculate_at_with_blocks(citizen, now_ms, site).map(|(targets, _)| targets)
    }

    pub(crate) fn calculate_at_with_blocks(
        citizen: &Citizen,
        now_ms: u64,
        site: Option<PlaceId>,
    ) -> Result<(Self, Vec<Vec<Recipe>>), SimulationError> {
        let mut target = Self {
            calculated_at_ms: now_ms,
            remaining_batches: [0; Recipe::COUNT],
            projected_order: Vec::new(),
            site,
        };
        let active = citizen
            .active_action()
            .and_then(|active| active.action().production_recipe());
        let blocks = projected_route_blocks_at(citizen, site, DAILY_CAPACITY_MS)?;
        target.projected_order = blocks.iter().flatten().copied().collect();
        for &recipe in &target.projected_order {
            target.remaining_batches[recipe as usize] += 1;
        }
        if let Some(recipe) = active {
            target.remaining_batches[recipe as usize] += 1;
            target.projected_order.insert(0, recipe);
        }
        Ok((target, blocks))
    }
}

pub(crate) fn crop_kind(recipe: Recipe) -> Option<crate::farming::CropKind> {
    crate::farming::CropKind::ALL
        .into_iter()
        .find(|kind| kind.recipe() == recipe)
}

pub(crate) fn stock_cap_batches(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    if recipe == Recipe::BrewBeer {
        return Some(
            citizen
                .accessible_properties()
                .map(|place| citizen.accessible_batches(place).count())
                .sum::<usize>() as f64
                * crate::brewing::FERMENTATION_MS as f64
                / (crate::brewing::FERMENTATION_MS + recipe.duration_ms(citizen).ok()?) as f64,
        );
    }
    let days = if crop_kind(recipe).is_some() {
        crate::farming::GROWTH_MS as f64 / crate::marketplace::DAY_MS as f64
    } else {
        1.0
    };
    Some(DAILY_CAPACITY_MS as f64 * days / recipe.duration_ms(citizen).ok()? as f64)
}

pub(crate) fn production_stock(good: Good, citizen: &Citizen) -> f64 {
    let mut stock = citizen.economic_owned_units(good) as f64;
    if let Some(active) = citizen.active_action()
        && let Some(recipe) = active.action().production_recipe()
        && recipe != Recipe::BrewBeer
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
    if good == Good::Beer {
        stock += citizen
            .accessible_properties()
            .map(|place| citizen.accessible_batches(place).flatten().count() as f64 * 1000.0)
            .sum::<f64>();
    }
    for place in citizen.accessible_properties() {
        for crop in citizen
            .accessible_crops(place)
            .flatten()
            .filter(|crop| crop.kind.good() == good)
        {
            stock += planned_crop_stock(citizen, place, crop);
        }
    }
    stock
}

fn planned_crop_stock(
    citizen: &Citizen,
    place: &crate::locations::Place,
    crop: &crate::farming::Crop,
) -> f64 {
    let travel_ms = (citizen.position().distance(place.position)
        * crate::locations::WALK_MS_PER_METRE)
        .ceil() as u64;
    let tending_ms = citizen
        .skill_practice_ms(Skill::Farming)
        .map(|practice| {
            (crate::farming::HOUR_MS as f64 / 2.0 * (1.0 - skill_duration_reduction(practice)))
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
            .plots()
            .get(plot)
            .is_some_and(|existing| existing.as_ref() == Some(crop))
    {
        tending_finish[stage] = citizen
            .current_time_ms()
            .saturating_add(active.remaining_ms());
    }
    crop.planned_yield(citizen.current_time_ms(), tending_finish) as f64
}

fn production_stocks(citizen: &Citizen) -> [f64; Good::COUNT] {
    let mut stock =
        std::array::from_fn(|index| citizen.economic_owned_units(Good::ALL[index]) as f64);
    if let Some(active) = citizen.active_action()
        && let Some(recipe) = active.action().production_recipe()
        && recipe != Recipe::BrewBeer
    {
        for good in Good::ALL {
            let inputs = recipe
                .inputs()
                .iter()
                .filter(|(input, _)| *input == good)
                .map(|(_, units)| *units as f64)
                .sum::<f64>();
            stock[good as usize] = (stock[good as usize] - inputs).max(0.0);
            stock[good as usize] += recipe
                .outputs()
                .iter()
                .filter(|(output, _)| *output == good)
                .map(|(_, units)| *units as f64)
                .sum::<f64>();
        }
    }
    if let Some(active) = citizen.active_action()
        && let crate::CitizenAction::Plant { crop, .. } = active.action()
    {
        stock[crop.good() as usize] += crop.recipe().outputs()[0].1 as f64;
    }
    let mut beer = 0.0;
    for place in citizen.accessible_properties() {
        beer += citizen.accessible_batches(place).flatten().count() as f64 * 1000.0;
        for crop in citizen.accessible_crops(place).flatten() {
            stock[crop.kind.good() as usize] += planned_crop_stock(citizen, place, crop);
        }
    }
    stock[Good::Beer as usize] += beer;
    stock
}

#[cfg(test)]
pub(crate) fn assert_production_stocks_match(citizen: &Citizen) {
    let stock = production_stocks(citizen);
    for good in Good::ALL {
        assert_eq!(
            stock[good as usize].to_bits(),
            production_stock(good, citizen).to_bits(),
            "{good:?}"
        );
    }
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

#[derive(Clone)]
pub(crate) struct ProductionPrices {
    inputs: [f64; Good::COUNT],
    purchasable: [bool; Good::COUNT],
    owned: [Quantity; Good::COUNT],
    stock: Option<[f64; Good::COUNT]>,
    outputs: [f64; Good::COUNT],
    minimum_margin: f64,
}

impl ProductionPrices {
    pub(crate) fn snapshot(citizen: &Citizen) -> Self {
        let reference = |good| citizen.prices().price(good).unwrap_or(f64::NAN);
        Self {
            stock: None,
            owned: std::array::from_fn(|index| citizen.usable_owned_units(Good::ALL[index])),
            purchasable: std::array::from_fn(|index| {
                citizen
                    .market()
                    .lowest_purchase_quote(citizen.market_party(), Good::ALL[index], &citizen.map())
                    .is_some()
            }),
            inputs: std::array::from_fn(|index| {
                let good = Good::ALL[index];
                citizen
                    .market()
                    .lowest_purchase_quote(citizen.market_party(), good, &citizen.map())
                    .unwrap_or_else(|| reference(good))
            }),
            outputs: std::array::from_fn(|index| {
                let good = Good::ALL[index];
                citizen
                    .market()
                    .lowest_listed_quote(good, &citizen.map())
                    .unwrap_or_else(|| reference(good))
            }),
            minimum_margin: f64::from(
                citizen
                    .market()
                    .production_adjustment_rate(citizen.economic_id()),
            ) / 100.0,
        }
    }

    pub(crate) fn meets_margin(
        &self,
        recipe: Recipe,
        citizen: &Citizen,
        site: Option<PlaceId>,
    ) -> bool {
        self.preferred_route(
            recipe,
            citizen,
            site,
            stock_fraction(recipe, citizen).is_some_and(|fraction| fraction >= 1.0),
            &self.owned,
            routes::InputSupply::Available,
        )
        .is_some()
    }

    fn preferred_route(
        &self,
        recipe: Recipe,
        citizen: &Citizen,
        site: Option<PlaceId>,
        export: bool,
        owned: &[Quantity; Good::COUNT],
        supply: routes::InputSupply,
    ) -> Option<ProductionRouteQuote> {
        let routes =
            routes::method_routes_with_owned(recipe, citizen, site, export, self, owned, supply);
        let purchasable = |route: &ProductionRouteQuote| {
            route
                .purchases
                .items()
                .all(|(good, units)| units == 0 || self.purchasable[good as usize])
        };
        [routes.buy, routes.make]
            .into_iter()
            .flatten()
            .filter(|route| {
                route.net_revenue > 0.0 && route.profit / route.net_revenue >= self.minimum_margin
            })
            // Missing offers must generate demand without displacing a chain we can already make.
            .max_by(|a, b| {
                purchasable(a)
                    .cmp(&purchasable(b))
                    .then_with(|| a.profit_per_hour().total_cmp(&b.profit_per_hour()))
            })
    }

    fn batch_cost(&self, recipe: Recipe, citizen: &Citizen, site: Option<PlaceId>) -> Option<f64> {
        let cost = self.input_value(recipe)
            + citizen.workshop_fee_cost(
                site.or_else(|| citizen.production_place(recipe).ok()),
                recipe,
                recipe.duration_ms(citizen).ok()?,
            );
        cost.is_finite().then_some(cost)
    }

    fn input_value(&self, recipe: Recipe) -> f64 {
        recipe
            .inputs()
            .iter()
            .map(|&(good, units)| {
                self.inputs[good as usize] * units as f64 / good.units_per_price_unit() as f64
            })
            .sum()
    }

    fn revenue(
        &self,
        recipe: Recipe,
        citizen: &Citizen,
        site: Option<PlaceId>,
        export: bool,
    ) -> Option<f64> {
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
                    self.outputs[good as usize]
                };
                let tax = site
                    .map_or_else(
                        || citizen.production_socage_rate(recipe, good),
                        |place| citizen.production_socage_rate_at(place, good),
                    )
                    .basis_points();
                let income = citizen
                    .sale_income_basis_points
                    .get(&(citizen.selling_place(), good))
                    .copied()
                    .unwrap_or(0);
                Some(
                    price * units as f64 / good.units_per_price_unit() as f64
                        * (1.0 - f64::from(tax) / 10_000.0)
                        * (1.0 - f64::from(income) / 10_000.0),
                )
            })
            .try_fold(0.0, |sum, value| Some(sum + value?))
    }

    fn profit(
        &self,
        recipe: Recipe,
        citizen: &Citizen,
        site: Option<PlaceId>,
        export: bool,
    ) -> Option<f64> {
        let revenue = self.revenue(recipe, citizen, site, export)?;
        let fees = citizen.workshop_fee_cost(
            site.or_else(|| citizen.production_place(recipe).ok()),
            recipe,
            recipe.duration_ms(citizen).ok()?,
        );
        let profit = revenue - self.input_value(recipe) - fees;
        profit.is_finite().then_some(profit)
    }

    #[cfg(test)]
    fn qualifying_profit(
        &self,
        recipe: Recipe,
        citizen: &Citizen,
        site: Option<PlaceId>,
        export: bool,
    ) -> Option<f64> {
        let revenue = self.revenue(recipe, citizen, site, export)?;
        let profit = self.profit(recipe, citizen, site, export)?;
        (revenue > 0.0 && profit / revenue >= self.minimum_margin).then_some(profit)
    }
}

pub(crate) fn meets_production_margin(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
) -> bool {
    ProductionPrices::snapshot(citizen).meets_margin(recipe, citizen, site)
}

pub fn recipe_profit(recipe: Recipe, citizen: &Citizen) -> Option<f64> {
    recipe_profit_at(recipe, citizen, stock_fraction(recipe, citizen)? >= 1.0)
}

pub(crate) fn recipe_profit_for_site(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
) -> Option<f64> {
    ProductionPrices::snapshot(citizen).profit(
        recipe,
        citizen,
        site,
        stock_fraction(recipe, citizen)? >= 1.0,
    )
}

fn recipe_profit_at(recipe: Recipe, citizen: &Citizen, export: bool) -> Option<f64> {
    ProductionPrices::snapshot(citizen).profit(recipe, citizen, None, export)
}

pub(crate) fn estimated_batch_cost(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
) -> Option<f64> {
    ProductionPrices::snapshot(citizen).batch_cost(recipe, citizen, site)
}

pub(crate) fn recipe_requirements(
    citizen: &Citizen,
    recipes: &[Recipe],
) -> Result<crate::marketplace::ShoppingList, SimulationError> {
    recipe_requirements_at(citizen, None, recipes)
}

pub(crate) fn recipe_requirements_at(
    citizen: &Citizen,
    site: Option<PlaceId>,
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
        if crop_kind(recipe).is_none() && recipe != Recipe::BrewBeer {
            for &(good, units) in recipe.outputs() {
                balance[good as usize] +=
                    usable_output(recipe, good, units, citizen, site, &mut tax_carry) as i128;
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
    site: Option<PlaceId>,
    tax_carry: &mut [u64; Good::COUNT],
) -> Quantity {
    let rate = u64::from(
        site.map_or_else(
            || citizen.production_socage_rate(recipe, good),
            |place| citizen.production_socage_rate_at(place, good),
        )
        .basis_points(),
    );
    let claim = u128::from(units) * u128::from(rate);
    let previous = tax_carry[good as usize];
    let total = u128::from(previous) + claim;
    let reserved = total.div_ceil(10000) - u128::from(previous).div_ceil(10000);
    tax_carry[good as usize] = (total % 10000) as u64;
    units.saturating_sub(reserved as u64)
}

#[cfg(test)]
pub(crate) fn projected_recipes(
    citizen: &Citizen,
    horizon_ms: u64,
) -> Result<Vec<Recipe>, SimulationError> {
    projected_recipes_at(citizen, None, horizon_ms)
}

pub(crate) fn projected_recipes_at(
    citizen: &Citizen,
    site: Option<PlaceId>,
    horizon_ms: u64,
) -> Result<Vec<Recipe>, SimulationError> {
    Ok(projected_route_blocks_at(citizen, site, horizon_ms)?
        .into_iter()
        .flatten()
        .collect())
}

pub(crate) fn projected_route_blocks_at(
    citizen: &Citizen,
    site: Option<PlaceId>,
    horizon_ms: u64,
) -> Result<Vec<Vec<Recipe>>, SimulationError> {
    let candidates = projection_candidates(citizen, site);
    let mut prices = ProductionPrices::snapshot(citizen);
    let eligible: [bool; Recipe::COUNT] = std::array::from_fn(|index| {
        candidates.contains(&Recipe::ALL[index])
            && citizen.eligible_sale_replenishment_with_cost(
                Recipe::ALL[index],
                site,
                prices.batch_cost(Recipe::ALL[index], citizen, site),
            )
    });
    let mut owned = prices.owned;
    let mut stock = production_stocks(citizen);
    if let Some(active) = citizen.active_action()
        && let Some(recipe) = active.action().production_recipe()
        && recipe != Recipe::BrewBeer
    {
        for &(good, units) in recipe.inputs() {
            owned[good as usize] = owned[good as usize].saturating_sub(units);
        }
    }
    let mut tax_carry = [0_u64; Good::COUNT];
    let crop_choices: Vec<_> = site
        .map_or_else(
            || citizen.farming_actions(),
            |field| citizen.farming_actions_at(field),
        )
        .into_iter()
        .filter_map(|action| match action {
            crate::CitizenAction::Plant { crop, .. } => Some(crop),
            _ => None,
        })
        .collect();
    let mut empty_plots = citizen
        .accessible_properties()
        .filter(|place| site.is_none_or(|site| site == place.id))
        .map(|place| {
            citizen
                .accessible_crops(place)
                .filter(|plot| plot.is_none())
                .count()
        })
        .sum::<usize>();
    let mut free_brew_slots = citizen
        .accessible_properties()
        .filter(|place| site.is_none_or(|site| site == place.id))
        .map(|place| {
            citizen
                .accessible_batches(place)
                .filter(|slot| slot.is_none())
                .count()
        })
        .sum::<usize>();
    let mut elapsed = 0;
    let mut blocks = Vec::new();
    while elapsed < horizon_ms {
        prices.stock = Some(stock);
        let next = candidates
            .iter()
            .copied()
            .filter_map(|recipe| {
                if !eligible[recipe as usize] || recipe == Recipe::BrewBeer && free_brew_slots == 0
                {
                    return None;
                }
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
                let route = prices.preferred_route(
                    recipe,
                    citizen,
                    site,
                    fraction >= 1.0,
                    &owned,
                    routes::InputSupply::Projected,
                )?;
                if route
                    .ordered_batches
                    .iter()
                    .any(|batch| !eligible[*batch as usize])
                {
                    return None;
                }
                if horizon_ms == DAILY_CAPACITY_MS
                    && route.duration_ms > horizon_ms.saturating_sub(elapsed)
                {
                    return None;
                }
                let rate = route.profit / route.duration_ms as f64;
                Some((recipe, fraction / rate, rate, route))
            })
            .min_by(|(a, af, ar, _), (b, bf, br, _)| {
                af.total_cmp(bf)
                    .then_with(|| br.total_cmp(ar))
                    .then_with(|| (*a as usize).cmp(&(*b as usize)))
            });
        let Some((_, _, _, route)) = next else {
            break;
        };
        elapsed += route.duration_ms;
        for &recipe in &route.ordered_batches {
            if recipe == Recipe::BrewBeer {
                free_brew_slots = free_brew_slots.saturating_sub(1);
            }
            if crop_kind(recipe).is_some() {
                empty_plots = empty_plots.saturating_sub(1);
            }
            for &(good, units) in recipe.inputs() {
                let consumed = units.min(owned[good as usize]);
                owned[good as usize] -= consumed;
                stock[good as usize] = (stock[good as usize] - consumed as f64).max(0.0);
            }
            for &(good, units) in recipe.outputs() {
                stock[good as usize] += units as f64;
                if crop_kind(recipe).is_none() && recipe != Recipe::BrewBeer {
                    owned[good as usize] = owned[good as usize]
                        .checked_add(usable_output(
                            recipe,
                            good,
                            units,
                            citizen,
                            site,
                            &mut tax_carry,
                        ))
                        .ok_or(SimulationError::InventoryOverflow)?;
                }
            }
        }
        blocks.push(route.ordered_batches);
    }
    Ok(blocks)
}

fn projection_candidates(citizen: &Citizen, site: Option<PlaceId>) -> Vec<Recipe> {
    match site {
        Some(place) => Recipe::ALL
            .into_iter()
            .filter(|recipe| {
                recipe.duration_ms(citizen).is_ok()
                    && citizen.validate_production_site(place, *recipe).is_ok()
            })
            .collect(),
        None => citizen.available_production_recipes().collect(),
    }
}

#[cfg(test)]
pub(crate) fn ranked_recipes(citizen: &Citizen) -> Vec<Recipe> {
    ranked_recipes_at(citizen, None)
}

pub(crate) fn ranked_recipes_at(citizen: &Citizen, site: Option<PlaceId>) -> Vec<Recipe> {
    ranked_recipes_at_prices(citizen, site, &ProductionPrices::snapshot(citizen))
}

pub(crate) fn ranked_recipes_at_prices(
    citizen: &Citizen,
    site: Option<PlaceId>,
    prices: &ProductionPrices,
) -> Vec<Recipe> {
    let mut choices: Vec<_> = projection_candidates(citizen, site)
        .into_iter()
        .filter_map(|recipe| {
            if !citizen.eligible_sale_replenishment_with_cost(
                recipe,
                site,
                prices.batch_cost(recipe, citizen, site),
            ) {
                return None;
            }
            let fraction = stock_fraction(recipe, citizen)?;
            let route = prices.preferred_route(
                recipe,
                citizen,
                site,
                fraction >= 1.0,
                &prices.owned,
                routes::InputSupply::Available,
            )?;
            let first = *route.ordered_batches.first()?;
            let rate = route.profit / route.duration_ms as f64;
            Some((first, fraction / rate, rate))
        })
        .collect();
    choices.sort_by(|(a, af, ar), (b, bf, br)| {
        af.total_cmp(bf)
            .then_with(|| br.total_cmp(ar))
            .then_with(|| (*a as usize).cmp(&(*b as usize)))
    });
    let mut recipes = Vec::new();
    for (recipe, _, _) in choices {
        if !recipes.contains(&recipe) {
            recipes.push(recipe);
        }
    }
    recipes
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
    fn production_snapshot_values_owned_inputs_at_competitive_prices_and_stays_fixed() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Weaver);
        let place = citizen.selling_place();
        let mut market = citizen.market().clone();
        let seller = crate::AgentId(uuid::Uuid::new_v4());
        market
            .list_at_quote(seller, place, Good::Flax, 1, 17.0)
            .unwrap();
        market
            .list_at_quote(seller, place, Good::Thread, 1, 31.0)
            .unwrap();
        let citizen = citizen.with_market(market);
        let prices = ProductionPrices::snapshot(&citizen);
        assert_eq!(prices.inputs[Good::Flax as usize], 17.0);
        assert_eq!(prices.outputs[Good::Thread as usize], 31.0);
        let held = citizen.with_good(Good::Flax, 10000).unwrap();
        assert_eq!(
            prices.input_value(Recipe::SpinThread),
            ProductionPrices::snapshot(&held).input_value(Recipe::SpinThread)
        );
        let mut market = held.market().clone();
        market
            .list_at_quote(
                crate::AgentId(uuid::Uuid::new_v4()),
                place,
                Good::Flax,
                10000,
                3.0,
            )
            .unwrap();
        let changed = held.with_market(market);
        assert_eq!(prices.inputs[Good::Flax as usize], 17.0);
        assert_eq!(
            ProductionPrices::snapshot(&changed).inputs[Good::Flax as usize],
            3.0
        );
    }

    #[test]
    fn production_margin_is_profit_over_net_revenue_after_taxes() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Weaver);
        let mut prices = ProductionPrices {
            inputs: [0.0; Good::COUNT],
            purchasable: [false; Good::COUNT],
            owned: [0; Good::COUNT],
            stock: None,
            outputs: [0.0; Good::COUNT],
            minimum_margin: 0.05,
        };
        prices.outputs[Good::Thread as usize] = 100.0;
        prices.inputs[Good::Flax as usize] = 95.0;
        let revenue = prices
            .revenue(Recipe::SpinThread, &citizen, None, false)
            .unwrap();
        let profit = prices
            .profit(Recipe::SpinThread, &citizen, None, false)
            .unwrap();
        assert!((profit / revenue - 0.05).abs() < 1e-12);
        assert!(
            prices
                .qualifying_profit(Recipe::SpinThread, &citizen, None, false)
                .is_some()
        );
        prices.inputs[Good::Flax as usize] = 95.01;
        assert!(
            prices
                .qualifying_profit(Recipe::SpinThread, &citizen, None, false)
                .is_none()
        );
        let mut taxed = citizen;
        taxed
            .sale_income_basis_points
            .insert((taxed.selling_place(), Good::Thread), 5000);
        prices.inputs[Good::Flax as usize] = 47.5;
        assert_eq!(
            prices
                .revenue(Recipe::SpinThread, &taxed, None, false)
                .unwrap(),
            revenue * 0.5
        );
        assert!(
            prices
                .qualifying_profit(Recipe::SpinThread, &taxed, None, false)
                .is_some()
        );
        prices.inputs[Good::Flax as usize] = 48.0;
        assert!(
            prices
                .qualifying_profit(Recipe::SpinThread, &taxed, None, false)
                .is_none()
        );
    }

    #[test]
    fn bulk_stock_accounts_for_active_production_and_input_clamping() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Miller);
        let (map, mill) = citizen
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
            .with_good(Good::Wheat, 600)
            .unwrap();
        let active = citizen
            .start_action(crate::CitizenAction::ProduceAt {
                place: mill,
                recipe: Recipe::MillFlour,
            })
            .unwrap();
        assert_production_stocks_match(&active);
        assert_eq!(production_stocks(&active)[Good::Wheat as usize], 300.0);
        assert_eq!(production_stocks(&active)[Good::Flour as usize], 240.0);
        let mut short = active;
        short.set_inventory_units(Good::Wheat, 0);
        assert_production_stocks_match(&short);
        assert_eq!(production_stocks(&short)[Good::Wheat as usize], 0.0);
    }

    #[test]
    fn bulk_stock_separates_crop_goods_and_excludes_private_fields() {
        use crate::farming::{Crop, CropKind};
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Farmer);
        let foreign_owner = crate::AgentId(uuid::Uuid::new_v4());
        let mut map = citizen.map().clone();
        for owner in [citizen.id(), foreign_owner] {
            let (next, field) = map
                .with_place(Location::Field, citizen.position(), Some(owner), "Field")
                .unwrap();
            map = next;
            for (plot, kind) in [CropKind::Wheat, CropKind::Flax].into_iter().enumerate() {
                map.set_crop(
                    field,
                    plot,
                    Some(Crop {
                        kind,
                        planted_at_ms: 0,
                        tended: [false; 2],
                    }),
                )
                .unwrap();
            }
        }
        let citizen = citizen.with_map(map).unwrap();
        assert_production_stocks_match(&citizen);
        let stocks = production_stocks(&citizen);
        assert_eq!(stocks[Good::Wheat as usize], 600.0);
        assert_eq!(stocks[Good::Flax as usize], 600.0);
    }

    #[test]
    fn bulk_stock_preserves_rounding_with_large_inventory_and_separate_batches() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Brewer)
            .with_good(Good::Beer, (1_u64 << 60) + 129)
            .unwrap();
        let mut map = citizen.map().clone();
        for name in ["First", "Second", "Third"] {
            let (next, brewery) = map
                .with_place(
                    Location::Brewery,
                    citizen.position(),
                    Some(citizen.id()),
                    name,
                )
                .unwrap();
            map = next;
            map.set_batch(
                brewery,
                0,
                Some(crate::brewing::Batch::Fermenting {
                    ready_at_ms: crate::brewing::FERMENTATION_MS,
                }),
            )
            .unwrap();
        }
        let citizen = citizen.with_map(map).unwrap();
        assert_production_stocks_match(&citizen);
        assert_eq!(
            production_stocks(&citizen)[Good::Beer as usize],
            citizen.economic_owned_units(Good::Beer) as f64 + 3000.0
        );
    }

    #[test]
    fn projection_candidates_preserve_global_and_site_eligibility() {
        let citizen = quoted_weaver(0);
        let foreign_owner = crate::AgentId(uuid::Uuid::new_v4());
        let (map, foreign) = citizen
            .map()
            .with_place(
                Location::Weavery,
                citizen.position(),
                Some(foreign_owner),
                "Foreign",
            )
            .unwrap();
        let public = map.public_place(Location::Market);
        let (map, _) = map
            .with_building(public, Location::Weavery, citizen.id(), "Public workshop")
            .unwrap();
        let (map, town) = map
            .with_place(Location::Warehouse, citizen.position(), None, "Town")
            .unwrap();
        let (map, _) = map
            .with_building(town, Location::Weavery, citizen.id(), "Town workshop")
            .unwrap();
        let citizen = citizen.with_map(map).unwrap();
        let mut sites: Vec<_> = citizen.map().places().keys().copied().map(Some).collect();
        sites.extend([None, Some(PlaceId(uuid::Uuid::new_v4()))]);
        for role in [
            StartingRole::Farmer,
            StartingRole::Miller,
            StartingRole::Woodcutter,
            StartingRole::Baker,
            StartingRole::Weaver,
            StartingRole::Tailor,
            StartingRole::Fisher,
            StartingRole::Brewer,
        ] {
            let citizen = citizen.clone().with_starting_role(role);
            for &site in &sites {
                let previous: Vec<_> = citizen
                    .available_production_recipes()
                    .filter(|&recipe| {
                        site.is_none_or(|place| {
                            citizen.validate_production_site(place, recipe).is_ok()
                        })
                    })
                    .collect();
                assert_eq!(
                    projection_candidates(&citizen, site),
                    previous,
                    "{role:?} {site:?}"
                );
            }
        }
        assert!(projection_candidates(&citizen, Some(public)).contains(&Recipe::WeaveCloth));
        assert!(projection_candidates(&citizen, Some(foreign)).is_empty());
        assert!(projection_candidates(&citizen, Some(town)).is_empty());
        let forest = citizen.map().public_place(Location::Forest);
        assert!(projection_candidates(&citizen, Some(forest)).contains(&Recipe::Forage));
    }

    #[test]
    fn physical_site_projection_uses_its_own_socage_and_net_dependencies() {
        let mut citizen = quoted_weaver(0)
            .with_good(Good::Thread, 1400)
            .unwrap()
            .with_good(Good::Berries, 10000)
            .unwrap()
            .with_good(Good::Water, 10000)
            .unwrap();
        citizen = citizen.with_prices(citizen.prices().with_price(Good::Cloth, 250.0).unwrap());
        let first = citizen.production_place(Recipe::WeaveCloth).unwrap();
        let (map, second) = citizen
            .map()
            .with_place(
                Location::Weavery,
                citizen.position(),
                Some(citizen.id()),
                "Second weavery",
            )
            .unwrap();
        citizen = citizen.with_map(map).unwrap();
        citizen.site_socage_rates.insert(
            (first, Good::Cloth),
            crate::taxation::TaxRate::new(10000).unwrap(),
        );
        assert!(recipe_profit_for_site(Recipe::WeaveCloth, &citizen, Some(first)).unwrap() < 0.0);
        assert!(
            projected_recipes_at(&citizen, Some(first), DAILY_CAPACITY_MS)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            projected_recipes_at(&citizen, Some(second), DAILY_CAPACITY_MS).unwrap(),
            vec![
                Recipe::WeaveCloth;
                (DAILY_CAPACITY_MS / Recipe::WeaveCloth.duration_ms(&citizen).unwrap()) as usize
            ]
        );
        citizen.site_socage_rates.insert(
            (second, Good::Thread),
            crate::taxation::TaxRate::new(5000).unwrap(),
        );
        let queue = [Recipe::SpinThread, Recipe::WeaveCloth];
        assert_eq!(
            recipe_requirements_at(&citizen, Some(second), &queue)
                .unwrap()
                .units(Good::Thread),
            100
        );
        assert_eq!(
            recipe_requirements_at(&citizen, Some(first), &queue)
                .unwrap()
                .units(Good::Thread),
            0
        );
    }

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
            .list_at_quote(
                crate::AgentId(uuid::Uuid::new_v4()),
                citizen.selling_place(),
                Good::Thread,
                10000,
                175.5,
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
    fn owned_and_missing_inputs_use_the_same_competitive_valuation() {
        let expected = 35.124 - 35.1;
        for owned in [0, 100, 1440] {
            let citizen = quoted_weaver(owned);
            assert!(
                (recipe_profit(Recipe::WeaveCloth, &citizen).unwrap() - expected).abs() < 1e-10
            );
            assert!(!meets_production_margin(Recipe::WeaveCloth, &citizen, None));
        }
        let mut reserved = quoted_weaver(0).with_good(Good::Thread, 200).unwrap();
        reserved.tax_reserved.insert(Good::Thread, 200);
        assert!((recipe_profit(Recipe::WeaveCloth, &reserved).unwrap() - expected).abs() < 1e-10);
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
        citizen
            .market
            .list_at_quote(
                crate::AgentId(uuid::Uuid::new_v4()),
                citizen.selling_place(),
                Good::Berries,
                10000,
                0.1,
            )
            .unwrap();

        let recipes = projected_recipes(&citizen, DAILY_CAPACITY_MS).unwrap();
        assert!(recipes.contains(&Recipe::BakeBerryPie));
        assert!(!recipes.contains(&Recipe::Forage));
    }

    #[test]
    fn owned_inventory_does_not_bypass_minimum_margin() {
        for owned in [0, 200, 800, 1400] {
            let citizen = quoted_weaver(owned)
                .with_good(Good::Berries, 10000)
                .unwrap()
                .with_good(Good::Water, 10000)
                .unwrap();
            assert!(
                projected_recipes(&citizen, DAILY_CAPACITY_MS)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                citizen
                    .production_restock_requirements()
                    .unwrap()
                    .items()
                    .next()
                    .is_none()
            );
        }
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
        assert_production_stocks_match(&planted);
        let completed = planted.advance(crate::farming::HOUR_MS).unwrap();
        assert_eq!(production_stock(Good::Wheat, &completed), 600.0);
        assert_production_stocks_match(&completed);
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
        assert_production_stocks_match(&citizen);
        let remote = citizen
            .with_position(crate::locations::Position { x: 100.0, y: 0.0 })
            .unwrap();
        assert_eq!(production_stock(Good::Wheat, &remote), 375.0);
        assert_production_stocks_match(&remote);
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
        assert_production_stocks_match(&tending);
        citizen.market_time_ms += HOUR_MS / 4;
        assert_eq!(production_stock(Good::Wheat, &citizen), 375.0);
        assert_production_stocks_match(&citizen);
        citizen.market_time_ms = 72 * HOUR_MS - HOUR_MS / 4;
        assert_eq!(production_stock(Good::Wheat, &citizen), 0.0);
        assert_production_stocks_match(&citizen);
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
            .unwrap()
            .with_price(Good::BerryPie, 0.01)
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
        assert_eq!(targets.remaining_batches(Recipe::BakeRolls), 11);
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
