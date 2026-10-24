use crate::{
    AgentId, AgentKind, CitizenAction, Coins, Quantity, SimulationError, Universe,
    locations::{BuildingId, ObjectId, ObjectKind, PlaceId, RoomId, RoomSize},
    marketplace::{Good, ShoppingList},
    production::{Skill, skill_duration_reduction},
};
use imbl::HashMap;
use rand::{RngExt, rngs::Xoshiro256PlusPlus};
use uuid::Uuid;

pub const HOUR_MS: u64 = 3_600_000;
pub const DAY_MS: u64 = 24 * HOUR_MS;
pub const MILESTONE_BASE_MS: u64 = 12 * HOUR_MS;
pub const CAPACITY_BASE_MS: u64 = 24 * HOUR_MS;
pub const BIDDING_MS: u64 = DAY_MS;
pub const DEADLINE_MS: u64 = 30 * DAY_MS;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProjectId(pub Uuid);
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContractId(pub Uuid);

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionItem {
    Room(RoomSize),
    Bed,
    PantryShelving,
    DryGoodsBin,
    FishCrate,
    WoodRack,
    TextileChest,
    LiquidBarrel,
    Millstone,
    Oven,
    SpinningWheel,
    Loom,
    SewingTable,
    BrewingEquipment,
    FermentationVessel,
}

