use crate::{
    Citizen, CitizenAction, Quantity, SimulationError,
    locations::PlaceId,
    marketplace::Good,
    production::{Recipe, Skill, skill_duration_reduction},
};

pub const FIELD_PLOTS: usize = 12;
pub const HOUR_MS: u64 = 3_600_000;
pub const GROWTH_MS: u64 = 96 * HOUR_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CropKind {
    Wheat,
    Flax,
    Cabbage,
}

impl CropKind {
    pub const ALL: [Self; 3] = [Self::Wheat, Self::Flax, Self::Cabbage];
    pub fn name(self) -> &'static str {
        match self {
            Self::Wheat => "Wheat",
            Self::Flax => "Flax",
            Self::Cabbage => "Cabbage",
        }
    }
    pub fn good(self) -> Good {
        match self {
            Self::Wheat => Good::Wheat,
            Self::Flax => Good::Flax,
            Self::Cabbage => Good::Cabbage,
        }
    }
    pub fn recipe(self) -> Recipe {
        match self {
            Self::Wheat => Recipe::GrowWheat,
            Self::Flax => Recipe::GrowFlax,
            Self::Cabbage => Recipe::GrowCabbage,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CropStatus {
    Growing,
    Ready,
    Dead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Crop {
    pub kind: CropKind,
    pub planted_at_ms: u64,
    pub tended: [bool; 2],
}

impl Crop {
    pub fn tend_window(self, stage: usize) -> Option<(u64, u64)> {
        let start = match stage {
            0 => 24,
            1 => 48,
            _ => return None,
        };
        Some((
            self.planted_at_ms.checked_add(start * HOUR_MS)?,
            self.planted_at_ms.checked_add((start + 24) * HOUR_MS)?,
        ))
    }
    pub fn ready_at_ms(self) -> u64 {
        self.planted_at_ms.saturating_add(GROWTH_MS)
    }
    pub fn status(self, now_ms: u64) -> CropStatus {
        if !self.tended.iter().any(|tended| *tended)
            && now_ms > self.planted_at_ms.saturating_add(72 * HOUR_MS)
        {
            CropStatus::Dead
        } else if now_ms >= self.ready_at_ms() {
            CropStatus::Ready
        } else {
            CropStatus::Growing
        }
    }
    pub fn expected_yield(self, now_ms: u64) -> Quantity {
        if self.status(now_ms) == CropStatus::Dead {
            0
        } else {
            let care = self.tended.into_iter().filter(|tended| *tended).count() as Quantity;
            match self.kind {
                CropKind::Cabbage => 2 + 3 * care,
                _ => 150 + 225 * care,
            }
        }
    }
    pub fn tend_missed(self, stage: usize, now_ms: u64) -> bool {
        self.tend_window(stage)
            .is_some_and(|(_, end)| !self.tended[stage] && now_ms > end)
    }
}

impl Citizen {
    pub fn current_time_ms(&self) -> u64 {
        self.market_time_ms
    }
    pub fn crop(&self, field: PlaceId, plot: usize) -> Result<Option<Crop>, SimulationError> {
        let place = self.map.place(field)?;
        if place.owner != Some(self.id) {
            return Err(SimulationError::PrivateProperty);
        }
        place
            .plots
            .get(plot)
            .copied()
            .ok_or(SimulationError::MissingProperty)
    }
    pub(crate) fn farming_duration_ms(
        &self,
        action: CitizenAction,
    ) -> Result<u64, SimulationError> {
        let (field, plot, base) = match action {
            CitizenAction::Plant { field, plot, .. } => (field, plot, HOUR_MS),
            CitizenAction::Tend { field, plot, .. } => (field, plot, HOUR_MS / 2),
            CitizenAction::Harvest { field, plot } => (field, plot, HOUR_MS),
            CitizenAction::Clear { field, plot } => (field, plot, HOUR_MS / 4),
            _ => return Err(SimulationError::MissingProperty),
        };
        let crop = self.crop(field, plot)?;
        let practice = self
            .skill_practice_ms(Skill::Farming)
            .ok_or(SimulationError::MissingSkill)?;
        let duration = (base as f64 * (1.0 - skill_duration_reduction(practice))).ceil() as u64;
        let finish = self
            .market_time_ms
            .checked_add(duration)
            .ok_or(SimulationError::TimeOverflow)?;
        match action {
            CitizenAction::Plant { .. } if crop.is_none() => {
                finish
                    .checked_add(GROWTH_MS)
                    .ok_or(SimulationError::TimeOverflow)?;
            }
            CitizenAction::Tend { stage, .. }
                if crop.is_some_and(|crop| {
                    crop.tend_window(stage).is_some_and(|(start, end)| {
                        !crop.tended[stage] && self.market_time_ms >= start && finish <= end
                    })
                }) => {}
            CitizenAction::Harvest { .. }
                if crop
                    .is_some_and(|crop| crop.status(self.market_time_ms) == CropStatus::Ready) => {}
            CitizenAction::Clear { .. }
                if crop
                    .is_some_and(|crop| crop.status(self.market_time_ms) == CropStatus::Dead) => {}
            _ => return Err(SimulationError::MissingInputs),
        }
        Ok(duration)
    }
    pub(crate) fn complete_farming(
        &mut self,
        action: CitizenAction,
        finish: u64,
    ) -> Result<(), SimulationError> {
        let (field, plot) = action.crop_plot().ok_or(SimulationError::MissingProperty)?;
        let crop = self.crop(field, plot)?;
        let next = match action {
            CitizenAction::Plant { crop: kind, .. } => Some(Crop {
                kind,
                planted_at_ms: finish,
                tended: [false; 2],
            }),
            CitizenAction::Tend { stage, .. } => {
                let mut crop = crop.ok_or(SimulationError::MissingInputs)?;
                crop.tended[stage] = true;
                Some(crop)
            }
            CitizenAction::Harvest { .. } => {
                let crop = crop.ok_or(SimulationError::MissingInputs)?;
                let good = crop.kind.good();
                let units = self
                    .units(good)
                    .checked_add(crop.expected_yield(finish))
                    .ok_or(SimulationError::InventoryOverflow)?;
                self.inventory.insert(good, units);
                None
            }
            CitizenAction::Clear { .. } => None,
            _ => return Err(SimulationError::MissingProperty),
        };
        self.map.set_crop(field, plot, next)?;
        Ok(())
    }
}

impl CitizenAction {
    pub fn crop_plot(self) -> Option<(PlaceId, usize)> {
        match self {
            Self::Plant { field, plot, .. }
            | Self::Tend { field, plot, .. }
            | Self::Harvest { field, plot }
            | Self::Clear { field, plot } => Some((field, plot)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locations::{Location, Position};

    fn farmer() -> (Citizen, PlaceId) {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_skill_practice_ms(Skill::Farming, Some(0))
            .unwrap();
        let (map, field) = citizen
            .map()
            .with_place(
                Location::Field,
                Position::default(),
                Some(citizen.id()),
                "Field",
            )
            .unwrap();
        (citizen.with_map(map).unwrap(), field)
    }
    fn perform(citizen: Citizen, action: CitizenAction) -> Citizen {
        let citizen = citizen.start_action(action).unwrap();
        citizen
            .advance(citizen.active_action().unwrap().remaining_ms())
            .unwrap()
    }
    #[test]
    fn cabbage_demand_and_whole_head_stock_drive_crop_allocation() {
        use crate::marketplace::ShoppingList;
        let (mut citizen, field) = farmer();
        citizen
            .market
            .set_request(citizen.id, ShoppingList::single(Good::Cabbage, 8))
            .unwrap();
        citizen.market.refresh_affordability(&[(citizen.id, 1000)]);
        assert_eq!(citizen.next_crop_kind(), Some(CropKind::Cabbage));
        assert_eq!(citizen.crop_demand()[0], 0.0);
        assert_eq!(citizen.crop_demand()[1], 0.0);
        assert!(citizen.crop_demand()[2] > 0.0);
        citizen = perform(
            citizen,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Cabbage,
            },
        );
        assert_eq!(
            citizen.crop(field, 0).unwrap().unwrap().kind,
            CropKind::Cabbage
        );
        citizen.inventory.insert(Good::Cabbage, 88);
        assert_eq!(citizen.next_crop_kind(), None);
    }

    #[test]
    fn planning_candidates_commit_crop_work_and_respect_future_capacity() {
        let (citizen, field) = farmer();
        let citizen = citizen
            .advance(6 * HOUR_MS)
            .unwrap()
            .with_good(Good::BreadRoll, 70)
            .unwrap()
            .with_coins(40)
            .unwrap();
        let plan = crate::planning::plan(&citizen).unwrap();
        assert!(!plan.actions().is_empty());
        let mut cropped = citizen.clone();
        let mut planted = 0;
        while let Some(action) = cropped.farming_actions().into_iter().next() {
            if !matches!(action, CitizenAction::Plant { .. }) {
                break;
            }
            cropped = perform(cropped, action);
            planted += 1;
            assert!(planted <= FIELD_PLOTS);
        }
        assert!(planted > 0);
        assert!(planted < FIELD_PLOTS);
        assert!(cropped.production_work_today_ms() <= crate::production::DAILY_CAPACITY_MS);
        let now = cropped.market_time_ms;
        let first = cropped.crop(field, 0).unwrap().unwrap();
        cropped = cropped
            .advance(first.planted_at_ms + 24 * HOUR_MS - now)
            .unwrap();
        assert!(
            cropped
                .farming_actions()
                .iter()
                .any(|action| matches!(action, CitizenAction::Tend { stage: 0, .. }))
        );
    }

    #[test]
    #[ignore = "manual multi-day farming viability probe"]
    fn town_farming_viability_probe() {
        use crate::{AgentKind, PlanningRuntime, StartingRole, Universe};
        let roles = [
            StartingRole::Farmer,
            StartingRole::Miller,
            StartingRole::Woodcutter,
            StartingRole::Baker,
            StartingRole::Weaver,
            StartingRole::Tailor,
        ];
        let mut universe = Universe::with_map(crate::locations::Map::random())
            .advance(6 * HOUR_MS)
            .unwrap();
        let mut ids = Vec::new();
        for role in roles.into_iter().cycle().take(24) {
            let mut citizen = Citizen::with_needs(-50.0, -66.6)
                .unwrap()
                .with_starting_role(role)
                .with_good(Good::BreadRoll, 70)
                .unwrap()
                .with_coins(crate::production::starting_coins(role))
                .unwrap()
                .with_garment_condition(Some(0.5))
                .unwrap();
            for (good, units) in crate::production::starting_inputs(role).items() {
                citizen = citizen.with_good(good, units).unwrap();
            }
            let (next, id) = universe.with_citizen(role.name(), citizen).unwrap();
            universe = next;
            let workplace = match role {
                StartingRole::Farmer => Some(Location::Field),
                StartingRole::Miller => Some(Location::Mill),
                StartingRole::Baker => Some(Location::Bakery),
                StartingRole::Weaver => Some(Location::Weavery),
                StartingRole::Tailor => Some(Location::Tailory),
                _ => None,
            };
            if let Some(kind) = workplace {
                universe = universe.with_property(id, kind).unwrap().0;
            }
            ids.push(id);
        }
        for id in &ids {
            universe = universe.start_planning(*id).unwrap();
        }
        let mut runtime = PlanningRuntime::default();
        for day in 1..=10 {
            let before = std::time::Instant::now();
            universe = universe
                .advance_with_planner(&mut runtime, 24 * HOUR_MS, || false)
                .unwrap()
                .unwrap();
            let map = universe.map();
            let crops: Vec<_> = map
                .places()
                .values()
                .flat_map(|place| place.plots.iter().flatten())
                .copied()
                .collect();
            let produced: u64 = ids
                .iter()
                .map(|id| {
                    let history = universe.citizen_history(*id).unwrap();
                    history
                        .completed
                        .iter()
                        .map(|day| {
                            day.produced[Good::Wheat as usize] + day.produced[Good::Flax as usize]
                        })
                        .sum::<u64>()
                        + history.current.produced[Good::Wheat as usize]
                        + history.current.produced[Good::Flax as usize]
                })
                .sum();
            for id in &ids {
                let AgentKind::Citizen(citizen) = &universe.agents()[id].kind;
                assert_eq!(citizen.map, universe.map());
            }
            eprintln!(
                "FARM DAY {day}: crops={} tends={} crop_produced={produced} wall_s={:.3}",
                crops.len(),
                crops
                    .iter()
                    .map(|crop| crop.tended.iter().filter(|done| **done).count())
                    .sum::<usize>(),
                before.elapsed().as_secs_f64()
            );
        }
    }

    #[test]
    fn demand_and_pipeline_choose_crops_without_disabling_existing_care() {
        use crate::marketplace::ShoppingList;
        let (mut citizen, field) = farmer();
        assert_eq!(citizen.next_crop_kind(), Some(CropKind::Wheat));
        citizen = perform(
            citizen,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Wheat,
            },
        );
        assert_eq!(citizen.next_crop_kind(), Some(CropKind::Flax));
        citizen
            .market
            .set_request(citizen.id, ShoppingList::single(Good::Flax, 5000))
            .unwrap();
        citizen.market.refresh_affordability(&[(citizen.id, 1000)]);
        assert_eq!(citizen.next_crop_kind(), Some(CropKind::Flax));
        citizen = citizen.advance(24 * HOUR_MS).unwrap();
        assert!(
            matches!(citizen.farming_actions()[0], CitizenAction::Tend { field: site, plot: 0, .. } if site == field)
        );
        citizen.market.refresh_affordability(&[(citizen.id, 0)]);
        assert_eq!(citizen.crop_demand(), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn town_consumption_includes_caravan_purchases_and_excludes_exports() {
        use crate::marketplace::ShoppingList;
        let (mut citizen, field) = farmer();
        citizen.market.prices = citizen
            .prices
            .with_price(Good::Wheat, Good::Wheat.core_price() * 0.4)
            .unwrap();
        citizen
            .market
            .list(citizen.id, field, Good::Wheat, 1000)
            .unwrap();
        citizen
            .market
            .caravans_with_limits(
                citizen.map.public_place(Location::Market),
                crate::marketplace::UPDATE_TIME_MS,
                [u64::MAX; Good::COUNT],
            )
            .unwrap();
        assert!(citizen.market.current_period().goods[Good::Wheat as usize].exported_units > 0);
        assert_eq!(citizen.crop_demand(), [1.0, 1.0, 1.0]);
        let (mut citizen, _) = farmer();
        citizen.market.prices = citizen
            .prices
            .with_price(Good::Wheat, Good::Wheat.core_price() * 2.0)
            .unwrap();
        citizen
            .market
            .set_request(citizen.id, ShoppingList::single(Good::Wheat, 1000))
            .unwrap();
        citizen.market.refresh_affordability(&[(citizen.id, 1000)]);
        let market = citizen.map.public_place(Location::Market);
        citizen
            .market
            .update(crate::marketplace::UPDATE_TIME_MS)
            .unwrap();
        citizen
            .market
            .caravans_with_limits(
                market,
                crate::marketplace::UPDATE_TIME_MS,
                [u64::MAX; Good::COUNT],
            )
            .unwrap();
        citizen
            .market
            .set_request(citizen.id, ShoppingList::default())
            .unwrap();
        citizen.market.refresh_affordability(&[(citizen.id, 1000)]);
        let before_purchase = citizen.crop_demand()[0];
        let bought = citizen
            .market
            .purchase(
                citizen.id,
                market,
                (Good::Wheat, 100),
                1000,
                crate::marketplace::UPDATE_TIME_MS,
                true,
            )
            .unwrap();
        assert_eq!(bought.units, 100);
        citizen
            .market
            .set_request(citizen.id, ShoppingList::default())
            .unwrap();
        citizen.market.refresh_affordability(&[(citizen.id, 1000)]);
        assert!(citizen.crop_demand()[0] > before_purchase);
        assert_eq!(citizen.crop_demand()[1], 0.0);
    }

    #[test]
    fn deadline_capacity_rejects_overlapping_work_and_time_overflow() {
        let (mut citizen, field) = farmer();
        citizen.market_time_ms = 48 * HOUR_MS - HOUR_MS / 2;
        for plot in 0..2 {
            citizen
                .map
                .set_crop(
                    field,
                    plot,
                    Some(Crop {
                        kind: CropKind::Wheat,
                        planted_at_ms: 0,
                        tended: [false, false],
                    }),
                )
                .unwrap();
        }
        assert!(!citizen.can_support_plant(field));
        citizen.market_time_ms = u64::MAX - HOUR_MS - 1;
        assert!(!citizen.can_support_plant(field));
        assert!(
            citizen
                .farming_actions()
                .iter()
                .all(|action| !matches!(action, CitizenAction::Plant { .. }))
        );
        assert_eq!(
            Crop {
                kind: CropKind::Wheat,
                planted_at_ms: u64::MAX,
                tended: [false, false]
            }
            .status(u64::MAX),
            CropStatus::Ready
        );
    }

    #[test]
    fn full_care_harvests_once_after_four_days() {
        let (citizen, field) = farmer();
        assert_eq!(citizen.map().place(field).unwrap().plots.len(), FIELD_PLOTS);
        let mut citizen = perform(
            citizen,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Wheat,
            },
        );
        let planted = citizen.crop(field, 0).unwrap().unwrap().planted_at_ms;
        assert_eq!(planted, HOUR_MS);
        assert_eq!(citizen.units(Good::Wheat), 0);
        assert_eq!(
            citizen
                .crop(field, 0)
                .unwrap()
                .unwrap()
                .expected_yield(planted),
            150
        );
        let tend = CitizenAction::Tend {
            field,
            plot: 0,
            stage: 0,
        };
        assert!(citizen.start_action(tend).is_err());
        citizen = citizen.advance(24 * HOUR_MS).unwrap();
        citizen = perform(citizen, tend);
        citizen = citizen
            .advance(planted + 48 * HOUR_MS - citizen.current_time_ms())
            .unwrap();
        citizen = perform(
            citizen,
            CitizenAction::Tend {
                field,
                plot: 0,
                stage: 1,
            },
        );
        assert_eq!(
            citizen
                .crop(field, 0)
                .unwrap()
                .unwrap()
                .expected_yield(citizen.current_time_ms()),
            600
        );
        citizen = citizen
            .advance(planted + GROWTH_MS - citizen.current_time_ms())
            .unwrap();
        citizen = perform(citizen, CitizenAction::Harvest { field, plot: 0 });
        assert_eq!(citizen.units(Good::Wheat), 600);
        assert_eq!(citizen.crop(field, 0).unwrap(), None);
        assert!(
            citizen
                .start_action(CitizenAction::Harvest { field, plot: 0 })
                .is_err()
        );
    }
    #[test]
    fn missed_windows_are_permanent_and_both_misses_require_clearing() {
        let (citizen, field) = farmer();
        let planted = perform(
            citizen,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Flax,
            },
        );
        let jump = planted.advance(80 * HOUR_MS).unwrap();
        let split = planted
            .advance(40 * HOUR_MS)
            .unwrap()
            .advance(40 * HOUR_MS)
            .unwrap();
        assert_eq!(jump.crop(field, 0), split.crop(field, 0));
        let crop = jump.crop(field, 0).unwrap().unwrap();
        assert_eq!(crop.status(jump.current_time_ms()), CropStatus::Dead);
        assert_eq!(crop.expected_yield(jump.current_time_ms()), 0);
        assert!(
            jump.start_action(CitizenAction::Tend {
                field,
                plot: 0,
                stage: 0
            })
            .is_err()
        );
        assert!(
            jump.start_action(CitizenAction::Harvest { field, plot: 0 })
                .is_err()
        );
        let clear = jump
            .start_action(CitizenAction::Clear { field, plot: 0 })
            .unwrap();
        assert!(clear.active_action().unwrap().duration_ms() <= HOUR_MS / 4);
        assert_eq!(
            clear.advance(HOUR_MS / 4).unwrap().crop(field, 0).unwrap(),
            None
        );
    }
    #[test]
    fn tend_completion_at_deadline_and_prediction_match() {
        let (citizen, field) = farmer();
        let planted = perform(
            citizen,
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Flax,
            },
        );
        let mut citizen = planted.advance(48 * HOUR_MS - HOUR_MS / 2).unwrap();
        // Fixed qualification removes practice rounding from the boundary assertion.
        citizen = citizen
            .with_skill_practice_ms(Skill::Farming, Some(0))
            .unwrap();
        let started = citizen
            .start_action(CitizenAction::Tend {
                field,
                plot: 0,
                stage: 0,
            })
            .unwrap();
        let actual = started.advance(HOUR_MS / 2).unwrap();
        let predicted = started.advance_predicted(HOUR_MS / 2).unwrap();
        assert_eq!(actual.crop(field, 0), predicted.crop(field, 0));
        let ripe = actual.advance(100 * HOUR_MS).unwrap();
        assert_eq!(
            ripe.crop(field, 0)
                .unwrap()
                .unwrap()
                .expected_yield(ripe.current_time_ms()),
            375
        );
        assert_eq!(
            ripe.crop(field, 0)
                .unwrap()
                .unwrap()
                .status(ripe.current_time_ms()),
            CropStatus::Ready
        );
    }
    #[test]
    fn fields_require_individual_ownership_and_crop_wealth_tracks_yield() {
        let (citizen, field) = farmer();
        let other = Citizen::new(0.0)
            .unwrap()
            .with_map(citizen.map())
            .unwrap()
            .with_skill_practice_ms(Skill::Farming, Some(0))
            .unwrap();
        assert_eq!(
            other.start_action(CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Wheat
            }),
            Err(SimulationError::PrivateProperty)
        );
        let planted = perform(
            citizen.clone(),
            CitizenAction::Plant {
                field,
                plot: 0,
                crop: CropKind::Wheat,
            },
        );
        assert!(
            (planted.wealth().unwrap()
                - citizen.wealth().unwrap()
                - citizen.prices.value(Good::Wheat, 150).unwrap() * 0.70)
                .abs()
                < 1e-9
        );
    }
}

#[derive(Clone, Copy)]
struct Labor {
    release: u64,
    deadline: u64,
    duration: u64,
}

impl Citizen {
    fn farm_work_ms(&self, base: u64) -> Option<u64> {
        let practice = self.skill_practice_ms(Skill::Farming)?;
        Some((base as f64 * (1.0 - skill_duration_reduction(practice))).ceil() as u64)
    }

    fn crop_demand(&self) -> [f64; 3] {
        let start = self
            .market_time_ms
            .saturating_sub(7 * crate::marketplace::DAY_MS);
        let mut demand = [0.0; 3];
        for (index, kind) in CropKind::ALL.into_iter().enumerate() {
            demand[index] = self
                .market
                .history()
                .iter()
                .filter(|day| {
                    day.good == kind.good()
                        && day.end_ms > start
                        && day.end_ms <= self.market_time_ms
                })
                .map(|day| {
                    day.local_traded_units as f64
                        + day.caravan_purchased_units as f64
                        + day.unmet_demand_units as f64
                })
                .sum::<f64>();
            let current = self.market.current_period().goods[kind.good() as usize];
            demand[index] += current.local_traded_units as f64
                + current.caravan_purchased_units as f64
                + current.affordable_demand_units as f64;
            demand[index] *= GROWTH_MS as f64 / kind.recipe().outputs()[0].1 as f64;
        }
        if demand.iter().sum::<f64>() == 0.0 {
            [1.0, 1.0, 1.0]
        } else {
            demand
        }
    }

    fn next_crop_kind(&self) -> Option<CropKind> {
        let demand = self.crop_demand();
        let total = demand.iter().sum::<f64>();
        let plots = self
            .owned_properties()
            .map(|place| place.plots.len())
            .sum::<usize>() as f64;
        let mut pipeline = CropKind::ALL.map(|kind| {
            (self.available_units(kind.good()) as f64
                + self.market.listed_units(self.id, kind.good()) as f64)
                / kind.recipe().outputs()[0].1 as f64
        });
        for place in self.owned_properties() {
            for crop in place.plots.iter().flatten() {
                if crop.status(self.market_time_ms) != CropStatus::Dead {
                    let index = CropKind::ALL
                        .iter()
                        .position(|kind| *kind == crop.kind)
                        .unwrap();
                    pipeline[index] += 1.0;
                }
            }
        }
        CropKind::ALL
            .into_iter()
            .enumerate()
            .map(|(index, kind)| (kind, plots * demand[index] / total - pipeline[index]))
            .filter(|(_, deficit)| *deficit > 0.0)
            .max_by(|(a, ad), (b, bd)| {
                ad.total_cmp(bd)
                    .then_with(|| (b.good() as usize).cmp(&(a.good() as usize)))
            })
            .map(|(kind, _)| kind)
    }

    fn can_support_plant(&self, field: PlaceId) -> bool {
        let Some(plant_ms) = self.farm_work_ms(HOUR_MS) else {
            return false;
        };
        let Some(tend_ms) = self.farm_work_ms(HOUR_MS / 2) else {
            return false;
        };
        let now = self.market_time_ms;
        let travel_ms = (self.position.distance(self.map.position(field))
            * crate::locations::WALK_MS_PER_METRE)
            .ceil() as u64;
        let Some(planted_at_ms) = now
            .checked_add(travel_ms)
            .and_then(|time| time.checked_add(plant_ms))
        else {
            return false;
        };
        if planted_at_ms.checked_add(GROWTH_MS).is_none() {
            return false;
        }
        let mut tasks = Vec::new();
        for place in self.owned_properties() {
            for crop in place
                .plots
                .iter()
                .flatten()
                .copied()
                .chain((place.id == field).then_some(Crop {
                    kind: CropKind::Wheat,
                    planted_at_ms,
                    tended: [false; 2],
                }))
            {
                if crop.status(now) == CropStatus::Dead {
                    continue;
                }
                for stage in 0..2 {
                    if !crop.tended[stage]
                        && let Some((release, deadline)) = crop.tend_window(stage)
                        && deadline >= now
                    {
                        tasks.push(Labor {
                            release: release.max(now),
                            deadline,
                            duration: tend_ms,
                        });
                    }
                }
                let release = crop.ready_at_ms().max(now);
                tasks.push(Labor {
                    release,
                    deadline: release.saturating_add(crate::marketplace::DAY_MS),
                    duration: plant_ms,
                });
            }
        }
        tasks.push(Labor {
            release: now,
            deadline: planted_at_ms,
            duration: travel_ms + plant_ms,
        });
        tasks.sort_by_key(|task| (task.deadline, task.release));
        let first_day = crate::production::work_period(now);
        let mut used = std::collections::BTreeMap::new();
        used.insert(first_day, self.production_work_today_ms());
        let mut fields: Vec<_> = self
            .owned_properties()
            .filter(|place| !place.plots.is_empty())
            .collect();
        fields.sort_by_key(|place| place.id.0);
        let mut position = self.map.position(self.home);
        let mut distance = 0.0;
        for field in fields {
            distance += position.distance(field.position);
            position = field.position;
        }
        distance += position.distance(self.map.position(self.home));
        let commute = (distance * crate::locations::WALK_MS_PER_METRE).ceil() as u64;
        // Leave time for food, trading, and the daily journey home before reserving crop labor.
        let capacity = crate::production::DAILY_CAPACITY_MS.saturating_sub(HOUR_MS + commute);
        let mut cursor = now;
        for task in tasks {
            let mut remaining = task.duration;
            let mut time = task.release.max(cursor);
            while remaining > 0 && time < task.deadline {
                let day = crate::production::work_period(time);
                let day_end = crate::marketplace::UPDATE_TIME_MS
                    .saturating_add(day.saturating_mul(crate::marketplace::DAY_MS))
                    .min(task.deadline);
                let spent = used.entry(day).or_insert(0_u64);
                let available = capacity.saturating_sub(*spent).min(day_end - time);
                let allocated = remaining.min(available);
                *spent += allocated;
                remaining -= allocated;
                time = if remaining == 0 {
                    time + allocated
                } else {
                    day_end
                };
            }
            if remaining > 0 {
                return false;
            }
            cursor = time;
        }
        true
    }

    pub(crate) fn farming_actions(&self) -> Vec<CitizenAction> {
        if self.skill_practice_ms(Skill::Farming).is_none() {
            return Vec::new();
        }
        let mut fields: Vec<_> = self
            .owned_properties()
            .filter(|place| !place.plots.is_empty())
            .collect();
        fields.sort_by_key(|place| place.id.0);
        let mut existing = Vec::new();
        let mut empty = Vec::new();
        for field in fields {
            let travel_ms = (self.position.distance(field.position)
                * crate::locations::WALK_MS_PER_METRE)
                .ceil() as u64;
            let mut arrived = self.clone();
            arrived.market_time_ms = self.market_time_ms.saturating_add(travel_ms);
            for (plot, crop) in field.plots.iter().enumerate() {
                if let Some(crop) = crop {
                    let actions = [
                        CitizenAction::Tend {
                            field: field.id,
                            plot,
                            stage: 0,
                        },
                        CitizenAction::Tend {
                            field: field.id,
                            plot,
                            stage: 1,
                        },
                        CitizenAction::Harvest {
                            field: field.id,
                            plot,
                        },
                        CitizenAction::Clear {
                            field: field.id,
                            plot,
                        },
                    ];
                    for action in actions {
                        if let Ok(duration) = arrived.farming_duration_ms(action)
                            && self
                                .production_work_today_ms()
                                .saturating_add(travel_ms)
                                .saturating_add(duration)
                                <= crate::production::DAILY_CAPACITY_MS
                        {
                            let deadline = if let CitizenAction::Tend { stage, .. } = action {
                                crop.tend_window(stage).unwrap().1
                            } else {
                                u64::MAX
                            };
                            existing.push((deadline, action));
                        }
                    }
                } else {
                    empty.push((field.id, plot));
                }
            }
        }
        existing.sort_by_key(|(deadline, _)| *deadline);
        if !existing.is_empty() {
            return existing.into_iter().map(|(_, action)| action).collect();
        }
        if let Some(crop) = self.next_crop_kind() {
            for (field, plot) in empty {
                if self.can_support_plant(field) {
                    return vec![CitizenAction::Plant { field, plot, crop }];
                }
            }
        }
        Vec::new()
    }
}
