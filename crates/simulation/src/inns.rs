use crate::{
    AgentId, AgentKind, Citizen, CitizenAction, Coins, SimulationError, StartingRole, Universe,
    construction::{ConstructionItem, HOUR_MS},
    construction_finance::ceil_coins,
    locations::{ObjectId, ObjectKind, Place, PlaceId},
    marketplace::DAY_MS,
};
use imbl::HashMap;

const WINDOW_MS: u64 = 3 * DAY_MS;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Booking {
    pub household: AgentId,
    pub place: PlaceId,
    pub bed: ObjectId,
    pub payment: Coins,
    pub accepted_at_ms: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccupancyInterval {
    pub start_ms: u64,
    pub end_ms: u64,
    pub available_beds: u64,
    pub sleeping_guests: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Inn {
    pub household: AgentId,
    pub beds: Vec<ObjectId>,
    pub session_price: f64,
    pub maintenance_floor: f64,
    pub last_priced_day: u64,
    pub occupancy: Vec<OccupancyInterval>,
}

impl Inn {
    pub fn payment(&self) -> Result<Coins, SimulationError> {
        ceil_coins(self.session_price.max(self.maintenance_floor))
    }

    pub fn occupancy_since(&self, now: u64) -> (u128, u128) {
        let start = now.saturating_sub(WINDOW_MS);
        self.occupancy
            .iter()
            .fold((0, 0), |(paid, available), interval| {
                let elapsed = interval
                    .end_ms
                    .min(now)
                    .saturating_sub(interval.start_ms.max(start));
                (
                    paid + u128::from(elapsed) * u128::from(interval.sleeping_guests),
                    available + u128::from(elapsed) * u128::from(interval.available_beds),
                )
            })
    }

    fn adapt_price(&mut self, now: u64) {
        let (paid, available) = self.occupancy_since(now);
        let factor = if available == 0 {
            1.0
        } else if paid * 5 > available * 4 {
            1.05
        } else if paid * 5 < available * 3 {
            0.95
        } else {
            1.0
        };
        self.session_price = (self.session_price * factor).max(self.maintenance_floor);
        self.last_priced_day = now / DAY_MS;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BedOffer {
    pub household: AgentId,
    pub place: PlaceId,
    pub bed: ObjectId,
    pub payment: Coins,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InnCosts {
    pub maintenance_daily: f64,
    pub earnings_daily: f64,
    pub sessions_per_day: f64,
    pub maintenance_floor: f64,
    pub starting_price: f64,
}

pub(crate) fn usable_bed(site: &Place, household: AgentId, bed: ObjectId) -> bool {
    !site.kind.is_town_owned()
        && !site.kind.is_public()
        && site.buildings.iter().any(|building| {
            building.owner == Some(household)
                && building.rooms.iter().any(|room| {
                    room.condition.condition_units() > 0
                        && room.objects.iter().any(|object| {
                            object.id == bed
                                && object.kind == ObjectKind::Bed { resident: None }
                                && object.condition.condition_units() > 0
                        })
                })
        })
}

impl Citizen {
    pub fn inn_booking(&self) -> Option<Booking> {
        self.inn_booking
    }

    pub(crate) fn travel_place(&self, id: PlaceId) -> Result<&Place, SimulationError> {
        if self.inn_booking.is_some_and(|booking| booking.place == id) {
            self.map.place(id)
        } else {
            self.accessible_place(id)
        }
    }
}

impl Universe {
    pub fn inns(&self) -> &HashMap<PlaceId, Inn> {
        &self.inns
    }

    fn innkeeper_available(&self, household: AgentId) -> bool {
        self.households.get(&household).is_some_and(|family| {
            family.members.iter().any(|id| {
                self.agents.get(id).is_some_and(|agent| {
                    let AgentKind::Citizen(citizen) = &agent.kind;
                    citizen.starting_role() == Some(StartingRole::Innkeeper)
                })
            })
        })
    }

    pub fn inn_costs(
        &self,
        household: AgentId,
        place: PlaceId,
        beds: &[ObjectId],
    ) -> Result<InnCosts, SimulationError> {
        let family = self
            .households
            .get(&household)
            .ok_or(SimulationError::AgentNotFound)?;
        let site = self.map.place(place)?;
        let mut maintenance_daily = 0.0;
        for bed in beds {
            if !usable_bed(site, household, *bed) {
                return Err(SimulationError::MissingProperty);
            }
            let room = site
                .buildings
                .iter()
                .flat_map(|b| &b.rooms)
                .find(|room| room.objects.iter().any(|object| object.id == *bed))
                .unwrap();
            let room_beds = room
                .objects
                .iter()
                .filter(|o| matches!(o.kind, ObjectKind::Bed { .. }))
                .count();
            let replacement = |item: ConstructionItem| {
                self.construction_estimate(
                    household,
                    place,
                    item.base_hours() * HOUR_MS,
                    item.wood(),
                )
            };
            let room_value =
                replacement(ConstructionItem::Room(room.size))? as f64 / room_beds as f64;
            let bed_value = replacement(ConstructionItem::Bed)? as f64;
            // Share the room between its beds; damaged rooms accelerate object decay.
            maintenance_daily += (room_value + bed_value * (5.0 - 4.0 * room.condition.fraction()))
                / (crate::condition::CONDITION_LIFETIME_MS / DAY_MS) as f64;
        }
        let budget = self.household_budget(household, 0)?;
        let innkeeper_hours = family
            .members
            .iter()
            .filter(|id| {
                let AgentKind::Citizen(citizen) = &self.agents[*id].kind;
                citizen.starting_role() == Some(StartingRole::Innkeeper)
            })
            .map(|id| self.expected_earning_hours(*id))
            .sum::<f64>();
        let earnings_daily = budget.hourly_earnings_target * innkeeper_hours;
        let sessions_per_day = beds.len() as f64 * DAY_MS as f64 / crate::SLEEP_DURATION_MS as f64;
        Ok(InnCosts {
            maintenance_daily,
            earnings_daily,
            sessions_per_day,
            maintenance_floor: if beds.is_empty() {
                0.0
            } else {
                maintenance_daily / sessions_per_day
            },
            starting_price: if beds.is_empty() {
                0.0
            } else {
                (maintenance_daily + earnings_daily) / (0.7 * sessions_per_day)
            },
        })
    }

    /// Only existing unassigned beds can be offered. An empty list closes the inn.
    pub fn configure_inn_beds(
        &self,
        household: AgentId,
        place: PlaceId,
        mut beds: Vec<ObjectId>,
    ) -> Result<Self, SimulationError> {
        if !self.innkeeper_available(household) {
            return Err(SimulationError::MissingSkill);
        }
        if self
            .inns
            .get(&place)
            .is_some_and(|inn| inn.household != household)
        {
            return Err(SimulationError::PrivateProperty);
        }
        beds.sort_by_key(|id| id.0);
        if beds.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SimulationError::InvalidQuantity);
        }
        let site = self.map.place(place)?;
        if !site
            .buildings
            .iter()
            .any(|building| building.owner == Some(household))
        {
            return Err(SimulationError::PrivateProperty);
        }
        let costs = self.inn_costs(household, place, &beds)?;
        let mut world = self.clone();
        let inn = world.inns.entry(place).or_insert(Inn {
            household,
            beds: Vec::new(),
            session_price: costs.starting_price,
            maintenance_floor: costs.maintenance_floor,
            last_priced_day: self.current_time_ms / DAY_MS,
            occupancy: Vec::new(),
        });
        if inn.session_price == 0.0 {
            inn.session_price = costs.starting_price;
        }
        inn.beds = beds;
        inn.maintenance_floor = costs.maintenance_floor;
        inn.session_price = inn.session_price.max(inn.maintenance_floor);
        world.refresh_market();
        Ok(world)
    }

    /// Quotes are read-only; acceptance rechecks the exact bed and payment.
    pub fn inn_offers(&self, guest: AgentId) -> Result<Vec<BedOffer>, SimulationError> {
        let AgentKind::Citizen(citizen) = &self
            .agents
            .get(&guest)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        let mut offers = Vec::new();
        for (place, inn) in &self.inns {
            if inn.household == citizen.economic_id() || !self.innkeeper_available(inn.household) {
                continue;
            }
            let site = self.map.place(*place)?;
            let payment = inn.payment()?;
            for bed in &inn.beds {
                if usable_bed(site, inn.household, *bed)
                    && site
                        .object(*bed)
                        .is_some_and(|object| object.workers.is_empty())
                    && !self.agents.values().any(|agent| {
                        let AgentKind::Citizen(c) = &agent.kind;
                        c.inn_booking
                            .is_some_and(|b| b.place == *place && b.bed == *bed)
                    })
                {
                    offers.push(BedOffer {
                        household: inn.household,
                        place: *place,
                        bed: *bed,
                        payment,
                    });
                }
            }
        }
        offers.sort_by_key(|offer| (offer.payment, offer.place.0, offer.bed.0));
        Ok(offers)
    }

    /// Payment is upfront for one eight-hour session and is not refunded.
    pub fn book_inn_bed(&self, guest: AgentId, offer: BedOffer) -> Result<Self, SimulationError> {
        let mut world = self.clone();
        world.refresh_construction_context()?;
        let AgentKind::Citizen(citizen) = &world
            .agents
            .get(&guest)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        if citizen.active_action.is_some()
            || citizen.active_plan.is_some()
            || citizen.inn_booking.is_some()
        {
            return Err(SimulationError::CitizenBusy);
        }
        if !world.inn_offers(guest)?.contains(&offer) {
            return Err(SimulationError::MissingInputs);
        }
        let payer = citizen.economic_id();
        let commitments = world.construction_commitments(payer, None)?;
        if citizen
            .coins
            .saturating_sub(citizen.construction_protected.max(commitments))
            .max(0)
            < offer.payment
        {
            return Err(SimulationError::MissingInputs);
        }
        let balance = world
            .account_coins(offer.household)?
            .checked_add(offer.payment)
            .ok_or(SimulationError::WealthOverflow)?;
        world.set_account_coins(payer, world.account_coins(payer)? - offer.payment)?;
        world.set_account_coins(offer.household, balance)?;
        for (account, receipt) in [(payer, false), (offer.household, true)] {
            let history = &mut world.household_histories.get_mut(&account).unwrap().current;
            let total = if receipt {
                &mut history.inn_receipts
            } else {
                &mut history.inn_payments
            };
            *total = total
                .checked_add(offer.payment)
                .ok_or(SimulationError::WealthOverflow)?;
        }
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&guest).unwrap().kind;
        citizen.inn_booking = Some(Booking {
            household: offer.household,
            place: offer.place,
            bed: offer.bed,
            payment: offer.payment,
            accepted_at_ms: world.current_time_ms,
        });
        world.refresh_market();
        let AgentKind::Citizen(citizen) = &world.agents[&guest].kind;
        let action = if citizen.position == world.map.position(offer.place) {
            CitizenAction::Sleep
        } else {
            CitizenAction::Travel(offer.place)
        };
        world = world.start_action(guest, action)?;
        Ok(world)
    }

    pub fn cancel_inn_booking(&self, guest: AgentId) -> Result<Self, SimulationError> {
        let mut world = self.clone();
        let AgentKind::Citizen(citizen) = &mut world
            .agents
            .get_mut(&guest)
            .ok_or(SimulationError::AgentNotFound)?
            .kind;
        if citizen.inn_booking.is_some() {
            citizen.map = world.map.clone();
            citizen.abandon_action()?;
            world.map = citizen.map.clone();
        }
        world.refresh_market();
        Ok(world)
    }

    pub(crate) fn resume_inn_sleep(&mut self, id: AgentId) -> Result<(), SimulationError> {
        let AgentKind::Citizen(citizen) = &self.agents[&id].kind;
        if let Some(booking) = citizen.inn_booking
            && citizen.active_action.is_none()
            && citizen.position == self.map.position(booking.place)
        {
            *self = self.start_action(id, CitizenAction::Sleep)?;
        }
        Ok(())
    }

    pub(crate) fn clean_inns(&mut self) {
        if self.inns.is_empty() {
            return;
        }
        self.inns.retain(|place, inn| {
            self.households.contains_key(&inn.household)
                && self.map.place(*place).is_ok_and(|site| {
                    site.buildings
                        .iter()
                        .any(|b| b.owner == Some(inn.household))
                })
        });
        for (place, inn) in self.inns.iter_mut() {
            let Ok(site) = self.map.place(*place) else {
                continue;
            };
            inn.beds.retain(|bed| {
                site.object_owner(*bed) == Some(inn.household)
                    && site
                        .object(*bed)
                        .is_some_and(|o| o.kind == ObjectKind::Bed { resident: None })
            });
        }
        let invalid: Vec<_> = self
            .agents
            .iter()
            .filter_map(|(id, agent)| {
                let AgentKind::Citizen(citizen) = &agent.kind;
                let booking = citizen.inn_booking?;
                let valid = self.inns.get(&booking.place).is_some_and(|inn| {
                    inn.household == booking.household
                        && inn.beds.contains(&booking.bed)
                        && self.innkeeper_available(inn.household)
                        && self
                            .map
                            .place(booking.place)
                            .is_ok_and(|site| usable_bed(site, inn.household, booking.bed))
                });
                (!valid).then_some(*id)
            })
            .collect();
        for id in invalid {
            let AgentKind::Citizen(citizen) = &mut self.agents.get_mut(&id).unwrap().kind;
            if let Some(b) = citizen.inn_booking {
                self.map.release_objects(b.place, [Some(b.bed), None], id);
            }
            citizen.inn_booking = None;
            citizen.active_action = None;
            citizen.active_plan = None;
        }
    }

    pub(crate) fn inn_usability_boundary(&self, elapsed: u64) -> u64 {
        if self.inns.is_empty() {
            return elapsed;
        }
        let usable: Vec<_> = self
            .inns
            .iter()
            .flat_map(|(place, inn)| {
                inn.beds.iter().filter_map(|bed| {
                    self.map
                        .place(*place)
                        .ok()
                        .filter(|site| usable_bed(site, inn.household, *bed))
                        .map(|_| (*place, inn.household, *bed))
                })
            })
            .collect();
        if usable.is_empty() {
            return elapsed;
        }
        let all_usable = |time| {
            let mut map = self.map.clone();
            let (rooms, objects) = self.construction_repair_credits(time);
            map.advance_conditions(time, &rooms, &objects);
            usable.iter().all(|(place, household, bed)| {
                map.place(*place)
                    .is_ok_and(|site| usable_bed(site, *household, *bed))
            })
        };
        if all_usable(elapsed) {
            return elapsed;
        }
        // Stop when a bed closes so neither capacity nor recovery includes closed time.
        let (mut low, mut high) = (1, elapsed);
        while low < high {
            let middle = low + (high - low) / 2;
            if all_usable(middle) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    pub(crate) fn record_inn_time(&mut self, elapsed: u64) -> Result<(), SimulationError> {
        let now = self.current_time_ms;
        let observations: Vec<_> = self
            .inns
            .iter()
            .map(|(place, inn)| {
                let available_beds = if self.innkeeper_available(inn.household) {
                    self.map.place(*place).map_or(0, |site| {
                        inn.beds
                            .iter()
                            .filter(|bed| usable_bed(site, inn.household, **bed))
                            .count() as u64
                    })
                } else {
                    0
                };
                let sleeping_guests = self
                    .agents
                    .values()
                    .filter(|agent| {
                        let AgentKind::Citizen(c) = &agent.kind;
                        c.inn_booking.is_some_and(|b| b.place == *place)
                            && c.active_action.is_some_and(|a| {
                                a.action() == CitizenAction::Sleep
                                    && a.claimed_objects[0] == c.inn_booking.map(|b| b.bed)
                            })
                    })
                    .count() as u64;
                (*place, available_beds, sleeping_guests)
            })
            .collect();
        for (place, available_beds, sleeping_guests) in observations {
            let inn = self.inns.get_mut(&place).unwrap();
            let interval = OccupancyInterval {
                start_ms: now,
                end_ms: now + elapsed,
                available_beds,
                sleeping_guests,
            };
            if let Some(last) = inn.occupancy.last_mut()
                && last.end_ms == now
                && last.available_beds == available_beds
                && last.sleeping_guests == sleeping_guests
            {
                last.end_ms = interval.end_ms;
            } else {
                inn.occupancy.push(interval);
            }
            let cutoff = (now + elapsed).saturating_sub(WINDOW_MS);
            inn.occupancy.retain(|i| i.end_ms > cutoff);
            if let Some(first) = inn.occupancy.first_mut() {
                first.start_ms = first.start_ms.max(cutoff);
            }
        }
        Ok(())
    }

    pub(crate) fn update_inn_prices(&mut self) -> Result<(), SimulationError> {
        let now = self.current_time_ms;
        let due: Vec<_> = self
            .inns
            .iter()
            .filter(|(_, inn)| inn.last_priced_day < now / DAY_MS)
            .map(|(place, _)| *place)
            .collect();
        for place in due {
            let inn = &self.inns[&place];
            let usable: Vec<_> = inn
                .beds
                .iter()
                .copied()
                .filter(|bed| {
                    self.map
                        .place(place)
                        .is_ok_and(|site| usable_bed(site, inn.household, *bed))
                })
                .collect();
            let costs = self.inn_costs(inn.household, place, &usable)?;
            let inn = self.inns.get_mut(&place).unwrap();
            inn.maintenance_floor = costs.maintenance_floor;
            inn.adapt_price(now);
            inn.payment()?;
        }
        Ok(())
    }

    pub(crate) fn validate_inns(&self) -> Result<(), crate::persistence::SaveError> {
        let invalid = || crate::persistence::SaveError::InvalidState("invalid inn or booking");
        for (place, inn) in &self.inns {
            let site = self.map.place(*place).map_err(|_| invalid())?;
            if !self.households.contains_key(&inn.household)
                || !site
                    .buildings
                    .iter()
                    .any(|b| b.owner == Some(inn.household))
                || !inn.session_price.is_finite()
                || !inn.maintenance_floor.is_finite()
                || inn.maintenance_floor < 0.0
                || inn.session_price < inn.maintenance_floor
                || inn.payment().is_err()
                || inn.last_priced_day != self.current_time_ms / DAY_MS
                || inn.beds.iter().enumerate().any(|(i, bed)| {
                    inn.beds[..i].contains(bed)
                        || site
                            .object(*bed)
                            .is_none_or(|object| object.kind != ObjectKind::Bed { resident: None })
                        || site.object_owner(*bed) != Some(inn.household)
                })
            {
                return Err(invalid());
            }
            let mut previous = None;
            for interval in &inn.occupancy {
                if interval.start_ms >= interval.end_ms
                    || interval.end_ms > self.current_time_ms
                    || interval.start_ms < self.current_time_ms.saturating_sub(WINDOW_MS)
                    || interval.sleeping_guests > interval.available_beds
                    || previous.is_some_and(|end| end != interval.start_ms)
                {
                    return Err(invalid());
                }
                previous = Some(interval.end_ms);
            }
            if previous.is_some_and(|end| end != self.current_time_ms) {
                return Err(invalid());
            }
        }
        let mut beds = std::collections::HashSet::new();
        for agent in self.agents.values() {
            let AgentKind::Citizen(citizen) = &agent.kind;
            let Some(booking) = citizen.inn_booking else {
                continue;
            };
            let inn = self.inns.get(&booking.place).ok_or_else(invalid)?;
            let site = self.map.place(booking.place).map_err(|_| invalid())?;
            if booking.household != inn.household
                || citizen.economic_id() == inn.household
                || !self.innkeeper_available(inn.household)
                || !inn.beds.contains(&booking.bed)
                || !usable_bed(site, inn.household, booking.bed)
                || booking.payment < 0
                || booking.accepted_at_ms > self.current_time_ms
                || !beds.insert((booking.place, booking.bed))
                || !citizen
                    .active_action
                    .is_some_and(|active| match active.action() {
                        CitizenAction::Sleep => active.claimed_objects[0] == Some(booking.bed),
                        CitizenAction::Travel(place) => place == booking.place,
                        _ => false,
                    })
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        locations::{Map, Object, Position, Room, RoomId, RoomSize},
        marketplace::Good,
        storage::GoodsOwner,
    };

    fn citizen(world: &Universe, id: AgentId) -> &Citizen {
        let AgentKind::Citizen(citizen) = &world.agents[&id].kind;
        citizen
    }

    fn round_trip(world: &Universe) {
        assert_eq!(
            *world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }

    fn host(
        world: Universe,
        name: &str,
        count: usize,
    ) -> (Universe, AgentId, PlaceId, Vec<ObjectId>) {
        let (world, host) = world
            .with_citizen(
                name,
                Citizen::with_needs(-50.0, 0.0)
                    .unwrap()
                    .with_coins(0)
                    .unwrap()
                    .with_starting_role(StartingRole::Innkeeper),
            )
            .unwrap();
        let mut world = world.with_member_accommodation(host).unwrap();
        let household = citizen(&world, host).economic_id();
        let home = citizen(&world, host).home().unwrap();
        let objects: Vec<_> = (0..count)
            .map(|_| Object::new(ObjectKind::Bed { resident: None }))
            .collect();
        let beds = objects.iter().map(|o| o.id).collect::<Vec<_>>();
        world.map.places_mut().get_mut(&home).unwrap().buildings[0]
            .rooms
            .push(Room {
                id: RoomId(uuid::Uuid::new_v4()),
                name: "Guests".into(),
                size: RoomSize::Large,
                condition: Default::default(),
                objects,
            });
        world.refresh_market();
        world = world
            .configure_inn_beds(household, home, beds.clone())
            .unwrap();
        (world, host, home, beds)
    }

    fn guest(world: Universe, funds: Coins) -> (Universe, AgentId) {
        world
            .with_citizen(
                "Guest",
                Citizen::with_needs(-50.0, 100.0)
                    .unwrap()
                    .with_coins(funds)
                    .unwrap(),
            )
            .unwrap()
    }

    fn fixture(count: usize) -> (Universe, AgentId, AgentId, PlaceId, Vec<ObjectId>) {
        let (world, host, place, beds) = host(Universe::with_map(Map::default()), "Inn", count);
        let (mut world, guest) = guest(world, 100000);
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&guest).unwrap().kind;
        citizen.discomfort = 50.0;
        (world, host, guest, place, beds)
    }

    #[test]
    fn role_and_configuration_never_create_assets_and_private_beds_stay_private() {
        let world = Universe::with_map(Map::default());
        let before = world.map.clone();
        let (world, id) = world
            .with_citizen(
                "Innkeeper",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Innkeeper),
            )
            .unwrap();
        assert_eq!(world.map, before);
        assert!(world.inns.is_empty());
        assert!(citizen(&world, id).skills.iter().all(Option::is_none));
        let (world, host, _, place, beds) = fixture(2);
        let family = citizen(&world, host).economic_id();
        let private = world
            .map
            .place(place)
            .unwrap()
            .objects()
            .find(|o| {
                o.kind
                    == ObjectKind::Bed {
                        resident: Some(host),
                    }
            })
            .unwrap()
            .id;
        assert!(
            world
                .configure_inn_beds(family, place, vec![private])
                .is_err()
        );
        assert!(
            world
                .configure_inn_beds(family, place, vec![beds[0], beds[0]])
                .is_err()
        );
        assert_eq!(
            world.map,
            world.configure_inn_beds(family, place, beds).unwrap().map
        );
        round_trip(&world);
    }

    #[test]
    fn costs_include_room_bed_and_unskilled_labour_with_consistent_session_units() {
        let (world, host, _, place, beds) = fixture(2);
        let family = citizen(&world, host).economic_id();
        let costs = world.inn_costs(family, place, &beds).unwrap();
        let estimate = |item: ConstructionItem| {
            world
                .construction_estimate(family, place, item.base_hours() * HOUR_MS, item.wood())
                .unwrap() as f64
        };
        close(
            costs.maintenance_daily,
            (estimate(ConstructionItem::Room(RoomSize::Large))
                + 2.0 * estimate(ConstructionItem::Bed))
                / 200.0,
        );
        assert_eq!(costs.sessions_per_day, 6.0);
        close(costs.maintenance_floor * 6.0, costs.maintenance_daily);
        close(
            costs.starting_price * 6.0 * 0.7,
            costs.maintenance_daily + costs.earnings_daily,
        );
        assert!(
            estimate(ConstructionItem::Bed)
                > ConstructionItem::Bed.wood() as f64
                    * world.construction_estimated_unit_price(family, Good::Wood)
        );
        close(world.inns[&place].session_price, costs.starting_price);
        let one = world.inn_costs(family, place, &beds[..1]).unwrap();
        close(one.maintenance_daily * 2.0, costs.maintenance_daily);
        assert!(one.starting_price > costs.starting_price);
    }

    #[test]
    fn strict_thresholds_floor_fractional_adaptation_and_integer_payments() {
        let (world, _, _, place, _) = fixture(1);
        for (paid, factor) in [(59, 0.95), (60, 1.0), (70, 1.0), (80, 1.0), (81, 1.05)] {
            let mut inn = world.inns[&place].clone();
            inn.session_price = 10.1;
            inn.maintenance_floor = 0.01;
            inn.occupancy = vec![OccupancyInterval {
                start_ms: 0,
                end_ms: 100,
                available_beds: 100,
                sleeping_guests: paid,
            }];
            inn.adapt_price(DAY_MS);
            close(inn.session_price, 10.1 * factor);
            assert_eq!(inn.payment().unwrap(), (10.1 * factor).ceil() as Coins);
        }
        let mut inn = world.inns[&place].clone();
        inn.session_price = 1.01;
        inn.maintenance_floor = 0.1;
        inn.occupancy = vec![OccupancyInterval {
            start_ms: 0,
            end_ms: DAY_MS,
            available_beds: 1,
            sleeping_guests: 1,
        }];
        for _ in 0..16 {
            inn.adapt_price(DAY_MS);
        }
        assert!(inn.payment().unwrap() > 2);
        inn.occupancy[0].sleeping_guests = 0;
        for _ in 0..200 {
            inn.adapt_price(DAY_MS);
        }
        close(inn.session_price, 0.1);
        assert_eq!(inn.payment().unwrap(), 1);
        inn.occupancy.clear();
        let price = inn.session_price;
        inn.adapt_price(2 * DAY_MS);
        assert_eq!(inn.session_price, price);
    }

    #[test]
    fn booking_locks_one_bed_and_upfront_payment_conserves_household_cash() {
        let (world, host, first, place, _) = fixture(1);
        let (world, second) = guest(world, 100000);
        let quote = world.inn_offers(first).unwrap()[0];
        let original = world.clone();
        assert_eq!(
            world.inn_offers(first).unwrap(),
            world.inn_offers(second).unwrap()
        );
        assert_eq!(world, original);
        let host_account = citizen(&world, host).economic_id();
        let payer = citizen(&world, first).economic_id();
        let before = world.households[&host_account].coins + world.households[&payer].coins;
        let booked = world.book_inn_bed(first, quote).unwrap();
        assert!(booked.book_inn_bed(second, quote).is_err());
        assert!(booked.inn_offers(second).unwrap().is_empty());
        assert_eq!(
            booked.households[&host_account].coins + booked.households[&payer].coins,
            before
        );
        assert_eq!(
            booked.household_histories[&payer].current.inn_payments,
            quote.payment
        );
        assert_eq!(
            booked.household_histories[&host_account]
                .current
                .inn_receipts,
            quote.payment
        );
        assert_eq!(
            citizen(&booked, first).inn_booking().unwrap().payment,
            quote.payment
        );
        assert_eq!(booked.inns[&place].occupancy_since(0), (0, 0));
        round_trip(&booked);
    }

    #[test]
    fn stale_quotes_funds_and_shared_household_races_recheck_at_acceptance() {
        let (world, _, _, place, _) = fixture(2);
        let (world, first) = guest(world, 0);
        let quote = world.inn_offers(first).unwrap()[0];
        assert!(world.book_inn_bed(first, quote).is_err());
        let mut funded = world.clone();
        let family = citizen(&funded, first).economic_id();
        funded.set_account_coins(family, quote.payment).unwrap();
        funded.refresh_market();
        let (funded, second) = funded
            .with_household_member(family, "Sibling", Citizen::new(0.0).unwrap())
            .unwrap();
        let other = funded
            .inn_offers(second)
            .unwrap()
            .into_iter()
            .find(|q| q.bed != quote.bed)
            .unwrap();
        let booked = funded.book_inn_bed(first, quote).unwrap();
        assert!(booked.book_inn_bed(second, other).is_err());
        let mut repriced = funded.clone();
        repriced.inns.get_mut(&place).unwrap().session_price += 2.0;
        assert!(repriced.book_inn_bed(first, quote).is_err());
    }

    #[test]
    fn paid_sleep_has_normal_recovery_and_bed_discomfort_relief_without_storage_access() {
        let (world, host, guest, place, _) = fixture(1);
        let owner = citizen(&world, host).economic_id();
        let world = world
            .with_stored_good(place, GoodsOwner::Household(owner), Good::BreadRoll, 20)
            .unwrap();
        assert_eq!(
            citizen(&world, host).stored_available_units(place, Good::BreadRoll),
            20
        );
        let quote = world.inn_offers(guest).unwrap()[0];
        let sleeping = world.book_inn_bed(guest, quote).unwrap();
        let c = citizen(&sleeping, guest);
        assert!(c.accessible_place(place).is_err());
        assert!(c.travel_place(place).is_ok());
        assert!(
            sleeping
                .map
                .place(place)
                .unwrap()
                .objects()
                .all(|o| !c.may_use_object(sleeping.map.place(place).unwrap(), o))
        );
        assert_eq!(c.stored_available_units(place, Good::BreadRoll), 0);
        assert_eq!(c.goods_owner(), GoodsOwner::Household(c.economic_id()));
        assert_ne!(c.economic_id(), owner);
        let outside = world
            .start_action(guest, CitizenAction::Sleep)
            .unwrap()
            .advance(HOUR_MS)
            .unwrap();
        let inside = sleeping.advance(HOUR_MS).unwrap();
        close(
            citizen(&inside, guest).tiredness(),
            citizen(&outside, guest).tiredness(),
        );
        close(
            citizen(&inside, guest).discomfort(),
            50.0 - 25.0 / 8.0 - 10.0 / 24.0,
        );
        assert_eq!(
            inside.inns[&place].occupancy_since(HOUR_MS),
            (u128::from(HOUR_MS), u128::from(HOUR_MS))
        );
        let finished = inside.advance(7 * HOUR_MS).unwrap();
        assert!(citizen(&finished, guest).inn_booking().is_none());
        assert!(
            finished
                .map
                .place(place)
                .unwrap()
                .object(quote.bed)
                .unwrap()
                .workers
                .is_empty()
        );
        assert_eq!(finished.inn_offers(guest).unwrap().len(), 1);
        round_trip(&finished);
    }

    #[test]
    fn travel_is_reserved_but_not_paid_occupancy_and_cancel_has_no_refund() {
        let (mut world, _, guest, place, _) = fixture(1);
        let AgentKind::Citizen(c) = &mut world.agents.get_mut(&guest).unwrap().kind;
        c.position = Position { x: 100.0, y: 0.0 };
        let quote = world.inn_offers(guest).unwrap()[0];
        let booked = world.book_inn_bed(guest, quote).unwrap();
        let duration = citizen(&booked, guest)
            .active_action()
            .unwrap()
            .duration_ms();
        assert_eq!(
            citizen(&booked, guest).active_action().unwrap().action(),
            CitizenAction::Travel(place)
        );
        round_trip(&booked);
        let travelling = booked.advance(duration / 2).unwrap();
        assert_eq!(travelling.inns[&place].occupancy_since(duration / 2).0, 0);
        let cancelled = travelling.cancel_inn_booking(guest).unwrap();
        assert_eq!(
            citizen(&cancelled, guest).coins(),
            citizen(&travelling, guest).coins()
        );
        assert!(citizen(&cancelled, guest).active_action().is_none());
        assert_eq!(cancelled.inn_offers(guest).unwrap().len(), 1);
        round_trip(&cancelled);
        let arrived = booked.advance(duration).unwrap();
        assert_eq!(
            citizen(&arrived, guest).active_action().unwrap().action(),
            CitizenAction::Sleep
        );
        assert_eq!(arrived.inns[&place].occupancy_since(duration).0, 0);
        let slept = arrived.advance(HOUR_MS).unwrap();
        assert_eq!(
            slept.inns[&place].occupancy_since(duration + HOUR_MS).0,
            u128::from(HOUR_MS)
        );
        round_trip(&slept);
    }

    #[test]
    fn interruption_and_guest_or_host_departure_release_claims_and_round_trip() {
        let (world, host, guest, place, _) = fixture(1);
        let quote = world.inn_offers(guest).unwrap()[0];
        let sleeping = world
            .book_inn_bed(guest, quote)
            .unwrap()
            .advance(HOUR_MS)
            .unwrap();
        let interrupted = sleeping
            .cancel_inn_booking(guest)
            .unwrap()
            .advance(HOUR_MS)
            .unwrap();
        assert_eq!(
            interrupted.inns[&place].occupancy_since(2 * HOUR_MS).0,
            u128::from(HOUR_MS)
        );
        round_trip(&interrupted);
        let guest_gone = sleeping.remove_citizen(guest).unwrap();
        assert!(
            guest_gone
                .map
                .place(place)
                .unwrap()
                .object(quote.bed)
                .unwrap()
                .workers
                .is_empty()
        );
        round_trip(&guest_gone);
        let host_gone = sleeping.remove_citizen(host).unwrap();
        assert!(host_gone.inns.is_empty());
        assert!(citizen(&host_gone, guest).inn_booking().is_none());
        assert!(citizen(&host_gone, guest).active_action().is_none());
        round_trip(&host_gone);
        let hostless = host_gone.remove_citizen(guest).unwrap();
        round_trip(&hostless);
    }

    #[test]
    fn capacity_tracks_partial_days_bed_changes_closures_and_rolling_expiry() {
        let (world, host, guest, place, beds) = fixture(2);
        let family = citizen(&world, host).economic_id();
        let world = world.advance(6 * HOUR_MS).unwrap();
        let world = world
            .configure_inn_beds(family, place, beds[..1].to_vec())
            .unwrap()
            .advance(6 * HOUR_MS)
            .unwrap();
        let world = world
            .configure_inn_beds(family, place, vec![])
            .unwrap()
            .advance(6 * HOUR_MS)
            .unwrap();
        assert!(world.inn_offers(guest).unwrap().is_empty());
        let world = world
            .configure_inn_beds(family, place, beds[..1].to_vec())
            .unwrap()
            .advance(6 * HOUR_MS)
            .unwrap();
        assert_eq!(
            world.inns[&place].occupancy_since(DAY_MS),
            (0, u128::from(24 * HOUR_MS))
        );
        let expired = world.advance(3 * DAY_MS).unwrap();
        assert_eq!(
            expired.inns[&place].occupancy_since(4 * DAY_MS),
            (0, u128::from(3 * DAY_MS))
        );
        assert!(
            expired.inns[&place]
                .occupancy
                .iter()
                .all(|i| i.start_ms >= DAY_MS)
        );
        round_trip(&expired);
    }

    #[test]
    fn competing_inns_prices_and_actual_occupancy_are_independent() {
        let (world, host1, place1, _) = host(Universe::with_map(Map::default()), "First", 1);
        let (world, _, place2, _) = host(world, "Second", 1);
        let (mut world, guest) = guest(world, 100000);
        let price = world.inns[&place1].session_price;
        world.inns.get_mut(&place2).unwrap().session_price = price;
        assert_eq!(world.inn_offers(guest).unwrap().len(), 2);
        let quote = world
            .inn_offers(guest)
            .unwrap()
            .into_iter()
            .find(|o| o.place == place1)
            .unwrap();
        let sleeping = world
            .book_inn_bed(guest, quote)
            .unwrap()
            .advance(8 * HOUR_MS)
            .unwrap();
        assert_eq!(
            sleeping.inns[&place1].occupancy_since(8 * HOUR_MS).0,
            u128::from(8 * HOUR_MS)
        );
        assert_eq!(sleeping.inns[&place2].occupancy_since(8 * HOUR_MS).0, 0);
        let mut repriced = sleeping.clone();
        // A partial first day can be highly occupied, without inventing earlier capacity.
        repriced.inns.get_mut(&place1).unwrap().occupancy = vec![OccupancyInterval {
            start_ms: 0,
            end_ms: 8 * HOUR_MS,
            available_beds: 1,
            sleeping_guests: 1,
        }];
        repriced
            .inns
            .get_mut(&place1)
            .unwrap()
            .adapt_price(8 * HOUR_MS);
        close(repriced.inns[&place1].session_price, price * 1.05);
        close(repriced.inns[&place2].session_price, price);
        assert!(citizen(&sleeping, host1).home().is_some());
        round_trip(&sleeping);
    }

    #[test]
    fn paid_sleep_prediction_cannot_change_world_reservations_money_history_or_rng() {
        let (world, _, guest, _, _) = fixture(1);
        let offer = world.inn_offers(guest).unwrap()[0];
        let booked = world.book_inn_bed(guest, offer).unwrap();
        let original = booked.clone();
        let c = citizen(&booked, guest);
        let rng = c.forage_rng.clone();
        let predicted = c.advance_predicted(crate::SLEEP_DURATION_MS).unwrap();
        assert!(predicted.inn_booking.is_none());
        assert_eq!(predicted.forage_rng, rng);
        assert_eq!(booked, original);
        round_trip(&booked);
    }

    #[test]
    fn closing_reserved_beds_or_losing_operator_cancels_travel_and_sleep() {
        let (world, host, guest, place, _) = fixture(1);
        let offer = world.inn_offers(guest).unwrap()[0];
        let booked = world.book_inn_bed(guest, offer).unwrap();
        let family = citizen(&booked, host).economic_id();
        let closed = booked.configure_inn_beds(family, place, vec![]).unwrap();
        assert!(citizen(&closed, guest).inn_booking.is_none());
        assert!(
            closed
                .map
                .place(place)
                .unwrap()
                .object(offer.bed)
                .unwrap()
                .workers
                .is_empty()
        );
        round_trip(&closed);
        let (mut shared, member) = booked
            .with_household_member(family, "Remaining host", Citizen::new(0.0).unwrap())
            .unwrap();
        shared = shared.remove_citizen(host).unwrap();
        assert!(shared.agents.contains_key(&member));
        assert!(shared.inn_offers(guest).unwrap().is_empty());
        assert!(citizen(&shared, guest).inn_booking.is_none());
        round_trip(&shared);
    }

    #[test]
    fn save_rejects_invalid_prices_capacity_bookings_and_history() {
        let (world, _, guest, place, _) = fixture(1);
        let quote = world.inn_offers(guest).unwrap()[0];
        let booked = world
            .book_inn_bed(guest, quote)
            .unwrap()
            .advance(HOUR_MS)
            .unwrap();
        for price in [-1.0, f64::NAN, f64::INFINITY] {
            let mut invalid = booked.clone();
            invalid.inns.get_mut(&place).unwrap().session_price = price;
            assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        }
        let mut invalid = booked.clone();
        invalid.inns.get_mut(&place).unwrap().occupancy[0].sleeping_guests = 2;
        assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        let mut invalid = booked.clone();
        let AgentKind::Citizen(c) = &mut invalid.agents.get_mut(&guest).unwrap().kind;
        c.inn_booking.as_mut().unwrap().accepted_at_ms = HOUR_MS + 1;
        assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        let mut invalid = booked.clone();
        invalid.inns.get_mut(&place).unwrap().beds.clear();
        assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        round_trip(&booked);
    }

    #[test]
    fn real_daily_pricing_uses_partial_day_occupancy_independently_and_locks_payment() {
        let mut initial = Universe::starting_at(20 * HOUR_MS);
        initial.map = Map::default();
        let (world, _, first, _) = host(initial, "Busy", 1);
        let (world, _, second, _) = host(world, "Empty", 1);
        let (mut world, guest) = guest(world, 100000);
        let price = world.inns[&first].session_price;
        world.inns.get_mut(&second).unwrap().session_price = price;
        let offer = world
            .inn_offers(guest)
            .unwrap()
            .into_iter()
            .find(|o| o.place == first)
            .unwrap();
        let booked = world.book_inn_bed(guest, offer).unwrap();
        let midnight = booked.advance(4 * HOUR_MS).unwrap();
        close(midnight.inns[&first].session_price, price * 1.05);
        close(midnight.inns[&second].session_price, price * 0.95);
        assert_eq!(
            citizen(&midnight, guest).inn_booking.unwrap().payment,
            offer.payment
        );
        assert_eq!(
            midnight.inns[&first].occupancy_since(DAY_MS),
            (u128::from(4 * HOUR_MS), u128::from(4 * HOUR_MS))
        );
        let unchanged = midnight.advance(1000).unwrap();
        close(
            unchanged.inns[&first].session_price,
            midnight.inns[&first].session_price,
        );
        round_trip(&midnight);
        round_trip(&unchanged);
    }

    #[test]
    fn a_bed_reaching_zero_condition_closes_at_observed_time_and_interrupts_sleep() {
        let (mut world, _, guest, place, beds) = fixture(1);
        let object = world
            .map
            .places_mut()
            .get_mut(&place)
            .unwrap()
            .objects_mut()
            .find(|o| o.id == beds[0])
            .unwrap();
        object.condition =
            crate::condition::Condition::from_units(crate::condition::base_wear(HOUR_MS / 2))
                .unwrap();
        world.refresh_market();
        let quote = world.inn_offers(guest).unwrap()[0];
        let closing = world.inn_usability_boundary(HOUR_MS);
        assert!(closing > 0 && closing <= HOUR_MS / 2);
        let booked = world.book_inn_bed(guest, quote).unwrap();
        let closed = booked.advance(HOUR_MS).unwrap();
        assert_eq!(
            closed.inns[&place].occupancy_since(HOUR_MS),
            (u128::from(closing), u128::from(closing))
        );
        assert!(citizen(&closed, guest).inn_booking.is_none());
        assert!(citizen(&closed, guest).active_action.is_none());
        assert!(closed.inn_offers(guest).unwrap().is_empty());
        close(
            citizen(&closed, guest).discomfort(),
            50.0 - 25.0 * closing as f64 / crate::SLEEP_DURATION_MS as f64 - 10.0 / 24.0,
        );
        round_trip(&closed);
    }

    #[test]
    fn zero_cash_hosts_can_trade_overflow_is_atomic_and_reload_continues_paid_travel() {
        let (mut world, host, guest, place, _) = fixture(1);
        let family = citizen(&world, host).economic_id();
        assert_eq!(world.households[&family].coins, 0);
        let AgentKind::Citizen(c) = &mut world.agents.get_mut(&guest).unwrap().kind;
        c.position = Position { x: 100.0, y: 0.0 };
        let offer = world.inn_offers(guest).unwrap()[0];
        let booked = world.book_inn_bed(guest, offer).unwrap();
        let loaded = Universe::from_save_str(&booked.to_save_string().unwrap()).unwrap();
        assert_eq!(
            booked.advance(9 * HOUR_MS).unwrap(),
            loaded.advance(9 * HOUR_MS).unwrap()
        );
        let mut overflowing = world.clone();
        overflowing.set_account_coins(family, Coins::MAX).unwrap();
        overflowing.refresh_market();
        let original = overflowing.clone();
        assert_eq!(
            overflowing.book_inn_bed(guest, offer),
            Err(SimulationError::WealthOverflow)
        );
        assert_eq!(overflowing, original);
        let host_departed = booked.remove_citizen(host).unwrap();
        assert_eq!(host_departed.map.place(place).unwrap().owner, None);
        assert!(host_departed.inns.is_empty());
        assert!(citizen(&host_departed, guest).inn_booking.is_none());
        round_trip(&host_departed);
    }

    #[test]
    fn innkeeper_earnings_target_excludes_other_household_earners() {
        let (world, host, _, place, beds) = fixture(1);
        let family = citizen(&world, host).economic_id();
        let (world, other) = world
            .with_household_member(
                family,
                "Other earner",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Fisher),
            )
            .unwrap();
        let budget = world.household_budget(family, 0).unwrap();
        let costs = world.inn_costs(family, place, &beds).unwrap();
        close(
            costs.earnings_daily,
            budget.hourly_earnings_target * world.expected_earning_hours(host),
        );
        close(
            budget.earning_hours_per_day,
            world.expected_earning_hours(host) + world.expected_earning_hours(other),
        );
        assert!(
            costs.earnings_daily < budget.hourly_earnings_target * budget.earning_hours_per_day
        );
        assert!(world.inn_costs(host, place, &[]).is_err());
        round_trip(&world);
    }
}