impl ConstructionItem {
    pub const OBJECTS: [Self; 14] = [
        Self::Bed,
        Self::PantryShelving,
        Self::DryGoodsBin,
        Self::FishCrate,
        Self::WoodRack,
        Self::TextileChest,
        Self::LiquidBarrel,
        Self::Millstone,
        Self::Oven,
        Self::SpinningWheel,
        Self::Loom,
        Self::SewingTable,
        Self::BrewingEquipment,
        Self::FermentationVessel,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Room(RoomSize::Small) => "Small room",
            Self::Room(RoomSize::Medium) => "Medium room",
            Self::Room(RoomSize::Large) => "Large room",
            _ => self.object_kind().unwrap().name(),
        }
    }
    pub fn base_hours(self) -> u64 {
        match self {
            Self::Room(RoomSize::Small) | Self::SpinningWheel => 30,
            Self::Room(RoomSize::Medium) => 60,
            Self::Room(RoomSize::Large) => 120,
            Self::Bed | Self::LiquidBarrel | Self::SewingTable | Self::FermentationVessel => 20,
            Self::PantryShelving | Self::DryGoodsBin | Self::WoodRack => 10,
            Self::FishCrate => 5,
            Self::TextileChest => 15,
            Self::Millstone | Self::Oven | Self::Loom => 40,
            Self::BrewingEquipment => 50,
        }
    }
    pub fn wood(self) -> Quantity {
        self.base_hours() * 200
    }
    pub fn area_m2(self) -> u32 {
        match self {
            Self::Room(size) => size.area_m2(),
            _ => self.object_kind().unwrap().footprint_m2(),
        }
    }
    pub fn object_kind(self) -> Option<ObjectKind> {
        Some(match self {
            Self::Room(_) => return None,
            Self::Bed => ObjectKind::Bed { resident: None },
            Self::PantryShelving => ObjectKind::PantryShelving,
            Self::DryGoodsBin => ObjectKind::DryGoodsBin,
            Self::FishCrate => ObjectKind::FishCrate,
            Self::WoodRack => ObjectKind::WoodRack,
            Self::TextileChest => ObjectKind::TextileChest,
            Self::LiquidBarrel => ObjectKind::LiquidBarrel,
            Self::Millstone => ObjectKind::Millstone,
            Self::Oven => ObjectKind::Oven,
            Self::SpinningWheel => ObjectKind::SpinningWheel,
            Self::Loom => ObjectKind::Loom,
            Self::SewingTable => ObjectKind::SewingTable,
            Self::BrewingEquipment => ObjectKind::BrewingEquipment,
            Self::FermentationVessel => ObjectKind::FermentationVessel { batch: None },
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum ConstructionOutput {
    Room(RoomId),
    Object(ObjectId),
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ConstructionStage {
    pub next_contract: ContractId,
    pub base_ms: u64,
    pub completed_base_ms: u64,
    pub installed_wood: Quantity,
}
impl ConstructionStage {
    pub fn remaining_base_ms(&self) -> u64 {
        self.base_ms - self.completed_base_ms
    }
    pub fn missing_wood(&self) -> Quantity {
        wood_through(self.base_ms).saturating_sub(self.installed_wood)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ConstructionProject {
    pub id: ProjectId,
    pub client: AgentId,
    pub place: PlaceId,
    pub building: BuildingId,
    pub room: Option<RoomId>,
    pub item: ConstructionItem,
    pub completed_base_ms: u64,
    pub installed_wood: Quantity,
    pub completed: bool,
    pub output: ConstructionOutput,
    pub stages: Vec<ConstructionStage>,
    pub escrow: Coins,
    pub funding_base_ms: u64,
    pub funding_offer: Coins,
    pub allocated_base_ms: u64,
    pub allocated_offer: Coins,
    pub scheduling: bool,
}
impl ConstructionProject {
    pub fn remaining_base_ms(&self) -> u64 {
        self.item.base_hours() * HOUR_MS - self.completed_base_ms
    }
    pub fn missing_wood(&self) -> Quantity {
        self.item.wood() - self.installed_wood
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractStatus {
    Bidding,
    Active,
    Completed,
    NoBids,
    Cancelled,
    Failed,
}
impl ContractStatus {
    pub fn open(self) -> bool {
        matches!(self, Self::Bidding | Self::Active)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ConstructionQuote {
    pub carpenter: AgentId,
    pub price: Coins,
    pub financing: Coins,
    pub procurement_cash: Coins,
    pub shared_wood: Quantity,
    pub carried_wood: Quantity,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ConstructionContract {
    pub id: ContractId,
    pub project: ProjectId,
    pub stage: usize,
    pub started: bool,
    pub posted_ms: u64,
    pub sequence: u64,
    pub bidding_ends_ms: u64,
    pub next_quote_ms: u64,
    pub maximum_offer: Coins,
    pub escrow: Coins,
    pub status: ContractStatus,
    pub quotes: Vec<ConstructionQuote>,
    pub carpenter: Option<AgentId>,
    pub awarded_ms: Option<u64>,
    pub deadline_ms: Option<u64>,
    pub price: Coins,
    pub paid: Coins,
    pub start_base_ms: u64,
    pub paid_base_ms: u64,
    pub ended_base_ms: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConstructionCash {
    pub escrow_deposited: Coins,
    pub escrow_refunded: Coins,
    pub payments_received: Coins,
    pub payments_paid: Coins,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ConstructionPayment {
    pub contract: ContractId,
    pub time_ms: u64,
    pub client: AgentId,
    pub carpenter: AgentId,
    pub amount: Coins,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Construction {
    pub projects: HashMap<ProjectId, ConstructionProject>,
    pub contracts: HashMap<ContractId, ConstructionContract>,
    pub payments: Vec<ConstructionPayment>,
    rng: Xoshiro256PlusPlus,
}
impl Default for Construction {
    fn default() -> Self {
        Self {
            projects: HashMap::new(),
            contracts: HashMap::new(),
            payments: Vec::new(),
            rng: rand::make_rng(),
        }
    }
}

fn wood_through(base_ms: u64) -> Quantity {
    (u128::from(base_ms) * 200).div_ceil(u128::from(HOUR_MS)) as Quantity
}
fn add_coins(to: &mut Coins, amount: Coins) -> Result<(), SimulationError> {
    *to = to
        .checked_add(amount)
        .ok_or(SimulationError::WealthOverflow)?;
    Ok(())
}

impl Universe {
    pub fn construction(&self) -> &Construction {
        &self.construction
    }

    pub fn construction_budget(
        &self,
        client: AgentId,
    ) -> Result<crate::construction_finance::HouseholdBudget, SimulationError> {
        let account = self.economic_id(client)?;
        self.household_budget(account, self.construction_commitments(account, None)?)
    }

    fn construction_cash(
        &mut self,
        account: AgentId,
        change: ConstructionCash,
    ) -> Result<(), SimulationError> {
        let cash = if let Some(history) = self.household_histories.get_mut(&account) {
            &mut history.current.construction
        } else {
            &mut self
                .citizen_histories
                .get_mut(&account)
                .ok_or(SimulationError::AgentNotFound)?
                .current
                .construction
        };
        add_coins(&mut cash.escrow_deposited, change.escrow_deposited)?;
        add_coins(&mut cash.escrow_refunded, change.escrow_refunded)?;
        add_coins(&mut cash.payments_received, change.payments_received)?;
        add_coins(&mut cash.payments_paid, change.payments_paid)
    }

    pub fn construction_reserved_area(&self, place: PlaceId, room: Option<RoomId>) -> u32 {
        self.construction
            .projects
            .values()
            .filter(|project| {
                !project.completed
                    && project.place == place
                    && project.room == room
                    && self.construction.contracts.values().any(|contract| {
                        contract.project == project.id
                            && (contract.status.open()
                                || project.completed_base_ms > 0
                                || project.installed_wood > 0)
                    })
            })
            .map(|project| project.item.area_m2())
            .sum()
    }

    pub fn validate_construction_destination(
        &self,
        client: AgentId,
        place: PlaceId,
        building: BuildingId,
        room: Option<RoomId>,
        item: ConstructionItem,
    ) -> Result<(), SimulationError> {
        let account = self.economic_id(client)?;
        let site = self.map.place(place)?;
        let building = site
            .buildings
            .iter()
            .find(|candidate| candidate.id == building)
            .ok_or(SimulationError::MissingProperty)?;
        if self.economic_id(building.owner)? != account {
            return Err(SimulationError::PrivateProperty);
        }
        match item {
            ConstructionItem::Room(_) => {
                if room.is_some()
                    || site
                        .owner
                        .map(|owner| self.economic_id(owner))
                        .transpose()?
                        != Some(account)
                {
                    return Err(SimulationError::PrivateProperty);
                }
                if item.area_m2()
                    > site
                        .available_area_m2()
                        .saturating_sub(self.construction_reserved_area(place, None))
                {
                    return Err(SimulationError::MissingProperty);
                }
            }
            _ => {
                let room = building
                    .rooms
                    .iter()
                    .find(|candidate| Some(candidate.id) == room)
                    .ok_or(SimulationError::MissingProperty)?;
                if item.area_m2()
                    > room
                        .available_area_m2()
                        .saturating_sub(self.construction_reserved_area(place, Some(room.id)))
                {
                    return Err(SimulationError::MissingProperty);
                }
            }
        }
        Ok(())
    }

    pub fn request_construction(
        &self,
        client: AgentId,
        place: PlaceId,
        building: BuildingId,
        room: Option<RoomId>,
        item: ConstructionItem,
        maximum_offer: Coins,
    ) -> Result<(Self, ContractId), SimulationError> {
        let account = self.economic_id(client)?;
        if let Some(project) = self.construction.projects.values().find(|project| {
            !project.completed
                && project.completed_base_ms == 0
                && project.installed_wood == 0
                && self.economic_id(project.client).ok() == Some(account)
                && project.place == place
                && project.building == building
                && project.room == room
                && project.item == item
                && !self
                    .construction
                    .contracts
                    .values()
                    .any(|contract| contract.project == project.id && contract.status.open())
        }) {
            return self.continue_construction(client, project.id, maximum_offer);
        }
        self.validate_construction_destination(client, place, building, room, item)?;
        let mut world = self.clone();
        let client = world.economic_id(client)?;
        let id = ProjectId(Uuid::new_v4());
        world.construction.projects.insert(
            id,
            ConstructionProject {
                id,
                client,
                place,
                building,
                room,
                item,
                completed_base_ms: 0,
                installed_wood: 0,
                completed: false,
                stages: (0..(item.base_hours() * HOUR_MS).div_ceil(MILESTONE_BASE_MS))
                    .map(|index| ConstructionStage {
                        next_contract: ContractId(Uuid::new_v4()),
                        base_ms: (item.base_hours() * HOUR_MS - index * MILESTONE_BASE_MS)
                            .min(MILESTONE_BASE_MS),
                        completed_base_ms: 0,
                        installed_wood: 0,
                    })
                    .collect(),
                escrow: 0,
                funding_base_ms: 0,
                funding_offer: 0,
                allocated_base_ms: 0,
                allocated_offer: 0,
                scheduling: false,
                output: if matches!(item, ConstructionItem::Room(_)) {
                    ConstructionOutput::Room(RoomId(Uuid::new_v4()))
                } else {
                    ConstructionOutput::Object(ObjectId(Uuid::new_v4()))
                },
            },
        );
        let contract = world.post_construction(id, maximum_offer)?;
        Ok((world, contract))
    }

    pub fn validate_construction_continuation(
        &self,
        client: AgentId,
        project: ProjectId,
    ) -> Result<(), SimulationError> {
        let site = self
            .construction
            .projects
            .get(&project)
            .ok_or(SimulationError::MissingProperty)?;
        if self.economic_id(client)? != self.economic_id(site.client)? {
            return Err(SimulationError::PrivateProperty);
        }
        if site.completed
            || self
                .construction
                .contracts
                .values()
                .any(|contract| contract.project == project && contract.status.open())
        {
            return Err(SimulationError::CitizenBusy);
        }
        let mut available = self.clone();
        available.construction.projects.remove(&project);
        available
            .construction
            .contracts
            .retain(|_, contract| contract.project != project);
        available.validate_construction_destination(
            client,
            site.place,
            site.building,
            site.room,
            site.item,
        )?;
        Ok(())
    }

    pub fn continue_construction(
        &self,
        client: AgentId,
        project: ProjectId,
        maximum_offer: Coins,
    ) -> Result<(Self, ContractId), SimulationError> {
        self.validate_construction_continuation(client, project)?;
        let previous = self
            .construction
            .contracts
            .values()
            .filter(|contract| contract.project == project)
            .max_by_key(|contract| contract.sequence);
        if previous.is_some_and(|contract| contract.status == ContractStatus::NoBids)
            && maximum_offer <= self.construction.projects[&project].funding_offer
        {
            return Err(SimulationError::InvalidQuantity);
        }
        let mut world = self.clone();
        let contract = world.post_construction(project, maximum_offer)?;
        Ok((world, contract))
    }

    fn post_construction(
        &mut self,
        project: ProjectId,
        maximum_offer: Coins,
    ) -> Result<ContractId, SimulationError> {
        let client = self.economic_id(self.construction.projects[&project].client)?;
        if maximum_offer <= 0 || maximum_offer > self.construction_budget(client)?.maximum_offer {
            return Err(SimulationError::InvalidQuantity);
        }
        self.set_account_coins(client, self.account_coins(client)? - maximum_offer)?;
        self.construction_cash(
            client,
            ConstructionCash {
                escrow_deposited: maximum_offer,
                ..Default::default()
            },
        )?;
        let site = self.construction.projects.get_mut(&project).unwrap();
        site.client = client;
        site.escrow = maximum_offer;
        site.funding_offer = maximum_offer;
        site.funding_base_ms = site.remaining_base_ms();
        site.allocated_base_ms = 0;
        site.allocated_offer = 0;
        site.scheduling = true;
        for stage in &mut site.stages {
            if self
                .construction
                .contracts
                .contains_key(&stage.next_contract)
            {
                stage.next_contract = ContractId(Uuid::new_v4());
            }
        }
        let stage = site
            .stages
            .iter()
            .position(|stage| stage.remaining_base_ms() > 0)
            .unwrap();
        let id = self.post_construction_stage(project, stage)?;
        self.process_construction()?;
        self.refresh_market();
        self.refresh_household_history(self.current_time_ms, false)?;
        Ok(id)
    }

    fn post_construction_stage(
        &mut self,
        project: ProjectId,
        stage: usize,
    ) -> Result<ContractId, SimulationError> {
        let site = self.construction.projects.get_mut(&project).unwrap();
        let part = &site.stages[stage];
        let start_base_ms = part.completed_base_ms;
        let base_ms = part.remaining_base_ms();
        let allocated_base_ms = site.allocated_base_ms + base_ms;
        let allocated_offer = (i128::from(site.funding_offer) * i128::from(allocated_base_ms)
            / i128::from(site.funding_base_ms)) as Coins;
        let maximum_offer = allocated_offer - site.allocated_offer;
        site.allocated_base_ms = allocated_base_ms;
        site.allocated_offer = allocated_offer;
        site.escrow -= maximum_offer;
        let id = part.next_contract;
        let bidding_ends_ms = self
            .current_time_ms
            .checked_add(BIDDING_MS)
            .ok_or(SimulationError::TimeOverflow)?;
        self.construction.contracts.insert(
            id,
            ConstructionContract {
                id,
                project,
                stage,
                started: false,
                posted_ms: self.current_time_ms,
                sequence: self.construction.contracts.len() as u64,
                bidding_ends_ms,
                next_quote_ms: self.current_time_ms,
                maximum_offer,
                escrow: maximum_offer,
                status: ContractStatus::Bidding,
                quotes: Vec::new(),
                carpenter: None,
                awarded_ms: None,
                deadline_ms: None,
                price: 0,
                paid: 0,
                start_base_ms,
                paid_base_ms: start_base_ms,
                ended_base_ms: None,
            },
        );
        Ok(id)
    }

    fn open_next_construction_stage(
        &mut self,
        project: ProjectId,
        current: usize,
    ) -> Result<(), SimulationError> {
        if !self.construction.projects[&project].scheduling {
            return Ok(());
        }
        let next = self.construction.projects[&project]
            .stages
            .iter()
            .enumerate()
            .skip(current + 1)
            .find_map(|(index, stage)| {
                (stage.remaining_base_ms() > 0
                    && !self.construction.contracts.values().any(|contract| {
                        contract.project == project
                            && contract.stage == index
                            && contract.status.open()
                    }))
                .then_some(index)
            });
        if let Some(stage) = next {
            self.post_construction_stage(project, stage)?;
        }
        Ok(())
    }

    pub fn cancel_construction(
        &self,
        client: AgentId,
        id: ContractId,
    ) -> Result<Self, SimulationError> {
        let contract = self
            .construction
            .contracts
            .get(&id)
            .ok_or(SimulationError::MissingProperty)?;
        if self.construction.projects[&contract.project].client != self.economic_id(client)? {
            return Err(SimulationError::PrivateProperty);
        }
        if contract.status != ContractStatus::Bidding {
            return Err(SimulationError::CitizenBusy);
        }
        let mut world = self.clone();
        world.close_construction(id, ContractStatus::Cancelled)?;
        world.refresh_construction_context()?;
        world.refresh_market();
        world.refresh_household_history(world.current_time_ms, false)?;
        Ok(world)
    }

    pub(crate) fn construction_commitments(
        &self,
        account: AgentId,
        exclude: Option<AgentId>,
    ) -> Result<Coins, SimulationError> {
        let mut total: Coins = 0;
        for contract in self
            .construction
            .contracts
            .values()
            .filter(|contract| contract.status.open())
        {
            for quote in &contract.quotes {
                if Some(quote.carpenter) != exclude
                    && self.economic_id(quote.carpenter)? == account
                    && (contract.status == ContractStatus::Bidding
                        || contract.carpenter == Some(quote.carpenter))
                {
                    add_coins(&mut total, quote.financing)?;
                }
            }
        }
        Ok(total)
    }

    fn other_construction_commitments(
        &self,
        account: AgentId,
        excluded: ContractId,
    ) -> Result<Coins, SimulationError> {
        let mut amount = 0;
        for contract in self
            .construction
            .contracts
            .values()
            .filter(|contract| contract.status.open() && contract.id != excluded)
        {
            for quote in contract.quotes.iter().filter(|quote| {
                contract.status == ContractStatus::Bidding
                    || contract.carpenter == Some(quote.carpenter)
            }) {
                if self.economic_id(quote.carpenter)? == account {
                    add_coins(&mut amount, quote.financing)?;
                }
            }
        }
        Ok(amount)
    }

    pub fn carpenter_committed_base_ms(&self, id: AgentId) -> u64 {
        self.construction
            .contracts
            .values()
            .filter(|contract| {
                contract.status.open()
                    && (contract.carpenter == Some(id)
                        || (contract.status == ContractStatus::Bidding
                            && contract.quotes.iter().any(|quote| quote.carpenter == id)))
            })
            .map(|contract| {
                self.construction.projects[&contract.project].stages[contract.stage]
                    .remaining_base_ms()
            })
            .sum()
    }

    pub fn carpenter_contract(&self, worker: AgentId) -> Option<&ConstructionContract> {
        let AgentKind::Citizen(citizen) = &self.agents.get(&worker)?.kind;
        let current = citizen
            .active_action()
            .and_then(|active| match active.action() {
                CitizenAction::Construct { contract, .. } => Some(contract),
                _ => None,
            });
        self.construction
            .contracts
            .values()
            .filter(|contract| {
                contract.status == ContractStatus::Active && contract.carpenter == Some(worker)
            })
            .min_by_key(|contract| {
                let stage = &self.construction.projects[&contract.project].stages[contract.stage];
                (
                    Some(contract.id) != current,
                    stage.missing_wood() > citizen.carried_available_units(Good::Wood),
                    contract.awarded_ms,
                    contract.sequence,
                )
            })
    }

    fn construction_quote(
        &self,
        id: AgentId,
        contract: &ConstructionContract,
        sealed_price: Option<Coins>,
    ) -> Result<Option<ConstructionQuote>, SimulationError> {
        let AgentKind::Citizen(worker) = &self.agents[&id].kind;
        let Some(practice) = worker.skill_practice_ms(Skill::Carpentry) else {
            return Ok(None);
        };
        let project = &self.construction.projects[&contract.project];
        if self.carpenter_committed_base_ms(id) > CAPACITY_BASE_MS {
            return Ok(None);
        }
        let factor = 1.0 - skill_duration_reduction(practice);
        let stage = &project.stages[contract.stage];
        let hours = stage.remaining_base_ms() as f64 / HOUR_MS as f64 * factor;
        let account = worker.economic_id();
        let commitments = if sealed_price.is_some() {
            self.other_construction_commitments(account, contract.id)?
        } else {
            self.construction_commitments(account, None)?
        };
        let budget = self.household_budget(account, commitments)?;
        let first_base = stage.remaining_base_ms();
        let first_wood = stage.missing_wood();
        let carried = worker.carried_available_units(Good::Wood);
        let shared = worker
            .usable_owned_units(Good::Wood)
            .saturating_sub(carried);
        let allocated = self
            .construction
            .contracts
            .values()
            .filter(|candidate| {
                candidate.status.open() && (sealed_price.is_none() || candidate.id != contract.id)
            })
            .flat_map(|contract| {
                contract.quotes.iter().filter(move |quote| {
                    contract.status == ContractStatus::Bidding
                        || contract.carpenter == Some(quote.carpenter)
                })
            })
            .filter(|quote| self.economic_id(quote.carpenter).ok() == Some(account))
            .fold(0u64, |total, quote| total.saturating_add(quote.shared_wood));
        let allocated_carried = self
            .construction
            .contracts
            .values()
            .filter(|other| other.status.open() && other.id != contract.id)
            .flat_map(|other| {
                other.quotes.iter().filter(move |quote| {
                    other.status == ContractStatus::Bidding
                        || other.carpenter == Some(quote.carpenter)
                })
            })
            .filter(|quote| quote.carpenter == id)
            .fold(0u64, |sum, quote| sum.saturating_add(quote.carried_wood));
        let carried_wood = first_wood.min(carried.saturating_sub(allocated_carried));
        let shared_wood = first_wood
            .saturating_sub(carried_wood)
            .min(shared.saturating_sub(allocated));
        let owned = carried_wood.saturating_add(shared_wood);
        let travel_hours = worker.position().distance(self.map.position(project.place))
            * crate::locations::WALK_MS_PER_METRE
            / HOUR_MS as f64;
        let procurement_hours = if stage.missing_wood() > carried_wood {
            2.0
        } else {
            0.0
        } + travel_hours;
        // Forecast eight earning hours daily, plus one day for supply uncertainty.
        if (hours + procurement_hours) / 8.0 + 1.0 > 30.0 {
            return Ok(None);
        }
        let price = if let Some(price) = sealed_price {
            price
        } else {
            self.carpenter_quote(
                &budget,
                &[(Good::Wood, stage.missing_wood())],
                hours,
                procurement_hours,
            )?
            .payment
            .max(1)
        };
        if price > contract.maximum_offer {
            return Ok(None);
        }
        let procurement_cash = (first_wood.saturating_sub(owned) as f64
            * self.construction_estimated_unit_price(account, Good::Wood))
        .ceil() as Coins;
        let finance = budget.construction_financing(
            procurement_cash,
            (first_base as f64 / HOUR_MS as f64 * factor + procurement_hours) / 8.0 + 1.0,
        )?;
        Ok(finance.can_finance.then_some(ConstructionQuote {
            carpenter: id,
            price,
            financing: finance.required_cash,
            procurement_cash: finance.procurement_cash,
            shared_wood,
            carried_wood,
        }))
    }

    pub(crate) fn construction_boundary(&self) -> Option<u64> {
        self.construction
            .contracts
            .values()
            .filter_map(|contract| match contract.status {
                ContractStatus::Bidding => {
                    Some(contract.next_quote_ms.min(contract.bidding_ends_ms))
                }
                ContractStatus::Active => contract.deadline_ms,
                _ => None,
            })
            .filter(|time| *time > self.current_time_ms)
            .min()
            .map(|time| time - self.current_time_ms)
    }

    pub(crate) fn process_construction(&mut self) -> Result<(), SimulationError> {
        let mut contracts: Vec<_> = self.construction.contracts.keys().copied().collect();
        contracts.sort_by_key(|id| (self.construction.contracts[id].posted_ms, id.0));
        let mut workers: Vec<_> = self.agents.keys().copied().collect();
        workers.sort_by_key(|id| id.0);
        for id in contracts {
            let status = self.construction.contracts[&id].status;
            if status == ContractStatus::Active
                && self.construction.contracts[&id]
                    .deadline_ms
                    .is_some_and(|deadline| deadline <= self.current_time_ms)
            {
                self.close_construction(id, ContractStatus::Failed)?;
            } else if status == ContractStatus::Bidding {
                if self.current_time_ms < self.construction.contracts[&id].bidding_ends_ms {
                    if self.current_time_ms < self.construction.contracts[&id].next_quote_ms {
                        continue;
                    }
                    let posted = self.construction.contracts[&id].posted_ms;
                    let next = posted
                        .checked_add(((self.current_time_ms - posted) / HOUR_MS + 1) * HOUR_MS)
                        .ok_or(SimulationError::TimeOverflow)?;
                    self.construction
                        .contracts
                        .get_mut(&id)
                        .unwrap()
                        .next_quote_ms = next.min(self.construction.contracts[&id].bidding_ends_ms);
                    for worker in &workers {
                        if !self.construction.contracts[&id]
                            .quotes
                            .iter()
                            .any(|quote| quote.carpenter == *worker)
                            && self.carpenter_committed_base_ms(*worker).saturating_add(
                                self.construction.projects
                                    [&self.construction.contracts[&id].project]
                                    .stages[self.construction.contracts[&id].stage]
                                    .remaining_base_ms(),
                            ) <= CAPACITY_BASE_MS
                            && let Some(quote) = self.construction_quote(
                                *worker,
                                &self.construction.contracts[&id],
                                None,
                            )?
                        {
                            self.construction
                                .contracts
                                .get_mut(&id)
                                .unwrap()
                                .quotes
                                .push(quote);
                        }
                    }
                } else {
                    self.award_construction(id)?;
                }
            }
        }
        self.refresh_construction_context()
    }

    fn award_construction(&mut self, id: ContractId) -> Result<(), SimulationError> {
        let snapshot = self.construction.contracts[&id].clone();
        let client = self.construction.projects[&snapshot.project].client;
        let mut valid = Vec::new();
        for quote in &snapshot.quotes {
            if let Some(current) =
                self.construction_quote(quote.carpenter, &snapshot, Some(quote.price))?
            {
                valid.push(current);
            }
        }
        let Some(lowest) = valid.iter().map(|quote| quote.price).min() else {
            return self.close_construction(id, ContractStatus::NoBids);
        };
        valid.retain(|quote| quote.price == lowest);
        let winner = valid.swap_remove(self.construction.rng.random_range(0..valid.len()));
        let mut commitments = self.other_construction_commitments(client, id)?;
        if self.economic_id(winner.carpenter)? == client {
            add_coins(&mut commitments, winner.financing)?;
        }
        let budget = self.household_budget(client, commitments)?;
        if i128::from(budget.cash) + i128::from(snapshot.maximum_offer - winner.price)
            < i128::from(budget.essential_reserve)
                + i128::from(budget.liabilities)
                + i128::from(budget.commitments)
        {
            return self.close_construction(id, ContractStatus::Cancelled);
        }
        let refund = snapshot.escrow - winner.price;
        self.set_account_coins(
            client,
            self.account_coins(client)?
                .checked_add(refund)
                .ok_or(SimulationError::WealthOverflow)?,
        )?;
        self.construction_cash(
            client,
            ConstructionCash {
                escrow_refunded: refund,
                ..Default::default()
            },
        )?;
        let contract = self.construction.contracts.get_mut(&id).unwrap();
        if let Some(quote) = contract
            .quotes
            .iter_mut()
            .find(|quote| quote.carpenter == winner.carpenter)
        {
            *quote = winner.clone();
        }
        contract.status = ContractStatus::Active;
        contract.carpenter = Some(winner.carpenter);
        contract.awarded_ms = Some(self.current_time_ms);
        contract.deadline_ms = Some(
            self.current_time_ms
                .checked_add(DEADLINE_MS)
                .ok_or(SimulationError::TimeOverflow)?,
        );
        contract.price = winner.price;
        contract.escrow = winner.price;
        Ok(())
    }

    fn close_construction(
        &mut self,
        id: ContractId,
        status: ContractStatus,
    ) -> Result<(), SimulationError> {
        let snapshot = self.construction.contracts[&id].clone();
        let client = self.construction.projects[&snapshot.project].client;
        let unallocated = self.construction.projects[&snapshot.project].escrow;
        let project = self
            .construction
            .projects
            .get_mut(&snapshot.project)
            .unwrap();
        project.escrow = 0;
        project.scheduling = false;
        self.set_account_coins(
            client,
            self.account_coins(client)?
                .checked_add(unallocated)
                .ok_or(SimulationError::WealthOverflow)?,
        )?;
        self.construction_cash(
            client,
            ConstructionCash {
                escrow_refunded: unallocated,
                ..Default::default()
            },
        )?;
        self.set_account_coins(
            client,
            self.account_coins(client)?
                .checked_add(snapshot.escrow)
                .ok_or(SimulationError::WealthOverflow)?,
        )?;
        self.construction_cash(
            client,
            ConstructionCash {
                escrow_refunded: snapshot.escrow,
                ..Default::default()
            },
        )?;
        let contract = self.construction.contracts.get_mut(&id).unwrap();
        contract.status = status;
        contract.ended_base_ms = Some(
            self.construction.projects[&snapshot.project].stages[snapshot.stage].completed_base_ms,
        );
        contract.escrow = 0;
        if let Some(worker) = snapshot.carpenter {
            let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&worker).unwrap().kind;
            let place = self.construction.projects[&snapshot.project].place;
            if citizen.active_action.is_some_and(|active| {
                matches!(active.action(), CitizenAction::Construct { contract, .. } if contract == id)
                    || (active.action() == CitizenAction::Travel(place) && self.map.place(place).is_ok_and(|site| !citizen.may_use_place(site)))
            }) { citizen.active_action = None; }
        }
        Ok(())
    }

    fn outstanding_construction_procurement(
        &self,
        contract: &ConstructionContract,
        quote: &ConstructionQuote,
    ) -> Result<Coins, SimulationError> {
        let project = &self.construction.projects[&contract.project];
        let stage = &project.stages[contract.stage];
        let required = stage.missing_wood();
        let AgentKind::Citizen(worker) = &self.agents[&quote.carpenter].kind;
        let mut worker = worker.clone();
        worker.storage = self.storage.clone();
        worker.market = self.market.clone();
        let account = worker.economic_id();
        let others = self
            .construction
            .contracts
            .values()
            .filter(|other| other.status.open() && other.id != contract.id)
            .flat_map(|contract| {
                contract.quotes.iter().filter(move |quote| {
                    contract.status == ContractStatus::Bidding
                        || contract.carpenter == Some(quote.carpenter)
                })
            })
            .filter(|other| self.economic_id(other.carpenter).ok() == Some(account))
            .fold(0u64, |total, other| total.saturating_add(other.shared_wood));
        let carried = worker.carried_available_units(Good::Wood);
        let allocated_carried = self
            .construction
            .contracts
            .values()
            .filter(|other| other.status.open() && other.id != contract.id)
            .flat_map(|other| {
                other.quotes.iter().filter(move |quote| {
                    other.status == ContractStatus::Bidding
                        || other.carpenter == Some(quote.carpenter)
                })
            })
            .filter(|other| other.carpenter == quote.carpenter)
            .fold(0u64, |sum, quote| sum.saturating_add(quote.carried_wood));
        let shared = worker
            .usable_owned_units(Good::Wood)
            .saturating_sub(carried)
            .saturating_sub(others);
        let owned = shared.saturating_add(carried.saturating_sub(allocated_carried));
        Ok((required.saturating_sub(owned) as f64
            * self.construction_estimated_unit_price(account, Good::Wood))
        .ceil() as Coins)
    }

    pub(crate) fn refresh_construction_context(&mut self) -> Result<(), SimulationError> {
        let mut ids: Vec<_> = self.agents.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            let account = self.economic_id(id)?;
            let escrow = self
                .construction
                .contracts
                .values()
                .filter(|contract| self.construction.projects[&contract.project].client == account)
                .try_fold(0i64, |sum, contract| {
                    sum.checked_add(contract.escrow)
                        .ok_or(SimulationError::WealthOverflow)
                })?;
            let escrow = self
                .construction
                .projects
                .values()
                .filter(|project| project.client == account)
                .try_fold(escrow, |sum, project| {
                    sum.checked_add(project.escrow)
                        .ok_or(SimulationError::WealthOverflow)
                })?;
            let current_contract = self.carpenter_contract(id).map(|contract| contract.id);
            let mut protected = 0i64;
            for contract in self
                .construction
                .contracts
                .values()
                .filter(|contract| contract.status.open())
            {
                for quote in contract.quotes.iter().filter(|quote| {
                    contract.status == ContractStatus::Bidding
                        || contract.carpenter == Some(quote.carpenter)
                }) {
                    if Some(contract.id) != current_contract
                        && self.economic_id(quote.carpenter)? == account
                    {
                        add_coins(
                            &mut protected,
                            self.outstanding_construction_procurement(contract, quote)?,
                        )?;
                    }
                }
            }
            let access = self
                .carpenter_contract(id)
                .map(|contract| self.construction.projects[&contract.project].place);
            let wood_reserve = self
                .construction
                .contracts
                .values()
                .filter(|contract| {
                    (contract.status == ContractStatus::Active
                        && contract
                            .carpenter
                            .and_then(|worker| self.economic_id(worker).ok())
                            == Some(account))
                        || (contract.status == ContractStatus::Bidding
                            && contract.quotes.iter().any(|quote| {
                                self.economic_id(quote.carpenter).ok() == Some(account)
                            }))
                })
                .try_fold(0u64, |total, contract| {
                    total
                        .checked_add(
                            self.construction.projects[&contract.project].stages[contract.stage]
                                .missing_wood(),
                        )
                        .ok_or(SimulationError::InventoryOverflow)
                })?;
            let coins = self.account_coins(account)?;
            let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&id).unwrap().kind;
            citizen.construction_escrow = escrow;
            citizen.construction_protected = protected;
            citizen.construction_access = access;
            citizen.construction_wood_reserve = wood_reserve;
            citizen.coins = coins;
        }
        Ok(())
    }

    pub(crate) fn advance_construction_work(
        &mut self,
        worker: AgentId,
        action: crate::ActiveAction,
        elapsed: u64,
        time: u64,
    ) -> Result<(), SimulationError> {
        let CitizenAction::Construct {
            contract: id,
            base_ms,
            duration_ms,
            ..
        } = action.action()
        else {
            return Ok(());
        };
        let snapshot = self
            .construction
            .contracts
            .get(&id)
            .ok_or(SimulationError::MissingProperty)?
            .clone();
        if snapshot.status != ContractStatus::Active || snapshot.carpenter != Some(worker) {
            return Err(SimulationError::MissingProperty);
        }
        let old_elapsed = action.duration_ms() - action.remaining_ms();
        let new_elapsed = (old_elapsed + elapsed).min(duration_ms);
        let before = u128::from(base_ms) * u128::from(old_elapsed) / u128::from(duration_ms);
        let after = u128::from(base_ms) * u128::from(new_elapsed) / u128::from(duration_ms);
        let site = self
            .construction
            .projects
            .get_mut(&snapshot.project)
            .unwrap();
        let progress = (after - before) as u64;
        site.completed_base_ms += progress;
        let stage = &mut site.stages[snapshot.stage];
        stage.completed_base_ms += progress;
        let portion_completed = stage.remaining_base_ms() == 0;
        let completed = site.remaining_base_ms() == 0;
        if portion_completed {
            let cumulative = snapshot.price;
            let payment = cumulative - snapshot.paid;
            let client = site.client;
            let account = self.economic_id(worker)?;
            self.set_account_coins(
                account,
                self.account_coins(account)?
                    .checked_add(payment)
                    .ok_or(SimulationError::WealthOverflow)?,
            )?;
            self.construction_cash(
                client,
                ConstructionCash {
                    payments_paid: payment,
                    ..Default::default()
                },
            )?;
            self.construction_cash(
                account,
                ConstructionCash {
                    payments_received: payment,
                    ..Default::default()
                },
            )?;
            let contract = self.construction.contracts.get_mut(&id).unwrap();
            contract.escrow -= payment;
            contract.paid = cumulative;
            contract.paid_base_ms =
                self.construction.projects[&snapshot.project].stages[snapshot.stage].base_ms;
            self.construction.payments.push(ConstructionPayment {
                contract: id,
                time_ms: time,
                client,
                carpenter: worker,
                amount: payment,
            });
        }
        if portion_completed {
            let contract = self.construction.contracts.get_mut(&id).unwrap();
            contract.status = ContractStatus::Completed;
            contract.ended_base_ms = Some(
                self.construction.projects[&snapshot.project].stages[snapshot.stage]
                    .completed_base_ms,
            );
        }
        if completed {
            self.finish_construction(snapshot.project)?;
        }
        Ok(())
    }

    fn finish_construction(&mut self, project: ProjectId) -> Result<(), SimulationError> {
        let site = &self.construction.projects[&project];
        self.map.install_construction(site)?;
        self.construction
            .projects
            .get_mut(&project)
            .unwrap()
            .completed = true;
        Ok(())
    }

    pub(crate) fn start_construction_action(
        &mut self,
        worker: AgentId,
    ) -> Result<bool, SimulationError> {
        let Some(contract) = self.carpenter_contract(worker).cloned() else {
            let AgentKind::Citizen(citizen) = &self.agents[&worker].kind;
            if citizen.active_action.is_none()
                && citizen.active_plan.is_none()
                && self
                    .construction
                    .contracts
                    .values()
                    .any(|contract| contract.carpenter == Some(worker) && !contract.status.open())
            {
                let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&worker).unwrap().kind;
                *citizen = citizen.start_planning()?;
                self.citizen_histories
                    .get_mut(&worker)
                    .unwrap()
                    .start_action(citizen)?;
                self.register_action_request(worker)?;
                return Ok(true);
            }
            return Ok(false);
        };
        let project = self.construction.projects[&contract.project].clone();
        let AgentKind::Citizen(citizen) = &self.agents[&worker].kind;
        if citizen.active_action.is_some() {
            return Ok(false);
        }
        use crate::planning::goals::Effect;
        if citizen.active_plan.as_ref().is_some_and(|plan| {
            !plan.finishes_committed_goal()
                && plan.plan().goals().first().is_some_and(|goal| {
                    goal.goal != Effect::Production && goal.goal != Effect::Wait
                })
        }) {
            return Ok(false);
        }
        let needs = [
            Effect::ReduceHunger,
            Effect::ReduceThirst,
            Effect::ReduceTiredness,
            Effect::ReduceClothingNeed,
            Effect::ReplenishReserves,
        ];
        if needs
            .into_iter()
            .any(|goal| crate::planning::goals::need_eligible(citizen, goal))
        {
            let mut idle = citizen.clone();
            idle.active_plan = None;
            let candidate = idle.start_planning()?;
            if candidate
                .active_plan
                .as_ref()
                .and_then(|plan| plan.plan().goals().first())
                .is_some_and(|goal| goal.goal != Effect::Production && goal.goal != Effect::Wait)
            {
                let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&worker).unwrap().kind;
                *citizen = candidate;
                self.citizen_histories
                    .get_mut(&worker)
                    .unwrap()
                    .start_action(citizen)?;
                self.register_action_request(worker)?;
                return Ok(true);
            }
        }
        let stage = &project.stages[contract.stage];
        let milestone_end = stage.base_ms;
        let wood = stage.missing_wood();
        let owned = citizen.carried_available_units(Good::Wood);
        let needed = wood.saturating_sub(owned);
        let stored = citizen
            .stored_sources(Good::Wood)
            .into_iter()
            .find(|(_, units)| *units > 0 && needed > 0);
        let listed = citizen
            .listed_sources(Good::Wood)
            .into_iter()
            .find(|(_, units)| *units > 0 && needed > 0);
        let action = if let Some((place, units)) = stored {
            if citizen.position() != self.map.position(place) {
                CitizenAction::Travel(place)
            } else {
                CitizenAction::CollectStored {
                    place,
                    list: ShoppingList::single(Good::Wood, units.min(needed)),
                }
            }
        } else if let Some((place, units)) = listed {
            if citizen.position() != self.map.position(place) {
                CitizenAction::Travel(place)
            } else {
                CitizenAction::WithdrawAt {
                    place,
                    good: Good::Wood,
                    units: units.min(needed),
                }
            }
        } else if owned < wood {
            let supplier = self
                .market
                .orders()
                .filter(|order| {
                    order.good == Good::Wood
                        && order.units > 0
                        && order.seller != citizen.market_party()
                        && self
                            .map
                            .place(order.place)
                            .is_ok_and(|site| site.kind != crate::locations::Location::Home)
                        && self.market.available_units_at(
                            citizen.market_party(),
                            order.place,
                            Good::Wood,
                        ) > 0
                })
                .min_by(|a, b| {
                    a.quoted_price
                        .total_cmp(&b.quoted_price)
                        .then_with(|| {
                            citizen
                                .position()
                                .distance(self.map.position(a.place))
                                .total_cmp(&citizen.position().distance(self.map.position(b.place)))
                        })
                        .then_with(|| a.place.0.cmp(&b.place.0))
                })
                .map(|order| order.place);
            if let Some(market) = supplier {
                if citizen.position() != self.map.position(market) {
                    CitizenAction::Travel(market)
                } else {
                    CitizenAction::BuyAt {
                        place: market,
                        list: ShoppingList::single(
                            Good::Wood,
                            (wood - owned).min(self.market.available_units_at(
                                citizen.market_party(),
                                market,
                                Good::Wood,
                            )),
                        ),
                    }
                }
            } else {
                CitizenAction::Wait
            }
        } else if citizen.position() != self.map.position(project.place) {
            CitizenAction::Travel(project.place)
        } else {
            let factor = 1.0
                - skill_duration_reduction(citizen.skill_practice_ms(Skill::Carpentry).unwrap());
            let remaining = milestone_end - stage.completed_base_ms;
            let base_ms = remaining.min((HOUR_MS as f64 / factor).floor() as u64);
            let duration_ms = (base_ms as f64 * factor).ceil() as u64;
            CitizenAction::Construct {
                contract: contract.id,
                place: project.place,
                base_ms,
                duration_ms,
                practice_ms: citizen.skill_practice_ms(Skill::Carpentry).unwrap(),
            }
        };
        self.cancel_maintenance(worker)?;
        let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&worker).unwrap().kind;
        citizen.active_plan = None;
        let started = citizen.start_action(action)?;
        if matches!(action, CitizenAction::Construct { .. }) && wood > 0 {
            citizen.set_inventory_units(Good::Wood, citizen.units(Good::Wood) - wood);
            self.construction
                .projects
                .get_mut(&project.id)
                .unwrap()
                .installed_wood += wood;
            self.construction
                .projects
                .get_mut(&project.id)
                .unwrap()
                .stages[contract.stage]
                .installed_wood += wood;
            if let Some(quote) = self
                .construction
                .contracts
                .get_mut(&contract.id)
                .unwrap()
                .quotes
                .iter_mut()
                .find(|quote| quote.carpenter == worker)
            {
                let carried = wood.min(quote.carried_wood);
                quote.carried_wood -= carried;
                quote.shared_wood = quote.shared_wood.saturating_sub(wood - carried);
            }

            self.citizen_histories
                .get_mut(&worker)
                .unwrap()
                .current
                .consumed[Good::Wood as usize] += wood;
        }
        let inventory = citizen.inventory.clone();
        *citizen = started;
        citizen.inventory = inventory;
        if matches!(action, CitizenAction::Construct { .. }) && !contract.started {
            self.construction
                .contracts
                .get_mut(&contract.id)
                .unwrap()
                .started = true;
            self.open_next_construction_stage(project.id, contract.stage)?;
            self.process_construction()?;
        }
        let AgentKind::Citizen(citizen) = &self.agents[&worker].kind;
        self.citizen_histories
            .get_mut(&worker)
            .unwrap()
            .start_action(citizen)?;
        self.register_action_request(worker)?;
        Ok(true)
    }
}

impl Universe {
    pub(crate) fn validate_construction(&self) -> Result<(), crate::persistence::SaveError> {
        use crate::persistence::SaveError;
        use std::collections::HashSet;
        let invalid = || SaveError::InvalidState("invalid construction state");
        let now = self.current_time_ms;

        let mut outputs = HashSet::new();
        let mut sequences = HashSet::new();
        let mut stage_ids = HashSet::new();
        let existing_outputs: HashSet<_> = self
            .map
            .places()
            .values()
            .flat_map(|place| {
                place.objects().map(|object| object.id.0).chain(
                    place
                        .buildings
                        .iter()
                        .flat_map(|building| building.rooms.iter().map(|room| room.id.0)),
                )
            })
            .collect();

        for (id, project) in &self.construction.projects {
            let total = project.item.base_hours() * HOUR_MS;
            let output = match project.output {
                ConstructionOutput::Room(id) => id.0,
                ConstructionOutput::Object(id) => id.0,
            };
            if !project.completed && existing_outputs.contains(&output) {
                return Err(invalid());
            }
            if *id != project.id
                || self.economic_id(project.client).is_err()
                || project.completed_base_ms > total
                || project.installed_wood > project.item.wood()
                || project.installed_wood < wood_through(project.completed_base_ms)
                || project.completed != (project.completed_base_ms == total)
                || !self
                    .construction
                    .contracts
                    .values()
                    .any(|contract| contract.project == *id)
            {
                return Err(invalid());
            }
            if project
                .stages
                .iter()
                .any(|stage| !stage_ids.insert(stage.next_contract))
                || project.stages.is_empty()
                || project.stages.iter().any(|stage| {
                    stage.base_ms == 0
                        || stage.base_ms > MILESTONE_BASE_MS
                        || stage.completed_base_ms > stage.base_ms
                        || stage.installed_wood > wood_through(stage.base_ms)
                        || stage.installed_wood < wood_through(stage.completed_base_ms)
                })
                || project
                    .stages
                    .iter()
                    .map(|stage| stage.base_ms)
                    .sum::<u64>()
                    != total
                || project
                    .stages
                    .iter()
                    .map(|stage| stage.completed_base_ms)
                    .sum::<u64>()
                    != project.completed_base_ms
                || project
                    .stages
                    .iter()
                    .map(|stage| stage.installed_wood)
                    .sum::<u64>()
                    != project.installed_wood
                || project.escrow < 0
                || project.funding_offer < 0
                || project.allocated_offer < 0
                || project.allocated_offer > project.funding_offer
                || project.allocated_base_ms > project.funding_base_ms
                || project.funding_base_ms > total
                || (project.funding_base_ms > 0
                    && project.allocated_offer
                        != (i128::from(project.funding_offer)
                            * i128::from(project.allocated_base_ms)
                            / i128::from(project.funding_base_ms))
                            as Coins)
                || (project.scheduling
                    && project.escrow != project.funding_offer - project.allocated_offer)
                || (!project.scheduling && project.escrow != 0)
            {
                return Err(invalid());
            }
            for (index, stage) in project.stages.iter().enumerate() {
                if self
                    .construction
                    .contracts
                    .get(&stage.next_contract)
                    .is_some_and(|contract| {
                        contract.project != project.id || contract.stage != index
                    })
                {
                    return Err(invalid());
                }
            }
            let place = self.map.place(project.place).map_err(|_| invalid())?;
            let building = place
                .buildings
                .iter()
                .find(|building| building.id == project.building)
                .ok_or_else(invalid)?;
            if self.economic_id(building.owner).ok() != self.economic_id(project.client).ok() {
                return Err(invalid());
            }
            match (&project.output, project.item) {
                (ConstructionOutput::Room(output), ConstructionItem::Room(size)) => {
                    if project.room.is_some()
                        || place.owner.and_then(|owner| self.economic_id(owner).ok())
                            != self.economic_id(project.client).ok()
                        || !outputs.insert(output.0)
                    {
                        return Err(invalid());
                    }
                    let room = building.rooms.iter().find(|room| room.id == *output);
                    if room.is_some() != project.completed
                        || room.is_some_and(|room| room.size != size)
                    {
                        return Err(invalid());
                    }
                }
                (ConstructionOutput::Object(output), item) if item.object_kind().is_some() => {
                    if !outputs.insert(output.0) {
                        return Err(invalid());
                    }
                    let room = building
                        .rooms
                        .iter()
                        .find(|room| Some(room.id) == project.room)
                        .ok_or_else(invalid)?;
                    let object = room.objects.iter().find(|object| object.id == *output);
                    // Bed assignment and fermentation batches can change after installation.
                    if object.is_some() != project.completed
                        || object.is_some_and(|object| {
                            !object.kind.same_capability(item.object_kind().unwrap())
                        })
                    {
                        return Err(invalid());
                    }
                }
                _ => return Err(invalid()),
            }
        }
        let mut open_stages = HashSet::new();
        for (id, contract) in &self.construction.contracts {
            let project = self
                .construction
                .projects
                .get(&contract.project)
                .ok_or_else(invalid)?;
            let stage = project.stages.get(contract.stage).ok_or_else(invalid)?;
            if !sequences.insert(contract.sequence)
                || contract.sequence >= self.construction.contracts.len() as u64
                || *id != contract.id
                || contract.maximum_offer < 0
                || contract.escrow < 0
                || contract.paid < 0
                || contract.price < 0
                || contract.posted_ms > now
                || contract.posted_ms.checked_add(BIDDING_MS) != Some(contract.bidding_ends_ms)
                || contract.start_base_ms >= stage.base_ms
                || contract.start_base_ms > stage.completed_base_ms
                || contract.paid_base_ms < contract.start_base_ms
                || contract.paid_base_ms > stage.completed_base_ms
                || contract.status.open() != contract.ended_base_ms.is_none()
                || contract
                    .ended_base_ms
                    .is_some_and(|end| end < contract.paid_base_ms || end > stage.completed_base_ms)
                || (contract.status.open() && !open_stages.insert((project.id, contract.stage)))
                || (contract.status.open()
                    && self.economic_id(project.client).ok() != Some(project.client))
            {
                return Err(invalid());
            }
            let awarded = contract.awarded_ms.is_some();
            if awarded != contract.carpenter.is_some() || awarded != contract.deadline_ms.is_some()
            {
                return Err(invalid());
            }
            if let Some(award) = contract.awarded_ms {
                if award != contract.bidding_ends_ms
                    || award > now
                    || award.checked_add(DEADLINE_MS) != contract.deadline_ms
                    || contract.price <= 0
                    || contract.price > contract.maximum_offer
                    || !contract.quotes.iter().any(|quote| {
                        Some(quote.carpenter) == contract.carpenter && quote.price == contract.price
                    })
                    || contract.paid
                        != if contract.paid_base_ms == stage.base_ms {
                            contract.price
                        } else {
                            0
                        }
                    || (contract.paid_base_ms != contract.start_base_ms
                        && contract.paid_base_ms != stage.base_ms)
                {
                    return Err(invalid());
                }
            } else if contract.price != 0
                || contract.paid != 0
                || contract.paid_base_ms != contract.start_base_ms
                || contract.started
            {
                return Err(invalid());
            }
            match contract.status {
                ContractStatus::Bidding
                    if awarded
                        || contract.bidding_ends_ms <= now
                        || contract.escrow != contract.maximum_offer
                        || contract.next_quote_ms <= now
                        || contract.next_quote_ms > contract.bidding_ends_ms =>
                {
                    return Err(invalid());
                }
                ContractStatus::Active
                    if !awarded
                        || contract.deadline_ms.is_none_or(|deadline| deadline <= now)
                        || contract.escrow != contract.price
                        || stage.remaining_base_ms() == 0 =>
                {
                    return Err(invalid());
                }
                ContractStatus::Completed
                    if !awarded
                        || stage.remaining_base_ms() != 0
                        || contract.escrow != 0
                        || contract.paid != contract.price
                        || contract.paid_base_ms != stage.base_ms
                        || !contract.started =>
                {
                    return Err(invalid());
                }
                ContractStatus::Failed
                    if !awarded
                        || contract.deadline_ms.is_none_or(|deadline| deadline > now)
                        || contract.escrow != 0 =>
                {
                    return Err(invalid());
                }
                ContractStatus::NoBids
                    if awarded || contract.bidding_ends_ms > now || contract.escrow != 0 =>
                {
                    return Err(invalid());
                }
                ContractStatus::Cancelled if awarded || contract.escrow != 0 => {
                    return Err(invalid());
                }
                _ => {}
            }
            let mut quoted = HashSet::new();
            for quote in &contract.quotes {
                let worker = self.agents.get(&quote.carpenter).ok_or_else(invalid)?;
                let AgentKind::Citizen(worker) = &worker.kind;
                if !quoted.insert(quote.carpenter)
                    || worker.skill_practice_ms(Skill::Carpentry).is_none()
                    || quote.price <= 0
                    || quote.price > contract.maximum_offer
                    || quote.financing < 0
                    || quote.procurement_cash < 0
                    || quote.procurement_cash > quote.financing
                    || quote.shared_wood.saturating_add(quote.carried_wood)
                        > wood_through(stage.base_ms - contract.start_base_ms)
                {
                    return Err(invalid());
                }
            }
            let paid = self
                .construction
                .payments
                .iter()
                .filter(|payment| payment.contract == *id)
                .try_fold(0i64, |sum, payment| sum.checked_add(payment.amount))
                .ok_or_else(invalid)?;
            if paid != contract.paid {
                return Err(invalid());
            }
        }
        for project in self.construction.projects.values() {
            for (index, stage) in project.stages.iter().enumerate() {
                let mut contracts: Vec<_> = self
                    .construction
                    .contracts
                    .values()
                    .filter(|contract| contract.project == project.id && contract.stage == index)
                    .collect();
                contracts.sort_by_key(|contract| contract.sequence);
                let mut previous_end = 0;
                for contract in contracts {
                    if contract.start_base_ms != previous_end {
                        return Err(invalid());
                    }
                    previous_end = contract.ended_base_ms.unwrap_or(stage.completed_base_ms);
                }
                if previous_end != stage.completed_base_ms {
                    return Err(invalid());
                }
            }
        }
        for worker in self.agents.keys() {
            if self.carpenter_committed_base_ms(*worker) > CAPACITY_BASE_MS {
                return Err(invalid());
            }
        }
        for payment in &self.construction.payments {
            let contract = self
                .construction
                .contracts
                .get(&payment.contract)
                .ok_or_else(invalid)?;
            if payment.time_ms > now
                || contract
                    .awarded_ms
                    .is_none_or(|award| award > payment.time_ms)
                || contract
                    .deadline_ms
                    .is_none_or(|deadline| payment.time_ms > deadline)
                || payment.amount < 0
                || payment.client != self.construction.projects[&contract.project].client
                || contract.carpenter != Some(payment.carpenter)
            {
                return Err(invalid());
            }
        }
        for (id, place) in self.map.places() {
            if self.construction_reserved_area(*id, None) > place.available_area_m2() {
                return Err(invalid());
            }
            for room in place.buildings.iter().flat_map(|building| &building.rooms) {
                if self.construction_reserved_area(*id, Some(room.id)) > room.available_area_m2() {
                    return Err(invalid());
                }
            }
        }
        let mut refreshed = self.clone();
        refreshed
            .refresh_construction_context()
            .map_err(|_| invalid())?;
        for (id, agent) in &self.agents {
            let AgentKind::Citizen(worker) = &agent.kind;
            let AgentKind::Citizen(expected) = &refreshed.agents[id].kind;
            if worker.construction_access != expected.construction_access
                || worker.construction_escrow != expected.construction_escrow
                || worker.construction_protected != expected.construction_protected
                || worker.construction_wood_reserve != expected.construction_wood_reserve
            {
                return Err(invalid());
            }
            if worker.active_plan.as_ref().is_some_and(|plan| {
                plan.plan()
                    .actions()
                    .iter()
                    .any(|action| matches!(action, CitizenAction::Construct { .. }))
            }) {
                return Err(invalid());
            }
            if let Some(action) = worker.active_action()
                && let CitizenAction::Construct {
                    contract: id,
                    place,
                    base_ms,
                    duration_ms,
                    practice_ms,
                } = action.action()
            {
                let contract = self.construction.contracts.get(&id).ok_or_else(invalid)?;
                let project = &self.construction.projects[&contract.project];
                let elapsed = action
                    .duration_ms()
                    .checked_sub(action.remaining_ms())
                    .ok_or_else(invalid)?;
                let progressed = if duration_ms == 0 {
                    return Err(invalid());
                } else {
                    (u128::from(base_ms) * u128::from(elapsed) / u128::from(duration_ms)) as u64
                };
                let stage = &project.stages[contract.stage];
                let start = stage
                    .completed_base_ms
                    .checked_sub(progressed)
                    .ok_or_else(invalid)?;
                if start < contract.start_base_ms {
                    return Err(invalid());
                }
                let factor = 1.0 - skill_duration_reduction(practice_ms);
                let milestone_end = stage.base_ms;
                let expected_base =
                    (milestone_end - start).min((HOUR_MS as f64 / factor).floor() as u64);
                if contract.status != ContractStatus::Active
                    || contract.carpenter != Some(worker.id)
                    || place != project.place
                    || worker.position() != self.map.position(project.place)
                    || worker.active_plan.is_some()
                    || practice_ms > crate::production::MAX_SKILL_PRACTICE_MS
                    || start < contract.start_base_ms
                    || base_ms != expected_base
                    || duration_ms != (base_ms as f64 * factor).ceil() as u64
                    || duration_ms > HOUR_MS
                    || action.duration_ms() != duration_ms
                    || worker.skill_practice_ms(Skill::Carpentry)
                        != Some(
                            practice_ms
                                .saturating_add(elapsed)
                                .min(crate::production::MAX_SKILL_PRACTICE_MS),
                        )
                    || stage.installed_wood < wood_through(milestone_end)
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Citizen, StartingRole, locations::Map};

    fn fixture(item: ConstructionItem, wood: Quantity) -> (Universe, AgentId, AgentId, ContractId) {
        let (world, client) = Universe::with_map(Map::default())
            .with_citizen(
                "Client",
                Citizen::new(-100.0).unwrap().with_coins(1_000_000).unwrap(),
            )
            .unwrap();
        let worker = Citizen::with_needs(-100.0, -100.0)
            .unwrap()
            .with_thirst(-100.0)
            .unwrap()
            .with_garment_condition(Some(1.0))
            .unwrap()
            .with_starting_role(StartingRole::Carpenter)
            .with_coins(100_000)
            .unwrap()
            .with_good(Good::Wood, wood)
            .unwrap()
            .with_good(Good::BreadRoll, 10_000)
            .unwrap()
            .with_good(Good::Water, 100_000)
            .unwrap();
        let (world, worker) = world.with_citizen("Carpenter", worker).unwrap();
        let AgentKind::Citizen(owner) = &world.agents[&client].kind;
        let place = owner.home;
        let building = &world.map.place(place).unwrap().buildings[0];
        let room = if matches!(item, ConstructionItem::Room(_)) {
            None
        } else {
            Some(building.rooms[0].id)
        };
        let (world, contract) = world
            .request_construction(client, place, building.id, room, item, 100_000)
            .unwrap();
        (world, client, worker, contract)
    }

    fn award(mut world: Universe, worker: AgentId) -> Universe {
        world = world.advance(BIDDING_MS).unwrap();
        world.cancel_maintenance(worker).unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&worker).unwrap().kind;
        citizen.abandon_action().unwrap();
        citizen.hunger = -100.0;
        citizen.thirst = -100.0;
        citizen.tiredness = -100.0;
        world.map = citizen.map.clone();
        world.storage = citizen.storage.clone();
        world.start_construction_action(worker).unwrap();
        world.refresh_market();
        world
    }

    #[test]
    fn capacity_counts_multiple_bids_and_allocates_carried_wood_once() {
        let (world, client, worker, first) =
            fixture(ConstructionItem::Room(RoomSize::Small), 2_400);
        let project = &world.construction.projects[&world.construction.contracts[&first].project];
        let place = project.place;
        let building = project.building;
        let (world, second) = world
            .request_construction(
                client,
                place,
                building,
                None,
                ConstructionItem::Room(RoomSize::Small),
                100_000,
            )
            .unwrap();
        let (world, third) = world
            .request_construction(
                client,
                place,
                building,
                None,
                ConstructionItem::Room(RoomSize::Small),
                100_000,
            )
            .unwrap();
        assert_eq!(world.carpenter_committed_base_ms(worker), CAPACITY_BASE_MS);
        let first_quote = &world.construction.contracts[&first].quotes[0];
        let second_quote = &world.construction.contracts[&second].quotes[0];
        assert_eq!(first_quote.carried_wood, 2_400);
        assert_eq!(first_quote.procurement_cash, 0);
        assert_eq!(second_quote.carried_wood, 0);
        assert!(second_quote.procurement_cash > 0);
        assert!(world.construction.contracts[&third].quotes.is_empty());
        let saved = world.to_save_string().unwrap();
        assert_eq!(world, Universe::from_save_str(&saved).unwrap());
        let world = award(world, worker).advance(6 * HOUR_MS).unwrap();
        assert!(world.carpenter_committed_base_ms(worker) <= CAPACITY_BASE_MS - 5 * HOUR_MS);
        let room = world
            .map
            .place(place)
            .unwrap()
            .buildings
            .iter()
            .find(|candidate| candidate.id == building)
            .unwrap()
            .rooms[0]
            .id;
        let (world, small) = world
            .request_construction(
                client,
                place,
                building,
                Some(room),
                ConstructionItem::FishCrate,
                10_000,
            )
            .unwrap();
        assert!(
            world.construction.contracts[&small]
                .quotes
                .iter()
                .any(|quote| quote.carpenter == worker)
        );
    }

    #[test]
    fn next_portion_auction_opens_on_work_and_project_escrow_is_split_invariant() {
        let (world, client, worker, first) =
            fixture(ConstructionItem::Room(RoomSize::Small), 6_000);
        let project = world.construction.contracts[&first].project;
        assert_eq!(world.construction.contracts.len(), 1);
        assert_eq!(world.construction.contracts[&first].maximum_offer, 40_000);
        assert_eq!(world.construction.projects[&project].escrow, 60_000);
        let world = award(world, worker).advance(HOUR_MS).unwrap();
        assert_eq!(world.construction.contracts.len(), 2);
        let second = world
            .construction
            .contracts
            .values()
            .find(|contract| contract.project == project && contract.stage == 1)
            .unwrap();
        assert_eq!(second.status, ContractStatus::Bidding);
        assert!(second.posted_ms >= BIDDING_MS && second.posted_ms <= world.current_time_ms);
        assert_eq!(world.construction.projects[&project].escrow, 20_000);
        let original_cash = world.account_coins(client).unwrap();
        let large = world.advance(2 * DAY_MS).unwrap();
        let mut split = world;
        for _ in 0..48 {
            split = split.advance(HOUR_MS).unwrap();
        }
        assert_eq!(large.construction, split.construction);
        assert_eq!(
            large.account_coins(client).unwrap(),
            split.account_coins(client).unwrap()
        );
        assert!(large.account_coins(client).unwrap() >= original_cash);
        let saved = large.to_save_string().unwrap();
        assert_eq!(large, Universe::from_save_str(&saved).unwrap());
    }

    #[test]
    fn escrow_is_wealth_and_cancel_returns_cash_exactly() {
        let (world, client, _, id) = fixture(ConstructionItem::FishCrate, 1_000);
        assert_eq!(world.account_coins(client).unwrap(), 900_000);
        let AgentKind::Citizen(citizen) = &world.agents[&client].kind;
        assert_eq!(citizen.construction_escrow, 100_000);
        assert_eq!(world.construction.contracts[&id].quotes.len(), 1);
        let world = world.cancel_construction(client, id).unwrap();
        assert_eq!(world.account_coins(client).unwrap(), 1_000_000);
        assert_eq!(world.construction.contracts[&id].escrow, 0);
        assert_eq!(
            world.construction_reserved_area(
                citizen.home,
                Some(world.map.place(citizen.home).unwrap().buildings[0].rooms[0].id)
            ),
            0
        );
        let saved = world.to_save_string().unwrap();
        assert_eq!(world, Universe::from_save_str(&saved).unwrap());
    }

    #[test]
    fn no_bids_refunds_and_retry_requires_larger_offer() {
        let (world, client, _, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let world = world.cancel_construction(client, id).unwrap();
        let project = world.construction.contracts[&id].project;
        let (world, low) = world.continue_construction(client, project, 1).unwrap();
        assert!(world.construction.contracts[&low].quotes.is_empty());
        let world = world.advance(BIDDING_MS).unwrap();
        assert_eq!(
            world.construction.contracts[&low].status,
            ContractStatus::NoBids
        );
        assert_eq!(world.account_coins(client).unwrap(), 1_000_000);
        assert!(world.continue_construction(client, project, 1).is_err());
        let site = &world.construction.projects[&project];
        assert!(
            world
                .request_construction(client, site.place, site.building, site.room, site.item, 1)
                .is_err()
        );
    }

    #[test]
    fn completion_settles_once_and_is_split_advance_invariant() {
        let (world, _, worker, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let world = award(world, worker);
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Active
        );
        let large = world.advance(6 * HOUR_MS).unwrap();
        let mut split = world;
        for _ in 0..24 {
            split = split.advance(HOUR_MS / 4).unwrap();
        }
        assert_eq!(large.construction, split.construction);
        assert_eq!(large.map, split.map);
        assert_eq!(
            large.account_coins(worker).unwrap(),
            split.account_coins(worker).unwrap()
        );
        let AgentKind::Citizen(a) = &large.agents[&worker].kind;
        let AgentKind::Citizen(b) = &split.agents[&worker].kind;
        assert_eq!(a.active_action(), b.active_action());
        assert_eq!(a.skills, b.skills);
        assert_eq!(a.inventory, b.inventory);
        let contract = &large.construction.contracts[&id];
        assert_eq!(contract.status, ContractStatus::Completed);
        assert_eq!(contract.paid, contract.price);
        assert_eq!(contract.escrow, 0);
        assert_eq!(large.construction.payments.len(), 1);
        let saved = large.to_save_string().unwrap();
        assert_eq!(large, Universe::from_save_str(&saved).unwrap());
    }

    #[test]
    fn partial_work_and_installed_materials_survive_unpaid_portion_failure() {
        let (world, client, worker, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let mut world = award(world, worker).advance(HOUR_MS + HOUR_MS / 2).unwrap();
        let project = world.construction.contracts[&id].project;
        let before = world.construction.projects[&project].clone();
        assert!(before.completed_base_ms > HOUR_MS);
        assert_eq!(before.installed_wood, 1_000);
        assert_eq!(world.construction.contracts[&id].paid, 0);
        let client_before = world.account_coins(client).unwrap();
        let escrow = world.construction.contracts[&id].escrow;
        world
            .construction
            .contracts
            .get_mut(&id)
            .unwrap()
            .deadline_ms = Some(world.current_time_ms + 1);
        world = world.advance(1).unwrap();
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Failed
        );
        assert_eq!(world.account_coins(client).unwrap(), client_before + escrow);
        let retained = world.construction.projects[&project].completed_base_ms;
        let (world, continuation) = world
            .continue_construction(client, project, 100_000)
            .unwrap();
        assert_eq!(
            world.construction.contracts[&continuation].start_base_ms,
            retained
        );
        assert_eq!(world.construction.projects[&project].missing_wood(), 0);
        assert!(
            world.construction.contracts[&continuation].quotes[0].price
                < world.construction.contracts[&id].price
        );
    }

    #[test]
    fn save_round_trip_mid_action_rejects_forged_work_and_payments() {
        let (world, _, worker, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let world = award(world, worker).advance(HOUR_MS / 3).unwrap();
        let save = world.to_save_string().unwrap();
        assert_eq!(world, Universe::from_save_str(&save).unwrap());
        let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
        let active = citizen.active_action().unwrap();
        assert!(world.start_action(worker, active.action()).is_err());
        let mut bad = world.clone();
        bad.construction.contracts.get_mut(&id).unwrap().paid = 1;
        assert!(bad.validate_construction().is_err());
        let mut bad = world;
        let AgentKind::Citizen(citizen) = &mut bad.agents.get_mut(&worker).unwrap().kind;
        if let CitizenAction::Construct { base_ms, .. } =
            &mut citizen.active_action.as_mut().unwrap().action
        {
            *base_ms *= 10;
        }
        assert!(bad.validate_construction().is_err());
    }
    #[test]
    fn real_deadline_preserves_work_then_continuation_finishes_after_save_load() {
        let (world, client, worker, id) = fixture(ConstructionItem::Room(RoomSize::Small), 2_400);
        let policy = crate::marketplace::CaravanPolicy {
            import_tariff_basis_points: 10_000,
            ..world.caravan_policy()
        };
        let world = world.with_caravan_policy(policy).unwrap();
        let world = award(world, worker);
        let mut world = world.advance(DEADLINE_MS + BIDDING_MS + HOUR_MS).unwrap();
        let project = world.construction.contracts[&id].project;
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Completed
        );
        assert!(world.construction.contracts.values().any(
            |contract| contract.project == project && contract.status == ContractStatus::Failed
        ));
        assert_eq!(
            world.construction.projects[&project].completed_base_ms,
            MILESTONE_BASE_MS
        );
        assert_eq!(world.construction.projects[&project].installed_wood, 2_400);
        assert!(world.construction.contracts[&id].paid > 0);
        let saved = world.to_save_string().unwrap();
        world = Universe::from_save_str(&saved).unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&worker).unwrap().kind;
        citizen.set_inventory_units(Good::Wood, 3_600);
        world.refresh_market();
        let (world, next) = world
            .continue_construction(client, project, 100_000)
            .unwrap();
        let world = award(world, worker).advance(DEADLINE_MS).unwrap();
        let AgentKind::Citizen(worker_state) = &world.agents[&worker].kind;
        assert_eq!(
            world.construction.contracts[&next].status,
            ContractStatus::Completed,
            "progress={}, wood={}, action={:?}, goal={:?}, needs={:?}",
            world.construction.projects[&project].completed_base_ms,
            world.construction.projects[&project].installed_wood,
            worker_state.active_action(),
            worker_state
                .active_plan
                .as_ref()
                .map(|plan| plan.plan().goals()),
            (
                worker_state.hunger(),
                worker_state.thirst(),
                worker_state.tiredness()
            )
        );
        assert_eq!(world.construction.projects[&project].installed_wood, 6_000);
        let saved = world.to_save_string().unwrap();
        assert_eq!(
            world.construction,
            Universe::from_save_str(&saved).unwrap().construction
        );
        let worker_history = world.citizen_history(worker).unwrap();
        let sleeping = worker_history
            .completed
            .iter()
            .map(|day| day.activity.sleep_ms)
            .sum::<u64>()
            + worker_history.current.activity.sleep_ms;
        assert!(sleeping > 0);
    }

    #[test]
    fn released_reservation_cannot_be_resurrected_over_new_work() {
        let (world, client, _, first) = fixture(ConstructionItem::Bed, 4_000);
        let site =
            world.construction.projects[&world.construction.contracts[&first].project].clone();
        let world = world.cancel_construction(client, first).unwrap();
        let (world, second) = world
            .request_construction(
                client,
                site.place,
                site.building,
                site.room,
                ConstructionItem::WoodRack,
                100_000,
            )
            .unwrap();
        assert_ne!(first, second);
        assert!(
            world
                .continue_construction(client, site.id, 100_000)
                .is_err()
        );
    }

    #[test]
    fn award_keeps_sealed_price_and_rechecks_exact_client_reserve() {
        let (mut world, client, worker, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let price = world.construction.contracts[&id].quotes[0].price;
        world
            .construction
            .contracts
            .get_mut(&id)
            .unwrap()
            .maximum_offer = price;
        let refund = 100_000 - price;
        world.construction.contracts.get_mut(&id).unwrap().escrow = price;
        world.set_account_coins(client, 900_000 + refund).unwrap();
        // Raising labour costs must not replace the sealed price.
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&worker).unwrap().kind;
        citizen.hunger_per_hour *= 10.0;
        let world = world.advance(BIDDING_MS).unwrap();
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Active
        );
        assert_eq!(world.construction.contracts[&id].price, price);

        let (mut world, client, _, id) = fixture(ConstructionItem::FishCrate, 1_000);
        world.set_account_coins(client, 0).unwrap();
        let world = world.advance(BIDDING_MS).unwrap();
        // The refunded difference is enough in this fixture; leave less escrow than the reserve.
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Active
        );
        let (mut world, client, _, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let price = world.construction.contracts[&id].quotes[0].price;
        world
            .construction
            .contracts
            .get_mut(&id)
            .unwrap()
            .maximum_offer = price;
        world.construction.contracts.get_mut(&id).unwrap().escrow = price;
        world.set_account_coins(client, 0).unwrap();
        let world = world.advance(BIDDING_MS).unwrap();
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Cancelled
        );
        assert_eq!(world.account_coins(client).unwrap(), price);
    }

    #[test]
    fn open_work_blocks_household_formation_but_cancelled_work_does_not() {
        let (world, client, worker, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let AgentKind::Citizen(citizen) = &world.agents[&client].kind;
        let home = citizen.home;
        assert!(
            world
                .with_household("Family", home, &[client, worker])
                .is_err()
        );
        let world = world.cancel_construction(client, id).unwrap();
        let (world, family) = world
            .with_household("Family", home, &[client, worker])
            .unwrap();
        let project = world.construction.contracts[&id].project;
        assert!(
            world
                .continue_construction(family, project, 100_000)
                .is_ok()
        );
    }

    #[test]
    fn partial_purchases_from_multiple_sellers_are_retained_until_later_supply() {
        let (world, _, worker, id) = fixture(ConstructionItem::FishCrate, 0);
        let policy = crate::marketplace::CaravanPolicy {
            import_tariff_basis_points: 10_000,
            ..world.caravan_policy()
        };
        let mut world = world.with_caravan_policy(policy).unwrap();
        for (name, wood) in [("First supplier", 400), ("Second supplier", 300)] {
            let (next, seller) = world
                .with_citizen(
                    name,
                    Citizen::new(-100.0)
                        .unwrap()
                        .with_good(Good::Wood, wood)
                        .unwrap(),
                )
                .unwrap();
            world = next
                .start_action(seller, CitizenAction::List(Good::Wood, wood))
                .unwrap()
                .advance(crate::TRADE_DURATION_MS)
                .unwrap();
        }
        let mut world = award(world, worker).advance(2 * HOUR_MS).unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
        assert_eq!(citizen.units(Good::Wood), 700);
        assert_eq!(citizen.excess_goods().unwrap().units(Good::Wood), 0);
        assert_eq!(world.market.requested(worker).units(Good::Wood), 300);
        assert_eq!(
            world.construction.projects[&world.construction.contracts[&id].project]
                .completed_base_ms,
            0
        );
        world = world.advance(DAY_MS).unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
        assert_eq!(citizen.units(Good::Wood), 700);
        assert_eq!(
            world
                .market
                .listed_units(citizen.market_party(), Good::Wood),
            0
        );
        let (next, seller) = world
            .with_citizen(
                "Later supplier",
                Citizen::new(-100.0)
                    .unwrap()
                    .with_good(Good::Wood, 300)
                    .unwrap(),
            )
            .unwrap();
        let world = next
            .start_action(seller, CitizenAction::List(Good::Wood, 300))
            .unwrap()
            .advance(2 * DAY_MS)
            .unwrap();
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Completed
        );
        let trades: Vec<_> = world
            .market
            .trades()
            .iter()
            .filter(|trade| trade.buyer.citizen() == Some(worker) && trade.good == Good::Wood)
            .collect();
        assert_eq!(trades.iter().map(|trade| trade.units).sum::<u64>(), 1_000);
        assert_eq!(trades.len(), 3);
        assert!(Universe::from_save_str(&world.to_save_string().unwrap()).is_ok());
    }

    #[test]
    fn tied_quotes_choose_persisted_random_winner_and_workers_hold_one_quote() {
        let (world, _, first, id) = fixture(ConstructionItem::FishCrate, 1_000);
        let worker = Citizen::with_needs(-100.0, -100.0)
            .unwrap()
            .with_thirst(-100.0)
            .unwrap()
            .with_garment_condition(Some(1.0))
            .unwrap()
            .with_starting_role(StartingRole::Carpenter)
            .with_coins(100_000)
            .unwrap()
            .with_good(Good::Wood, 1_000)
            .unwrap()
            .with_good(Good::BreadRoll, 10_000)
            .unwrap()
            .with_good(Good::Water, 100_000)
            .unwrap();
        let (world, second) = world.with_citizen("Second carpenter", worker).unwrap();
        let world = world.advance(HOUR_MS).unwrap();
        let quotes = &world.construction.contracts[&id].quotes;
        assert_eq!(quotes.len(), 2);
        assert_eq!(quotes[0].price, quotes[1].price);
        let large = world.advance(BIDDING_MS - HOUR_MS).unwrap();
        let mut split = world;
        for _ in 0..46 {
            split = split.advance(HOUR_MS / 2).unwrap();
        }
        assert_eq!(
            large.construction.contracts[&id].carpenter,
            split.construction.contracts[&id].carpenter
        );
        assert!(
            matches!(large.construction.contracts[&id].carpenter, Some(worker) if worker == first || worker == second)
        );
        assert_eq!(large.construction.rng, split.construction.rng);
    }

    #[test]
    fn shared_financing_limits_new_quotes_and_capital_spending_but_allows_meals() {
        let (world, client, first, old) = fixture(ConstructionItem::FishCrate, 0);
        let world = world.cancel_construction(client, old).unwrap();
        let second = Citizen::with_needs(-100.0, -100.0)
            .unwrap()
            .with_starting_role(StartingRole::Carpenter)
            .with_garment_condition(Some(1.0))
            .unwrap();
        let (world, second) = world.with_citizen("Partner", second).unwrap();
        let AgentKind::Citizen(worker) = &world.agents[&first].kind;
        let (mut world, family) = world
            .with_household("Carpenters", worker.home, &[first, second])
            .unwrap();
        let financing = world
            .construction_quote(first, &world.construction.contracts[&old], None)
            .unwrap()
            .unwrap()
            .financing;
        world.set_account_coins(family, financing).unwrap();
        world.refresh_market();
        let project = world.construction.contracts[&old].project;
        let (mut world, id) = world
            .continue_construction(client, project, 100_000)
            .unwrap();
        assert_eq!(world.construction.contracts[&id].quotes.len(), 1);
        let reserved_worker = world.construction.contracts[&id].quotes[0].carpenter;
        let spender = if reserved_worker == first {
            second
        } else {
            first
        };
        let capital = world.construction.contracts[&id].quotes[0].procurement_cash;
        assert!(capital > 0);
        let AgentKind::Citizen(seller) = &mut world.agents.get_mut(&client).unwrap().kind;
        seller.set_inventory_units(Good::Wood, 10_000);
        seller.set_inventory_units(Good::BreadRoll, 100);
        world = world
            .start_action(client, CitizenAction::List(Good::Wood, 10_000))
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        world = world
            .start_action(
                spender,
                CitizenAction::Buy(ShoppingList::single(Good::Wood, 10_000)),
            )
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert!(world.account_coins(family).unwrap() >= capital);
        world = world
            .start_action(client, CitizenAction::List(Good::BreadRoll, 100))
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        world = world
            .start_action(
                spender,
                CitizenAction::Buy(ShoppingList::single(Good::BreadRoll, 100)),
            )
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert!(world.account_coins(family).unwrap() < capital);
    }

    #[test]
    fn two_active_household_carpenters_spend_each_procurement_allocation_and_finish() {
        let (world, client, first, old) = fixture(ConstructionItem::Room(RoomSize::Small), 0);
        let world = world.cancel_construction(client, old).unwrap();
        let policy = crate::marketplace::CaravanPolicy {
            import_tariff_basis_points: 10_000,
            ..world.caravan_policy()
        };
        let world = world.with_caravan_policy(policy).unwrap();
        let second = Citizen::with_needs(-100.0, -100.0)
            .unwrap()
            .with_thirst(-100.0)
            .unwrap()
            .with_starting_role(StartingRole::Carpenter)
            .with_coins(100_000)
            .unwrap()
            .with_garment_condition(Some(1.0))
            .unwrap()
            .with_good(Good::BreadRoll, 10_000)
            .unwrap()
            .with_good(Good::Water, 100_000)
            .unwrap();
        let (world, second) = world.with_citizen("Partner", second).unwrap();
        let AgentKind::Citizen(worker) = &world.agents[&first].kind;
        let (world, family) = world
            .with_household("Carpenters", worker.home, &[first, second])
            .unwrap();
        let project = world.construction.contracts[&old].project;
        let (world, room_job) = world
            .continue_construction(client, project, 100_000)
            .unwrap();
        let mut world = world.advance(BIDDING_MS).unwrap();
        let room_worker = world.construction.contracts[&room_job].carpenter.unwrap();
        let other = if room_worker == first { second } else { first };
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&other).unwrap().kind;
        *citizen = citizen
            .clone()
            .with_skill_practice_ms(
                Skill::Carpentry,
                Some(crate::production::MAX_SKILL_PRACTICE_MS),
            )
            .unwrap();
        let AgentKind::Citizen(owner) = &world.agents[&client].kind;
        let place = owner.home;
        let building = &world.map.place(place).unwrap().buildings[0];
        let (next, object_job) = world
            .request_construction(
                client,
                place,
                building.id,
                Some(building.rooms[0].id),
                ConstructionItem::FishCrate,
                100_000,
            )
            .unwrap();
        world = next.advance(BIDDING_MS).unwrap();
        assert_eq!(
            world.construction.contracts[&room_job].status,
            ContractStatus::Active
        );
        assert_eq!(
            world.construction.contracts[&object_job].status,
            ContractStatus::Active
        );
        assert_ne!(
            world.construction.contracts[&room_job].carpenter,
            world.construction.contracts[&object_job].carpenter
        );
        let market = world.map.public_place(crate::locations::Location::Market);
        world
            .market
            .list_at_quote(client, market, Good::Wood, 7_000, 65.0)
            .unwrap();
        // A cash shock leaves exactly both funded first-material purchases available.
        let party = crate::marketplace::MarketParty::Household(family);
        let capital = world
            .market
            .purchase_cost_at(party, market, Good::Wood, 2_400)
            .unwrap()
            + world
                .market
                .purchase_cost_at(party, market, Good::Wood, 1_000)
                .unwrap();
        world.set_account_coins(family, capital).unwrap();
        for id in [first, second] {
            world.cancel_maintenance(id).unwrap();
            let AgentKind::Citizen(worker) = &mut world.agents.get_mut(&id).unwrap().kind;
            worker.abandon_action().unwrap();
            worker.hunger = -100.0;
            worker.thirst = -100.0;
            worker.tiredness = -100.0;
            world.map = worker.map.clone();
            world.storage = worker.storage.clone();
        }
        world.refresh_construction_context().unwrap();
        world.refresh_market();
        for worker in [first, second] {
            world.start_construction_action(worker).unwrap();
        }
        let world = world.advance(2 * HOUR_MS).unwrap();
        assert!(
            world.construction.projects[&world.construction.contracts[&room_job].project]
                .installed_wood
                >= 2_400
        );
        assert_eq!(
            world.construction.projects[&world.construction.contracts[&object_job].project]
                .installed_wood,
            1_000
        );
        let world = world.advance(5 * DAY_MS).unwrap();
        assert_eq!(
            world.construction.contracts[&room_job].status,
            ContractStatus::Completed
        );
        assert_eq!(
            world.construction.contracts[&object_job].status,
            ContractStatus::Completed
        );
    }

    #[test]
    fn different_carpenters_can_work_on_overlapping_portions_of_one_project() {
        let (world, _, first, id) = fixture(ConstructionItem::Room(RoomSize::Small), 6_000);
        let mut world = award(world, first).advance(HOUR_MS).unwrap();
        let project = world.construction.contracts[&id].project;
        let (next, second) = world
            .with_citizen(
                "Second carpenter",
                Citizen::with_needs(-100.0, -100.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Carpenter)
                    .with_garment_condition(Some(1.0))
                    .unwrap()
                    .with_good(Good::BreadRoll, 10_000)
                    .unwrap()
                    .with_good(Good::Water, 100_000)
                    .unwrap()
                    .with_coins(100_000)
                    .unwrap()
                    .with_good(Good::Wood, 6_000)
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &next.agents[&second].kind;
        let home = citizen.home;
        world = next
            .with_stored_good(
                home,
                crate::storage::GoodsOwner::Agent(second),
                Good::BreadRoll,
                10_000,
            )
            .unwrap()
            .with_stored_good(
                home,
                crate::storage::GoodsOwner::Agent(second),
                Good::Water,
                100_000,
            )
            .unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&second).unwrap().kind;
        citizen.set_inventory_units(Good::BreadRoll, 0);
        citizen.set_inventory_units(Good::Water, 0);
        let second_id = world
            .construction
            .contracts
            .values()
            .find(|contract| contract.project == project && contract.stage == 1)
            .unwrap()
            .id;
        // Isolate simultaneous contract execution from the need planner's scheduling.
        world.current_time_ms = world.construction.contracts[&second_id].bidding_ends_ms;
        let snapshot = world.construction.contracts[&second_id].clone();
        let quote = world
            .construction_quote(second, &snapshot, None)
            .unwrap()
            .unwrap();
        world
            .construction
            .contracts
            .get_mut(&second_id)
            .unwrap()
            .quotes = vec![quote];
        world.award_construction(second_id).unwrap();
        assert_eq!(
            world.construction.contracts[&id].status,
            ContractStatus::Active
        );
        assert_eq!(
            world.construction.contracts[&second_id].carpenter,
            Some(second)
        );
        for worker in [first, second] {
            let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&worker).unwrap().kind;
            citizen.active_action = None;
            citizen.active_plan = None;
            citizen.hunger = -100.0;
            citizen.thirst = -100.0;
            citizen.tiredness = -100.0;
            citizen.position = world
                .map
                .position(world.construction.projects[&project].place);
        }
        world.refresh_construction_context().unwrap();
        world.refresh_market();
        for worker in [first, second] {
            assert!(world.start_construction_action(worker).unwrap());
            let AgentKind::Citizen(citizen) = &world.agents[&worker].kind;
            let action = citizen.active_action().unwrap();
            assert!(
                matches!(action.action(), CitizenAction::Construct { .. }),
                "worker {worker:?}: {:?}",
                action.action()
            );
            world
                .advance_construction_work(
                    worker,
                    action,
                    HOUR_MS / 2,
                    world.current_time_ms + HOUR_MS / 2,
                )
                .unwrap();
        }
        let stages = &world.construction.projects[&project].stages;
        assert!(stages[0].completed_base_ms > HOUR_MS / 2);
        assert!(stages[1].completed_base_ms >= HOUR_MS / 2);
        assert_eq!(
            world.construction.projects[&project].completed_base_ms,
            stages
                .iter()
                .map(|stage| stage.completed_base_ms)
                .sum::<u64>()
        );
        assert_eq!(world.construction.projects[&project].installed_wood, 4_800);
    }

    #[test]
    fn sibling_carpenters_each_keep_their_own_carried_material_allocation() {
        let (world, client, first, old) = fixture(ConstructionItem::FishCrate, 1_000);
        let world = world.cancel_construction(client, old).unwrap();
        let (world, second) = world
            .with_citizen(
                "Partner",
                Citizen::new(-100.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Carpenter)
                    .with_good(Good::Wood, 1_000)
                    .unwrap()
                    .with_coins(100_000)
                    .unwrap(),
            )
            .unwrap();
        let AgentKind::Citizen(worker) = &world.agents[&first].kind;
        let (world, _) = world
            .with_household("Carpenters", worker.home, &[first, second])
            .unwrap();
        let project = world.construction.contracts[&old].project;
        let (world, id) = world
            .continue_construction(client, project, 100_000)
            .unwrap();
        let quotes = &world.construction.contracts[&id].quotes;
        assert_eq!(quotes.len(), 2);
        assert!(quotes.iter().all(|quote| quote.carried_wood == 1_000
            && quote.shared_wood == 0
            && quote.procurement_cash == 0));
    }

    #[test]
    fn shared_owned_wood_is_allocated_once_across_quotes() {
        let (world, client, first, old) = fixture(ConstructionItem::FishCrate, 0);
        let world = world.cancel_construction(client, old).unwrap();
        let (world, store) = world
            .with_property(first, crate::locations::Location::Bakery)
            .unwrap();
        let second = Citizen::new(-100.0)
            .unwrap()
            .with_starting_role(StartingRole::Carpenter)
            .with_garment_condition(Some(1.0))
            .unwrap();
        let (world, second) = world.with_citizen("Partner", second).unwrap();
        let AgentKind::Citizen(worker) = &world.agents[&first].kind;
        let home = worker.home;
        let (world, family) = world
            .with_household("Carpenters", home, &[first, second])
            .unwrap();
        let world = world
            .with_stored_good(
                store,
                crate::storage::GoodsOwner::Household(family),
                Good::Wood,
                1_000,
            )
            .unwrap();
        let project = world.construction.contracts[&old].project;
        let (world, id) = world
            .continue_construction(client, project, 100_000)
            .unwrap();
        let quotes = &world.construction.contracts[&id].quotes;
        assert_eq!(quotes.len(), 2);
        assert_eq!(
            quotes.iter().map(|quote| quote.shared_wood).sum::<u64>(),
            1_000
        );
        assert_eq!(
            quotes
                .iter()
                .filter(|quote| quote.procurement_cash > 0)
                .count(),
            1
        );
    }
}
