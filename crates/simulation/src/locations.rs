use crate::{AgentId, SimulationError};
use imbl::HashMap;
use rand::RngExt;
use uuid::Uuid;

pub const MAP_HALF_SIZE_METRES: f64 = 1000.0;
pub const WALK_MS_PER_METRE: f64 = 600.0;
pub const PLOT_AREA_LIMIT_M2: u32 = 120;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomSize {
    Small,
    Medium,
    Large,
}
impl RoomSize {
    pub fn area_m2(self) -> u32 {
        match self {
            Self::Small => 6,
            Self::Medium => 12,
            Self::Large => 24,
        }
    }
    pub fn fitting(area_m2: u32) -> Option<Self> {
        [Self::Small, Self::Medium, Self::Large]
            .into_iter()
            .find(|size| size.area_m2() >= area_m2)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq)]
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlaceId(pub Uuid);

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    Home,
    Field,
    Mill,
    Bakery,
    Weavery,
    Tailory,
    Brewery,
    Forest,
    River,
    Market,
    Warehouse,
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
            Self::Brewery => "brewery",
            Self::Forest => "Forest",
            Self::River => "River",
            Self::Market => "Market",
            Self::Warehouse => "Warehouse",
        }
    }
    pub fn minimum_spacing_metres(self) -> f64 {
        match self {
            Self::Forest | Self::River => 200.0,
            Self::Field => 100.0,
            _ => 20.0,
        }
    }
    pub fn is_town_owned(self) -> bool {
        self == Self::Warehouse
    }
    pub fn is_public(self) -> bool {
        matches!(self, Self::Forest | Self::River | Self::Market)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Place {
    pub id: PlaceId,
    pub kind: Location,
    pub position: Position,
    pub owner: Option<AgentId>,
    pub name: String,
    pub buildings: Vec<Building>,
    pub outdoor_objects: Vec<Object>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BuildingId(pub Uuid);
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RoomId(pub Uuid);
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObjectId(pub Uuid);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Building {
    pub id: BuildingId,
    pub name: String,
    pub owner: AgentId,
    pub rooms: Vec<Room>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub size: RoomSize,
    pub objects: Vec<Object>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Object {
    pub id: ObjectId,
    pub kind: ObjectKind,
    pub workers: Vec<AgentId>,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectKind {
    Bed {
        resident: Option<AgentId>,
    },
    PantryShelving,
    DryGoodsBin,
    FishCrate,
    WoodRack,
    TextileChest,
    LiquidBarrel,
    BerryBushes,
    Trees,
    FishingSpot,
    WaterSource,
    Millstone,
    Oven,
    SpinningWheel,
    Loom,
    SewingTable,
    BrewingEquipment,
    FermentationVessel {
        batch: Option<crate::brewing::Batch>,
    },
    CropPlot {
        crop: Option<crate::farming::Crop>,
    },
}
impl ObjectKind {
    pub fn footprint_m2(self) -> u32 {
        match self {
            Self::Bed { .. } | Self::WoodRack | Self::SewingTable => 3,
            Self::PantryShelving
            | Self::DryGoodsBin
            | Self::SpinningWheel
            | Self::FermentationVessel { .. } => 2,
            Self::FishCrate | Self::TextileChest | Self::LiquidBarrel => 1,
            Self::Millstone | Self::Loom | Self::BrewingEquipment => 6,
            Self::Oven => 4,
            Self::BerryBushes
            | Self::Trees
            | Self::FishingSpot
            | Self::WaterSource
            | Self::CropPlot { .. } => 0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Bed { .. } => "Bed",
            Self::PantryShelving => "Pantry shelving",
            Self::DryGoodsBin => "Dry goods bin",
            Self::FishCrate => "Fish crate",
            Self::WoodRack => "Wood rack",
            Self::TextileChest => "Textile chest",
            Self::LiquidBarrel => "Liquid barrel",
            Self::BerryBushes => "Berry bushes",
            Self::Trees => "Trees",
            Self::FishingSpot => "Fishing spot",
            Self::WaterSource => "Water source",
            Self::Millstone => "Millstone",
            Self::Oven => "Oven",
            Self::SpinningWheel => "Spinning wheel",
            Self::Loom => "Loom",
            Self::SewingTable => "Sewing table",
            Self::BrewingEquipment => "Brewing equipment",
            Self::FermentationVessel { .. } => "Fermentation vessel",
            Self::CropPlot { .. } => "Crop plot",
        }
    }
    pub fn accepts(self, good: crate::marketplace::Good) -> bool {
        use crate::marketplace::Good::*;
        match self {
            Self::PantryShelving => matches!(good, Berries | BreadRoll | BerryPie | Cabbage),
            Self::DryGoodsBin => matches!(good, Wheat | Flour | Hops),
            Self::FishCrate => good == Fish,
            Self::WoodRack => good == Wood,
            Self::TextileChest => matches!(good, Flax | Thread | Cloth | FlaxBlock | FlaxGarment),
            Self::LiquidBarrel => matches!(good, Water | Beer),
            _ => false,
        }
    }
    pub fn is_storage(self) -> bool {
        matches!(
            self,
            Self::PantryShelving
                | Self::DryGoodsBin
                | Self::FishCrate
                | Self::WoodRack
                | Self::TextileChest
                | Self::LiquidBarrel
        )
    }
    pub fn single_good(self) -> bool {
        matches!(self, Self::DryGoodsBin | Self::LiquidBarrel)
    }
    fn storage_kit(kind: Location) -> Vec<Self> {
        use ObjectKind::*;
        match kind {
            Location::Home => vec![
                PantryShelving,
                FishCrate,
                TextileChest,
                LiquidBarrel,
                LiquidBarrel,
            ],
            Location::Field => vec![DryGoodsBin, DryGoodsBin, TextileChest, PantryShelving],
            Location::Mill => vec![DryGoodsBin, DryGoodsBin],
            Location::Bakery => vec![DryGoodsBin, WoodRack, LiquidBarrel, PantryShelving],
            Location::Weavery | Location::Tailory => vec![TextileChest],
            Location::Brewery => vec![
                DryGoodsBin,
                DryGoodsBin,
                WoodRack,
                LiquidBarrel,
                LiquidBarrel,
            ],
            Location::Forest => vec![PantryShelving, WoodRack],
            Location::River => vec![FishCrate, LiquidBarrel],
            Location::Market | Location::Warehouse => vec![
                PantryShelving,
                FishCrate,
                WoodRack,
                TextileChest,
                DryGoodsBin,
                DryGoodsBin,
                DryGoodsBin,
                LiquidBarrel,
                LiquidBarrel,
            ],
        }
    }
    pub fn unlimited_workers(self) -> bool {
        matches!(
            self,
            Self::BerryBushes | Self::Trees | Self::FishingSpot | Self::WaterSource
        )
    }
    pub(crate) fn for_recipe(recipe: crate::production::Recipe) -> Self {
        use crate::production::Recipe;
        match recipe {
            Recipe::Forage => Self::BerryBushes,
            Recipe::ChopWood => Self::Trees,
            Recipe::CatchFish => Self::FishingSpot,
            Recipe::FetchWater => Self::WaterSource,
            Recipe::MillFlour => Self::Millstone,
            Recipe::BakeRolls | Recipe::BakeBerryPie => Self::Oven,
            Recipe::SpinThread => Self::SpinningWheel,
            Recipe::WeaveCloth => Self::Loom,
            Recipe::MakeClothingBlock | Recipe::AssembleGarment => Self::SewingTable,
            Recipe::BrewBeer => Self::BrewingEquipment,
            _ => Self::CropPlot { crop: None },
        }
    }
    pub(crate) fn same_capability(self, other: Self) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}
impl Room {
    pub fn used_area_m2(&self) -> u32 {
        self.objects
            .iter()
            .map(|object| object.kind.footprint_m2())
            .sum()
    }
    pub fn available_area_m2(&self) -> u32 {
        self.size.area_m2().saturating_sub(self.used_area_m2())
    }
}
impl Object {
    pub fn new(kind: ObjectKind) -> Self {
        Self {
            id: ObjectId(Uuid::new_v4()),
            kind,
            workers: Vec::new(),
        }
    }
    pub(crate) fn available(&self, worker: AgentId) -> bool {
        self.kind.unlimited_workers() || self.workers.is_empty() || self.workers == [worker]
    }
}
macro_rules! slots {
    ($name:ident, $method:ident, $variant:ident, $field:ident, $ty:ty) => {
        pub struct $name<'a>(&'a Place);
        impl<'a> $name<'a> {
            pub fn iter(&self) -> impl Iterator<Item = &'a Option<$ty>> + 'a + use<'a> {
                self.0.objects().filter_map(|object| match &object.kind {
                    ObjectKind::$variant { $field } => Some($field),
                    _ => None,
                })
            }
            pub fn get(&self, index: usize) -> Option<&'a Option<$ty>> {
                self.iter().nth(index)
            }
            pub fn len(&self) -> usize {
                self.iter().count()
            }
            pub fn is_empty(&self) -> bool {
                self.iter().next().is_none()
            }
            pub fn contains(&self, value: &Option<$ty>) -> bool {
                self.iter().any(|slot| slot == value)
            }
        }
        impl std::ops::Index<usize> for $name<'_> {
            type Output = Option<$ty>;
            fn index(&self, index: usize) -> &Self::Output {
                self.get(index).expect("object slot exists")
            }
        }
        impl Place {
            pub fn $method(&self) -> $name<'_> {
                $name(self)
            }
        }
    };
}
slots!(CropSlots, plots, CropPlot, crop, crate::farming::Crop);
slots!(
    BatchSlots,
    batches,
    FermentationVessel,
    batch,
    crate::brewing::Batch
);
impl Place {
    pub fn used_area_m2(&self) -> u32 {
        self.buildings
            .iter()
            .flat_map(|building| &building.rooms)
            .map(|room| room.size.area_m2())
            .sum()
    }
    pub fn available_area_m2(&self) -> u32 {
        PLOT_AREA_LIMIT_M2.saturating_sub(self.used_area_m2())
    }
    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.outdoor_objects.iter().chain(
            self.buildings
                .iter()
                .flat_map(|building| building.rooms.iter().flat_map(|room| room.objects.iter())),
        )
    }
    pub(crate) fn objects_mut(&mut self) -> impl Iterator<Item = &mut Object> {
        self.outdoor_objects
            .iter_mut()
            .chain(self.buildings.iter_mut().flat_map(|building| {
                building
                    .rooms
                    .iter_mut()
                    .flat_map(|room| room.objects.iter_mut())
            }))
    }
    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects().find(|object| object.id == id)
    }
    pub fn object_owner(&self, id: ObjectId) -> Option<AgentId> {
        self.buildings
            .iter()
            .find(|building| {
                building
                    .rooms
                    .iter()
                    .any(|room| room.objects.iter().any(|object| object.id == id))
            })
            .map(|building| building.owner)
            .or_else(|| {
                self.outdoor_objects
                    .iter()
                    .any(|object| object.id == id)
                    .then_some(self.owner)
                    .flatten()
            })
    }
    pub fn object_room(&self, id: ObjectId) -> Option<RoomId> {
        self.buildings
            .iter()
            .flat_map(|building| &building.rooms)
            .find(|room| room.objects.iter().any(|object| object.id == id))
            .map(|room| room.id)
    }
    pub(crate) fn crop_object(&self, index: usize) -> Option<&Object> {
        self.objects()
            .filter(|object| matches!(object.kind, ObjectKind::CropPlot { .. }))
            .nth(index)
    }
    pub(crate) fn batch_index(&self, id: ObjectId) -> Option<usize> {
        self.objects()
            .filter(|object| matches!(object.kind, ObjectKind::FermentationVessel { .. }))
            .position(|object| object.id == id)
    }
    pub(crate) fn batch_object(&self, index: usize) -> Option<&Object> {
        self.objects()
            .filter(|object| matches!(object.kind, ObjectKind::FermentationVessel { .. }))
            .nth(index)
    }
}
impl Building {
    fn initial(kind: Location, owner: AgentId) -> Self {
        use ObjectKind::*;
        let kinds = match kind {
            Location::Home => vec![Bed {
                resident: Some(owner),
            }],
            Location::Mill => vec![Millstone],
            Location::Bakery => vec![Oven],
            Location::Weavery => vec![SpinningWheel, Loom],
            Location::Tailory => vec![SewingTable],
            Location::Brewery => std::iter::once(BrewingEquipment)
                .chain(
                    (0..crate::brewing::BREWERY_SLOTS).map(|_| FermentationVessel { batch: None }),
                )
                .collect(),
            _ => Vec::new(),
        };
        let storage = ObjectKind::storage_kit(kind);
        let mut rooms = vec![Room {
            id: RoomId(Uuid::new_v4()),
            name: "Storeroom".into(),
            size: RoomSize::fitting(storage.iter().map(|kind| kind.footprint_m2()).sum())
                .expect("initial storage fits"),
            objects: storage.into_iter().map(Object::new).collect(),
        }];
        if !kinds.is_empty() {
            rooms.insert(
                0,
                Room {
                    id: RoomId(Uuid::new_v4()),
                    name: if kind == Location::Home {
                        "Bedroom"
                    } else {
                        "Workroom"
                    }
                    .into(),
                    size: RoomSize::fitting(kinds.iter().map(|kind| kind.footprint_m2()).sum())
                        .expect("initial equipment fits"),
                    objects: kinds.into_iter().map(Object::new).collect(),
                },
            );
        }
        Self {
            id: BuildingId(Uuid::new_v4()),
            name: kind.name().into(),
            owner,
            rooms,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
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
        map = map
            .with_place(Location::Warehouse, market, None, "Town warehouse")?
            .0;
        Ok(map)
    }
    pub fn random() -> Self {
        let mut map = Self {
            places: HashMap::new(),
            random_positions: true,
        };
        for kind in [Location::Forest, Location::River, Location::Market] {
            map = map
                .with_place(kind, map.next_position(kind), None, kind.name())
                .unwrap()
                .0;
        }
        map = map
            .with_place(
                Location::Warehouse,
                map.next_position(Location::Warehouse),
                None,
                "Town warehouse",
            )
            .unwrap()
            .0;
        map
    }
    pub fn with_warehouse_position(&self, position: Position) -> Result<Self, SimulationError> {
        if !position.valid() {
            return Err(SimulationError::InvalidPosition);
        }
        let mut map = self.clone();
        let warehouse = map
            .places
            .iter_mut()
            .map(|(_, place)| place)
            .find(|place| place.kind.is_town_owned())
            .expect("town warehouse exists");
        warehouse.position = position;
        Ok(map)
    }

    pub(crate) fn set_crop(
        &mut self,
        field: PlaceId,
        plot: usize,
        crop: Option<crate::farming::Crop>,
    ) -> Result<(), SimulationError> {
        let place = self
            .places
            .get_mut(&field)
            .ok_or(SimulationError::PlaceNotFound)?;
        let object = place
            .objects_mut()
            .filter(|object| matches!(object.kind, ObjectKind::CropPlot { .. }))
            .nth(plot)
            .ok_or(SimulationError::MissingProperty)?;
        object.kind = ObjectKind::CropPlot { crop };
        Ok(())
    }

    pub(crate) fn places_mut(&mut self) -> &mut HashMap<PlaceId, Place> {
        &mut self.places
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
    pub fn next_position(&self, kind: Location) -> Position {
        if !self.random_positions {
            return Position::default();
        }
        let mut rng = rand::make_rng::<rand::rngs::SmallRng>();
        loop {
            let position = Position {
                x: rng.random_range(-850.0..=850.0),
                y: rng.random_range(-850.0..=850.0),
            };
            if self.places.values().all(|place| {
                position.distance(place.position)
                    >= kind
                        .minimum_spacing_metres()
                        .max(place.kind.minimum_spacing_metres())
            }) {
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
        if (kind.is_public() || kind.is_town_owned()) != owner.is_none() {
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
                buildings: if matches!(
                    kind,
                    Location::Home
                        | Location::Field
                        | Location::Mill
                        | Location::Bakery
                        | Location::Weavery
                        | Location::Tailory
                        | Location::Brewery
                ) {
                    vec![Building::initial(
                        kind,
                        owner.expect("private building owner"),
                    )]
                } else {
                    Vec::new()
                },
                outdoor_objects: {
                    let mut objects: Vec<_> = match kind {
                        Location::Forest => vec![
                            Object::new(ObjectKind::BerryBushes),
                            Object::new(ObjectKind::Trees),
                        ],
                        Location::River => vec![
                            Object::new(ObjectKind::FishingSpot),
                            Object::new(ObjectKind::WaterSource),
                        ],
                        Location::Field => (0..crate::farming::FIELD_PLOTS)
                            .map(|_| Object::new(ObjectKind::CropPlot { crop: None }))
                            .collect(),
                        _ => Vec::new(),
                    };
                    if kind.is_public() || kind.is_town_owned() {
                        objects.extend(ObjectKind::storage_kit(kind).into_iter().map(Object::new));
                    }
                    objects
                },
            },
        );
        Ok((map, id))
    }
}

impl Map {
    pub(crate) fn validate_save(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        let mut identities = std::collections::HashSet::new();
        for (id, place) in &self.places {
            if *id != place.id
                || !place.position.valid()
                || place.used_area_m2() > PLOT_AREA_LIMIT_M2
                || !identities.insert(id.0)
                || (place.kind.is_public() || place.kind.is_town_owned()) != place.owner.is_none()
                || (place.kind == Location::Field
                    && place.plots().len() != crate::farming::FIELD_PLOTS)
            {
                return Err(SaveError::InvalidState("invalid map place"));
            }
            for building in &place.buildings {
                if !identities.insert(building.id.0)
                    || building.id.0.get_version_num() != 4
                    || building.rooms.is_empty()
                {
                    return Err(SaveError::InvalidState("invalid building"));
                }
                for room in &building.rooms {
                    if !identities.insert(room.id.0)
                        || room.id.0.get_version_num() != 4
                        || room.used_area_m2() > room.size.area_m2()
                        || room.objects.iter().any(|object| {
                            object.kind.unlimited_workers()
                                || matches!(object.kind, ObjectKind::CropPlot { .. })
                        })
                    {
                        return Err(SaveError::InvalidState("invalid room"));
                    }
                }
            }
            for object in place.objects() {
                if !identities.insert(object.id.0)
                    || object.id.0.get_version_num() != 4
                    || (!object.kind.unlimited_workers() && object.workers.len() > 1)
                    || (object.kind.is_storage() && !object.workers.is_empty())
                    || object
                        .workers
                        .iter()
                        .enumerate()
                        .any(|(i, worker)| object.workers[..i].contains(worker))
                    || matches!(object.kind, ObjectKind::FermentationVessel { batch: Some(batch) } if !batch.valid())
                {
                    return Err(SaveError::InvalidState("invalid object"));
                }
            }
            if place.outdoor_objects.iter().any(|object| {
                !object.kind.unlimited_workers()
                    && !matches!(object.kind, ObjectKind::CropPlot { .. })
                    && !object.kind.is_storage()
            }) {
                return Err(SaveError::InvalidState("indoor object outdoors"));
            }
        }
        for kind in [
            Location::Forest,
            Location::River,
            Location::Market,
            Location::Warehouse,
        ] {
            if !self
                .places
                .values()
                .any(|place| place.kind == kind && place.owner.is_none())
            {
                return Err(SaveError::InvalidState("missing public place"));
            }
        }
        Ok(())
    }
}

impl Map {
    pub fn with_building(
        &self,
        place: PlaceId,
        kind: Location,
        owner: AgentId,
        name: impl Into<String>,
    ) -> Result<(Self, BuildingId), SimulationError> {
        if !matches!(
            kind,
            Location::Home
                | Location::Mill
                | Location::Bakery
                | Location::Weavery
                | Location::Tailory
                | Location::Brewery
        ) {
            return Err(SimulationError::MissingProperty);
        }
        let mut map = self.clone();
        let site = map
            .places
            .get_mut(&place)
            .ok_or(SimulationError::PlaceNotFound)?;
        let mut building = Building::initial(kind, owner);
        building.name = name.into();
        let id = building.id;
        let area: u32 = building.rooms.iter().map(|room| room.size.area_m2()).sum();
        if area > site.available_area_m2() {
            return Err(SimulationError::MissingProperty);
        }
        site.buildings.push(building);
        Ok((map, id))
    }
    pub(crate) fn assign_bedroom(
        &mut self,
        home: PlaceId,
        resident: AgentId,
        allowed_owners: &[AgentId],
    ) -> Result<(), SimulationError> {
        if self
            .place(home)?
            .buildings
            .iter()
            .filter(|building| allowed_owners.contains(&building.owner))
            .flat_map(|building| &building.rooms)
            .flat_map(|room| &room.objects)
            .any(|object| {
                object.kind
                    == ObjectKind::Bed {
                        resident: Some(resident),
                    }
            })
        {
            return Ok(());
        }
        let place = self.place(home)?;
        if !place
            .buildings
            .iter()
            .any(|building| allowed_owners.contains(&building.owner))
        {
            return Err(SimulationError::MissingProperty);
        }
        let vacant_bed = place
            .buildings
            .iter()
            .filter(|building| allowed_owners.contains(&building.owner))
            .flat_map(|building| &building.rooms)
            .flat_map(|room| &room.objects)
            .any(|object| object.kind == ObjectKind::Bed { resident: None });
        if !vacant_bed && place.available_area_m2() < RoomSize::Small.area_m2() {
            return Err(SimulationError::MissingProperty);
        }
        for (_, place) in self.places.iter_mut() {
            for object in place.objects_mut() {
                if object.kind
                    == (ObjectKind::Bed {
                        resident: Some(resident),
                    })
                {
                    object.kind = ObjectKind::Bed { resident: None };
                }
            }
        }
        let place = self.places.get_mut(&home).expect("home exists");
        if let Some(bed) = place
            .buildings
            .iter_mut()
            .filter(|building| allowed_owners.contains(&building.owner))
            .flat_map(|building| &mut building.rooms)
            .flat_map(|room| &mut room.objects)
            .find(|object| object.kind == ObjectKind::Bed { resident: None })
        {
            bed.kind = ObjectKind::Bed {
                resident: Some(resident),
            };
            return Ok(());
        }
        place
            .buildings
            .iter_mut()
            .find(|building| allowed_owners.contains(&building.owner))
            .expect("accessible home building exists")
            .rooms
            .push(Room {
                id: RoomId(Uuid::new_v4()),
                name: "Bedroom".into(),
                size: RoomSize::Small,
                objects: vec![Object::new(ObjectKind::Bed {
                    resident: Some(resident),
                })],
            });
        Ok(())
    }
    pub(crate) fn release_objects(
        &mut self,
        site: PlaceId,
        ids: [Option<ObjectId>; 2],
        worker: AgentId,
    ) {
        if ids[0].is_none() {
            return;
        }
        if let Some(place) = self.places.get_mut(&site) {
            for object in place
                .objects_mut()
                .filter(|object| ids.contains(&Some(object.id)))
            {
                object.workers.retain(|id| *id != worker);
            }
        }
    }
    pub(crate) fn claim_objects(
        &mut self,
        place: PlaceId,
        ids: [Option<ObjectId>; 2],
        worker: AgentId,
    ) -> Result<(), SimulationError> {
        let place = self
            .places
            .get_mut(&place)
            .ok_or(SimulationError::PlaceNotFound)?;
        for id in ids.into_iter().flatten() {
            let object = place
                .objects_mut()
                .find(|object| object.id == id)
                .ok_or(SimulationError::MissingProperty)?;
            if !object.available(worker) {
                return Err(SimulationError::MissingInputs);
            }
            if !object.workers.contains(&worker) {
                object.workers.push(worker);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod space_tests {
    use super::*;

    #[test]
    fn initial_rooms_use_smallest_fitting_preset() {
        let owner = AgentId(Uuid::new_v4());
        let mut map = Map::default();
        for kind in [
            Location::Home,
            Location::Field,
            Location::Mill,
            Location::Bakery,
            Location::Weavery,
            Location::Tailory,
            Location::Brewery,
        ] {
            let (next, site) = map
                .with_place(kind, Position::default(), Some(owner), "Test")
                .unwrap();
            map = next;
            for room in &map.place(site).unwrap().buildings[0].rooms {
                assert_eq!(Some(room.size), RoomSize::fitting(room.used_area_m2()));
            }
        }
        map.validate_save().unwrap();
    }

    #[test]
    fn mixed_contents_fit_by_area_and_oversized_contents_fail() {
        let owner = AgentId(Uuid::new_v4());
        let (mut map, site) = Map::default()
            .with_place(Location::Home, Position::default(), Some(owner), "Home")
            .unwrap();
        let room = &mut map.places_mut().get_mut(&site).unwrap().buildings[0].rooms[0];
        room.objects.push(Object::new(ObjectKind::PantryShelving));
        room.objects.push(Object::new(ObjectKind::TextileChest));
        assert_eq!(room.available_area_m2(), 0);
        map.validate_save().unwrap();
        map.places_mut().get_mut(&site).unwrap().buildings[0].rooms[0]
            .objects
            .push(Object::new(ObjectKind::FishCrate));
        assert!(map.validate_save().is_err());
    }

    #[test]
    fn relocation_preserves_shared_room_and_reuses_unassigned_bed() {
        let resident = AgentId(Uuid::new_v4());
        let other = AgentId(Uuid::new_v4());
        let (map, old) = Map::default()
            .with_place(Location::Home, Position::default(), Some(resident), "Old")
            .unwrap();
        let (mut map, new) = map
            .with_place(Location::Home, Position::default(), Some(other), "New")
            .unwrap();
        let old_room = &mut map.places_mut().get_mut(&old).unwrap().buildings[0].rooms[0];
        let room_id = old_room.id;
        old_room.objects.push(Object::new(ObjectKind::TextileChest));
        let new_room = &mut map.places_mut().get_mut(&new).unwrap().buildings[0].rooms[0];
        new_room
            .objects
            .push(Object::new(ObjectKind::Bed { resident: None }));
        map.assign_bedroom(new, resident, &[resident, other])
            .unwrap();
        let old_room = &map.place(old).unwrap().buildings[0].rooms[0];
        assert_eq!(old_room.id, room_id);
        assert_eq!(old_room.objects.len(), 2);
        assert_eq!(old_room.objects[0].kind, ObjectKind::Bed { resident: None });
        assert_eq!(map.place(new).unwrap().buildings[0].rooms.len(), 2);
        assert!(map.place(new).unwrap().objects().any(|object| object.kind
            == ObjectKind::Bed {
                resident: Some(resident)
            }));
        map.validate_save().unwrap();
    }

    #[test]
    fn relocation_uses_only_household_owned_buildings() {
        let resident = AgentId(Uuid::new_v4());
        let foreign = AgentId(Uuid::new_v4());
        let (map, home) = Map::default()
            .with_place(Location::Home, Position::default(), Some(resident), "Home")
            .unwrap();
        let (mut map, _) = map
            .with_building(home, Location::Home, foreign, "Foreign")
            .unwrap();
        let place = map.places_mut().get_mut(&home).unwrap();
        place.buildings[0].rooms[0].objects[0].kind = ObjectKind::Bed { resident: None };
        place.buildings[1].rooms[0].objects[0].kind = ObjectKind::Bed { resident: None };
        place.buildings.swap(0, 1);
        let foreign_rooms = place.buildings[0].rooms.clone();
        map.assign_bedroom(home, resident, &[resident]).unwrap();
        let place = map.place(home).unwrap();
        assert_eq!(place.buildings[0].rooms, foreign_rooms);
        assert_eq!(
            place.buildings[1].rooms[0].objects[0].kind,
            ObjectKind::Bed {
                resident: Some(resident)
            }
        );
        let newcomer = AgentId(Uuid::new_v4());
        map.assign_bedroom(home, newcomer, &[resident, newcomer])
            .unwrap();
        let place = map.place(home).unwrap();
        assert_eq!(place.buildings[0].rooms, foreign_rooms);
        assert_eq!(place.buildings[1].rooms.len(), 3);
        map.validate_save().unwrap();
    }

    #[test]
    fn plot_limit_rejects_additional_building_without_changing_map() {
        let owner = AgentId(Uuid::new_v4());
        let (mut map, site) = Map::default()
            .with_place(Location::Home, Position::default(), Some(owner), "Home")
            .unwrap();
        for _ in 0..9 {
            map = map
                .with_building(site, Location::Home, owner, "Home")
                .unwrap()
                .0;
        }
        assert_eq!(map.place(site).unwrap().available_area_m2(), 0);
        assert!(
            map.with_building(site, Location::Home, owner, "Extra")
                .is_err()
        );
        map.validate_save().unwrap();
        let place = map.places_mut().get_mut(&site).unwrap();
        place.buildings[0].rooms[0].size = RoomSize::Medium;
        assert!(map.validate_save().is_err());
    }
}
