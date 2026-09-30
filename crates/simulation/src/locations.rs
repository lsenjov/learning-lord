use crate::SimulationError;
use rand::RngExt;

pub const MAP_HALF_SIZE_METRES: f64 = 1000.0;
pub const WALK_MS_PER_METRE: f64 = 600.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub(crate) fn valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.x.abs() <= MAP_HALF_SIZE_METRES
            && self.y.abs() <= MAP_HALF_SIZE_METRES
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    House,
    Forest,
    River,
    Market,
}

impl Location {
    pub const ALL: [Self; 4] = [Self::House, Self::Forest, Self::River, Self::Market];

    pub fn name(self) -> &'static str {
        match self {
            Self::House => "House",
            Self::Forest => "Forest",
            Self::River => "River",
            Self::Market => "Market",
        }
    }
}

/// Standalone fixtures may co-locate sites; universes generate a distinct map by default.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Map {
    forest: Position,
    river: Position,
    market: Position,
}

impl Map {
    pub fn new(
        forest: Position,
        river: Position,
        market: Position,
    ) -> Result<Self, SimulationError> {
        if ![forest, river, market].into_iter().all(Position::valid) {
            return Err(SimulationError::InvalidPosition);
        }
        Ok(Self {
            forest,
            river,
            market,
        })
    }

    pub fn random() -> Self {
        let mut rng = rand::make_rng::<rand::rngs::SmallRng>();
        let mut points = vec![Position::default()];
        while points.len() < 4 {
            let point = Position {
                x: rng.random_range(-850.0..=850.0),
                y: rng.random_range(-850.0..=850.0),
            };
            if points
                .iter()
                .all(|&existing| point.distance(existing) >= 200.0)
            {
                points.push(point);
            }
        }
        Self {
            forest: points[1],
            river: points[2],
            market: points[3],
        }
    }

    pub fn position(self, location: Location) -> Position {
        match location {
            Location::House => Position::default(),
            Location::Forest => self.forest,
            Location::River => self.river,
            Location::Market => self.market,
        }
    }
}
