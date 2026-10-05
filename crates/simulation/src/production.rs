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
    pub const ALL: [Self; 6] = [
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

pub fn starting_coins(role: crate::StartingRole) -> f64 {
    match role {
        crate::StartingRole::Farmer | crate::StartingRole::Woodcutter => 2.0,
        _ => 0.0,
    }
}
