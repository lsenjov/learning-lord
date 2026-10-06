use crate::{AgentId, SimulationError};
use imbl::HashMap;
use rand::RngExt;
use uuid::Uuid;

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlaceId(pub Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    Home,
    Field,
    Mill,
    Bakery,
    Weavery,
    Tailory,
    Forest,
    River,
    Market,
}

impl Location {
    pub fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Field => "field",
            Self::Mill => "mill",
            Self::Bakery => "bakery",
            Self::Weavery => "weavery",
            Self::Tailory => "tailory",
            Self::Forest => "Forest",
            Self::River => "River",
            Self::Market => "Market",
        }
    }
    pub fn is_public(self) -> bool {
        matches!(self, Self::Forest | Self::River | Self::Market)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub id: PlaceId,
    pub kind: Location,
    pub position: Position,
    pub owner: Option<AgentId>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    places: HashMap<PlaceId, Place>,
    random_positions: bool,
}

impl Default for Map {
    fn default() -> Self {
        Self::new(
            Position::default(),
            Position::default(),
            Position::default(),
        )
        .unwrap()
    }
}

impl Map {
    pub fn new(
        forest: Position,
        river: Position,
        market: Position,
    ) -> Result<Self, SimulationError> {
        let mut map = Self {
            places: HashMap::new(),
            random_positions: false,
        };
        for (kind, position) in [
            (Location::Forest, forest),
            (Location::River, river),
            (Location::Market, market),
        ] {
            map = map.with_place(kind, position, None, kind.name())?.0;
        }
        Ok(map)
    }
    pub fn random() -> Self {
        let mut map = Self {
            places: HashMap::new(),
            random_positions: true,
        };
        for kind in [Location::Forest, Location::River, Location::Market] {
            map = map
                .with_place(kind, map.next_position(), None, kind.name())
                .unwrap()
                .0;
        }
        map
    }
    pub fn places(&self) -> &HashMap<PlaceId, Place> {
        &self.places
    }
    pub fn place(&self, id: PlaceId) -> Result<&Place, SimulationError> {
        self.places.get(&id).ok_or(SimulationError::PlaceNotFound)
    }
    pub fn public_place(&self, kind: Location) -> PlaceId {
        self.places
            .values()
            .find(|place| place.kind == kind && place.owner.is_none())
            .expect("public site exists")
            .id
    }
    pub fn position(&self, id: PlaceId) -> Position {
        self.place(id).expect("place exists").position
    }
    pub fn next_position(&self) -> Position {
        if !self.random_positions {
            return Position::default();
        }
        let mut rng = rand::make_rng::<rand::rngs::SmallRng>();
        loop {
            let position = Position {
                x: rng.random_range(-850.0..=850.0),
                y: rng.random_range(-850.0..=850.0),
            };
            if self
                .places
                .values()
                .all(|place| position.distance(place.position) >= 200.0)
            {
                return position;
            }
        }
    }
    pub(crate) fn with_place_name(
        &self,
        id: PlaceId,
        name: String,
    ) -> Result<Self, SimulationError> {
        let mut map = self.clone();
        map.places
            .get_mut(&id)
            .ok_or(SimulationError::PlaceNotFound)?
            .name = name;
        Ok(map)
    }

    pub fn with_place(
        &self,
        kind: Location,
        position: Position,
        owner: Option<AgentId>,
        name: impl Into<String>,
    ) -> Result<(Self, PlaceId), SimulationError> {
        if !position.valid() {
            return Err(SimulationError::InvalidPosition);
        }
        if kind.is_public() != owner.is_none() {
            return Err(SimulationError::InvalidOwnership);
        }
        let id = PlaceId(Uuid::new_v4());
        let mut map = self.clone();
        map.places.insert(
            id,
            Place {
                id,
                kind,
                position,
                owner,
                name: name.into(),
            },
        );
        Ok((map, id))
    }
}
