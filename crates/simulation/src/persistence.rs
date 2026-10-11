use crate::{AgentKind, Citizen, CitizenAction, Universe, locations::Location, marketplace::Good};
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeSeed, MapAccess, Visitor},
};
use std::fmt;

pub const SAVE_VERSION: u32 = 2;

#[derive(Debug)]
pub enum SaveError {
    UnsupportedVersion(u32),
    InvalidState(&'static str),
    Format(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "Unsupported save version {version}; expected {SAVE_VERSION}"
            ),
            Self::InvalidState(reason) => write!(formatter, "Invalid saved universe: {reason}"),
            Self::Format(reason) => write!(formatter, "Invalid save format: {reason}"),
        }
    }
}

impl std::error::Error for SaveError {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save<T> {
    version: u32,
    universe: T,
}

// An untyped version scan repeatedly scans numeric RON saves; decode the envelope once.
struct SaveLoader<'a> {
    unsupported_version: &'a mut Option<u32>,
}

impl<'de> DeserializeSeed<'de> for SaveLoader<'_> {
    type Value = Universe;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Universe, D::Error> {
        deserializer.deserialize_struct("Save", &["version", "universe"], self)
    }
}

impl<'de> Visitor<'de> for SaveLoader<'_> {
    type Value = Universe;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a saved universe with version and universe fields")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Universe, M::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Version,
            Universe,
        }

        let mut version = None;
        let mut universe = None;
        while let Some(field) = map.next_key()? {
            match field {
                Field::Version => {
                    if version.is_some() {
                        return Err(de::Error::duplicate_field("version"));
                    }
                    let value = map.next_value::<u32>()?;
                    if value != SAVE_VERSION {
                        *self.unsupported_version = Some(value);
                        return Err(de::Error::custom("unsupported save version"));
                    }
                    version = Some(value);
                }
                Field::Universe => {
                    if universe.is_some() {
                        return Err(de::Error::duplicate_field("universe"));
                    }
                    universe = Some(map.next_value()?);
                }
            }
        }
        version.ok_or_else(|| de::Error::missing_field("version"))?;
        universe.ok_or_else(|| de::Error::missing_field("universe"))
    }
}

impl Universe {
    pub fn to_save_string(&self) -> Result<String, SaveError> {
        ron::ser::to_string_pretty(
            &Save {
                version: SAVE_VERSION,
                universe: self,
            },
            ron::ser::PrettyConfig::default(),
        )
        .map_err(|error| SaveError::Format(error.to_string()))
    }

    pub fn from_save_str(input: &str) -> Result<Self, SaveError> {
        let mut unsupported_version = None;
        let mut deserializer = ron::Deserializer::from_str(input)
            .map_err(|error| SaveError::Format(error.to_string()))?;
        let mut universe = SaveLoader {
            unsupported_version: &mut unsupported_version,
        }
        .deserialize(&mut deserializer)
        .map_err(|error| match unsupported_version {
            Some(version) => SaveError::UnsupportedVersion(version),
            None => SaveError::Format(deserializer.span_error(error).to_string()),
        })?;
        deserializer
            .end()
            .map_err(|error| SaveError::Format(deserializer.span_error(error).to_string()))?;
        universe.validate_save()?;
        // Validate actions before adding plots so invalid legacy indices stay invalid.
        expand_legacy_fields(&mut universe.map);
        for (_, agent) in universe.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            expand_legacy_fields(&mut citizen.map);
        }
        Ok(universe)
    }

    fn validate_save(&self) -> Result<(), SaveError> {
        self.map.validate_save()?;
        self.market.validate_save()?;
        self.storage.validate_save()?;
        if self.town_treasury < 0 {
            return Err(SaveError::InvalidState("negative town treasury"));
        }
        for (id, agent) in &self.agents {
            let AgentKind::Citizen(citizen) = &agent.kind;
            if *id != citizen.id || !self.citizen_histories.contains_key(id) {
                return Err(SaveError::InvalidState(
                    "citizen identity or history mismatch",
                ));
            }
            citizen.validate_save()?;
            if let Some(active) = citizen.active_action() {
                let action = active.action();
                if action.production_recipe() == Some(crate::production::Recipe::BrewBeer) {
                    let site = action
                        .required_place(citizen)
                        .ok_or(SaveError::InvalidState("missing preparation site"))?;
                    if !self.map.place(site).is_ok_and(|place| {
                        place.owner == Some(*id)
                            && place.kind == Location::Brewery
                            && place
                                .batches
                                .iter()
                                .filter(|batch| **batch == Some(crate::brewing::Batch::Preparing))
                                .count()
                                == 1
                    }) {
                        return Err(SaveError::InvalidState("invalid active preparation"));
                    }
                }
                if let CitizenAction::CollectBeer { brewery, slot } = action
                    && !self.map.place(brewery).is_ok_and(|place| {
                        place.owner == Some(*id)
                            && place.kind == Location::Brewery
                            && place
                                .batches
                                .get(slot)
                                .copied()
                                .flatten()
                                .is_some_and(|batch| batch.ready(self.current_time_ms))
                    })
                {
                    return Err(SaveError::InvalidState("invalid active collection"));
                }
            }
            self.citizen_histories[id].validate_save(self.current_time_ms)?;
        }
        for (id, rule) in &self.taxation.rules {
            if *id != rule.id || id.0 >= self.taxation.next_rule_id {
                return Err(SaveError::InvalidState("invalid tax rule identity"));
            }
            match &rule.kind {
                crate::taxation::TaxKind::Socage { rates }
                | crate::taxation::TaxKind::Asset { rates }
                | crate::taxation::TaxKind::Income { rates } => {
                    if rates.values().any(|rate| rate.basis_points() > 10_000) {
                        return Err(SaveError::InvalidState("invalid tax rate"));
                    }
                }
                crate::taxation::TaxKind::FlatFee { coins, .. } if *coins < 0 => {
                    return Err(SaveError::InvalidState("negative tax fee"));
                }
                _ => {}
            }
        }
        if self.taxation.accounts.values().any(|account| {
            !account.asset_remainder.is_finite()
                || !(0.0..1.0).contains(&account.asset_remainder)
                || account.income_remainder >= 10_000
                || account.arrears < 0
                || account.goods_owed().is_err()
        }) {
            return Err(SaveError::InvalidState("invalid tax account"));
        }
        for place in self
            .map
            .places()
            .values()
            .filter(|place| place.kind == Location::Brewery)
        {
            let owner = place
                .owner
                .ok_or(SaveError::InvalidState("unowned brewery"))?;
            let Some(agent) = self.agents.get(&owner) else {
                return Err(SaveError::InvalidState("missing brewery owner"));
            };
            let AgentKind::Citizen(citizen) = &agent.kind;
            let preparing = place
                .batches
                .iter()
                .filter(|batch| **batch == Some(crate::brewing::Batch::Preparing))
                .count();
            let active_prep = citizen.active_action().is_some_and(|active| {
                active.action().production_recipe() == Some(crate::production::Recipe::BrewBeer)
                    && active.action().required_place(citizen) == Some(place.id)
            });
            if preparing != usize::from(active_prep) {
                return Err(SaveError::InvalidState("orphan brewery preparation"));
            }
            if let Some(active) = citizen.active_action()
                && let CitizenAction::CollectBeer { brewery, slot } = active.action()
                && brewery == place.id
                && !place
                    .batches
                    .get(slot)
                    .copied()
                    .flatten()
                    .is_some_and(|batch| batch.ready(self.current_time_ms))
            {
                return Err(SaveError::InvalidState("invalid active beer collection"));
            }
        }
        self.validate_tax_reservations()?;
        Ok(())
    }

    fn validate_tax_reservations(&self) -> Result<(), SaveError> {
        use crate::taxation::ReservationStock;
        let mut arrears = imbl::HashMap::new();
        for (key, account) in &self.taxation.accounts {
            if !self.agents.contains_key(&key.payer)
                || self.map.place(key.place).is_err()
                || !self.taxation.rules.contains_key(&key.rule)
            {
                return Err(SaveError::InvalidState(
                    "tax account references missing state",
                ));
            }
            let total = arrears.entry((key.rule, key.payer)).or_insert(0_i64);
            *total = total
                .checked_add(account.arrears)
                .ok_or(SaveError::InvalidState("tax arrears overflow"))?;
        }
        let mut reserved_accounts = imbl::HashMap::new();
        for (stock, claims) in &self.taxation.reservations {
            let (good, reserved) = match stock {
                ReservationStock::Carried(carrier, good) => {
                    let Some(agent) = self.agents.get(carrier) else {
                        return Err(SaveError::InvalidState(
                            "tax reservation has missing carrier",
                        ));
                    };
                    let AgentKind::Citizen(citizen) = &agent.kind;
                    (*good, citizen.tax_reserved_units(*good))
                }
                ReservationStock::Stored(key) => {
                    if self.map.place(key.place).is_err()
                        || matches!(key.owner, crate::storage::GoodsOwner::Agent(id) if !self.agents.contains_key(&id))
                    {
                        return Err(SaveError::InvalidState(
                            "tax reservation has missing storage",
                        ));
                    }
                    (
                        key.good,
                        self.storage.reserved_units(key.place, key.owner, key.good),
                    )
                }
            };
            let mut stock_total = 0_u64;
            for (key, units) in claims {
                if key.good != Some(good) || !self.taxation.accounts.contains_key(key) {
                    return Err(SaveError::InvalidState(
                        "tax reservation has invalid account",
                    ));
                }
                stock_total = stock_total
                    .checked_add(*units)
                    .ok_or(SaveError::InvalidState("tax reservation overflow"))?;
                let total = reserved_accounts.entry(*key).or_insert(0_u64);
                *total = total
                    .checked_add(*units)
                    .ok_or(SaveError::InvalidState("tax reservation overflow"))?;
            }
            if stock_total != reserved {
                return Err(SaveError::InvalidState("tax reservation stock mismatch"));
            }
        }
        if reserved_accounts != self.taxation.reserved_accounts
            || reserved_accounts.iter().any(|(key, units)| {
                self.taxation.accounts[key]
                    .goods_owed()
                    .map_or(true, |owed| *units > owed)
            })
        {
            return Err(SaveError::InvalidState("tax reservation account mismatch"));
        }
        Ok(())
    }
}

impl Citizen {
    fn validate_save(&self) -> Result<(), SaveError> {
        self.map.validate_save()?;
        self.market.validate_save()?;
        self.storage.validate_save()?;
        if !self.position.valid()
            || ![
                self.hunger,
                self.thirst,
                self.tiredness,
                self.hunger_per_hour,
            ]
            .into_iter()
            .all(f64::is_finite)
            || self.hunger_per_hour < 0.0
            || self.coins < 0
            || self
                .diet
                .into_iter()
                .any(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
            || self
                .garment_condition
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || self.map.place(self.home).is_err()
            || self
                .last_meal_started_ms
                .is_some_and(|time| time > self.market_time_ms)
            || Good::ALL
                .into_iter()
                .any(|good| self.tax_reserved_units(good) > self.units(good))
        {
            return Err(SaveError::InvalidState("invalid citizen state"));
        }
        for good in Good::ALL {
            self.prices
                .with_price(good, self.prices.price(good).unwrap_or(f64::NAN))
                .map_err(|_| SaveError::InvalidState("invalid citizen price"))?;
        }
        if let Some(active) = self.active_action {
            if active.duration_ms == 0
                || active.remaining_ms == 0
                || active.remaining_ms > active.duration_ms
                || active.meal_duration_ms > active.duration_ms
                || !matches!(active.drink_good, Good::Water | Good::Beer)
                || active.drink_units > crate::DRINK_WATER_GRAMS
                || !active.meal_nutrition.is_finite()
                || active.meal_nutrition < 0.0
                || !active.travel_origin.valid()
                || active
                    .travel_destination
                    .is_some_and(|position| !position.valid())
                || active
                    .drink_units
                    .checked_mul(crate::DRINK_DURATION_MS)
                    .is_none()
            {
                return Err(SaveError::InvalidState("invalid active action"));
            }
            validate_action(active.action, &self.map)?;
        }
        if let Some(active) = &self.active_plan {
            active.validate_save()?;
            if self.active_action.is_none() {
                return Err(SaveError::InvalidState("active plan has no action"));
            }
            for &action in active.plan().actions() {
                validate_action(action, &self.map)?;
            }
        }
        Ok(())
    }
}

fn expand_legacy_fields(map: &mut crate::locations::Map) {
    for (_, place) in map.places_mut().iter_mut() {
        if place.kind == Location::Field && place.plots.len() == 12 {
            place.plots.resize(crate::farming::FIELD_PLOTS, None);
        }
    }
}

fn validate_action(action: CitizenAction, map: &crate::locations::Map) -> Result<(), SaveError> {
    let place = match action {
        CitizenAction::Travel(place)
        | CitizenAction::BuyAt { place, .. }
        | CitizenAction::ProduceAt { place, .. }
        | CitizenAction::CollectStored { place, .. }
        | CitizenAction::DepositStored { place, .. } => Some(place),
        CitizenAction::CollectBeer { brewery, slot } => {
            if !map
                .place(brewery)
                .is_ok_and(|place| place.kind == Location::Brewery && slot < place.batches.len())
            {
                return Err(SaveError::InvalidState("invalid brewing action"));
            }
            Some(brewery)
        }
        CitizenAction::Plant { field, plot, .. }
        | CitizenAction::Harvest { field, plot }
        | CitizenAction::Clear { field, plot }
        | CitizenAction::Tend { field, plot, .. } => {
            let valid = map
                .place(field)
                .is_ok_and(|place| place.kind == Location::Field && plot < place.plots.len());
            if !valid || matches!(action, CitizenAction::Tend { stage, .. } if stage >= 2) {
                return Err(SaveError::InvalidState("invalid crop action"));
            }
            Some(field)
        }
        _ => None,
    };
    if place.is_some_and(|place| map.place(place).is_err()) {
        return Err(SaveError::InvalidState("action references missing place"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TRADE_DURATION_MS,
        farming::CropKind,
        locations::{Map, Position},
        marketplace::ShoppingList,
        production::Recipe,
        storage::GoodsOwner,
        taxation::{AccountKey, TaxAccount, TaxKind, TaxRate},
    };
    use rand::SeedableRng;

    fn round_trip(world: &Universe) -> Universe {
        let encoded = world.to_save_string().unwrap();
        assert!(encoded.contains("version: 2"));
        let loaded = Universe::from_save_str(&encoded).unwrap();
        assert_eq!(&loaded, world);
        loaded
    }

    #[test]
    fn complex_universe_round_trip_preserves_history_crops_storage_and_fractional_taxes() {
        let (world, seller) = Universe::with_map(Map::default())
            .with_citizen(
                "Seller",
                Citizen::new(12.345678901234567)
                    .unwrap()
                    .with_coins(200)
                    .unwrap()
                    .with_good(Good::BreadRoll, 20)
                    .unwrap(),
            )
            .unwrap();
        let (world, buyer) = world
            .with_citizen(
                "Buyer",
                Citizen::new(-20.0).unwrap().with_coins(200).unwrap(),
            )
            .unwrap();
        let market = world.map().public_place(Location::Market);
        let world = world
            .start_action(seller, CitizenAction::Travel(market))
            .unwrap()
            .start_action(seller, CitizenAction::List(Good::BreadRoll, 10))
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let world = world
            .start_action(buyer, CitizenAction::Travel(market))
            .unwrap()
            .start_action(
                buyer,
                CitizenAction::Buy(ShoppingList::single(Good::BreadRoll, 2)),
            )
            .unwrap()
            .advance(TRADE_DURATION_MS)
            .unwrap();
        let (world, field) = world.with_property(seller, Location::Field).unwrap();
        let (world, tax) = world
            .with_tax_rule(
                field,
                "Field income",
                TaxKind::Income {
                    rates: [(Good::BreadRoll, TaxRate::new(1234).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let mut world = world
            .with_stored_good(field, GoodsOwner::Agent(seller), Good::Wheat, 37)
            .unwrap();
        world
            .map
            .set_crop(
                field,
                0,
                Some(crate::farming::Crop {
                    kind: CropKind::Cabbage,
                    planted_at_ms: world.current_time_ms,
                    tended: [true, false],
                }),
            )
            .unwrap();
        world.taxation.accounts.insert(
            AccountKey {
                place: field,
                rule: tax,
                payer: seller,
                good: Some(Good::BreadRoll),
            },
            TaxAccount {
                goods_due_scaled: 12345,
                income_remainder: 2468,
                asset_remainder: 0.12345678901234567,
                arrears: 3,
            },
        );
        let loaded = round_trip(&world);
        assert_eq!(
            loaded.advance(123_456).unwrap(),
            world.advance(123_456).unwrap()
        );
    }

    fn resize_fields(world: &mut Universe, count: usize) {
        let resize = |map: &mut Map| {
            for (_, place) in map.places_mut().iter_mut() {
                if place.kind == Location::Field {
                    place.plots.resize(count, None);
                }
            }
        };
        resize(&mut world.map);
        for (_, agent) in world.agents.iter_mut() {
            let AgentKind::Citizen(citizen) = &mut agent.kind;
            resize(&mut citizen.map);
        }
    }

    #[test]
    fn legacy_fields_expand_without_changing_crops_or_committed_plans() {
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen(
                "Hungry",
                Citizen::with_needs(80.0, -100.0)
                    .unwrap()
                    .with_good(Good::BreadRoll, 10)
                    .unwrap(),
            )
            .unwrap();
        let (mut world, field) = world.with_property(id, Location::Field).unwrap();
        world
            .map
            .set_crop(
                field,
                11,
                Some(crate::farming::Crop {
                    kind: CropKind::Cabbage,
                    planted_at_ms: 0,
                    tended: [true, false],
                }),
            )
            .unwrap();
        let mut world = world.start_planning(id).unwrap().advance(1000).unwrap();
        resize_fields(&mut world, 12);
        let AgentKind::Citizen(citizen) = &world.agents[&id].kind;
        assert!(citizen.active_plan().is_some());
        let loaded = Universe::from_save_str(&world.to_save_string().unwrap()).unwrap();
        let plots = &loaded.map.place(field).unwrap().plots;
        assert_eq!(&plots[..12], &world.map.place(field).unwrap().plots);
        assert_eq!(plots.len(), 24);
        assert!(plots[12..].iter().all(Option::is_none));
        resize_fields(&mut world, 24);
        assert_eq!(loaded, world);
        round_trip(&loaded);
        assert_eq!(loaded.advance(1000).unwrap(), world.advance(1000).unwrap());
    }

    #[test]
    fn rejects_malformed_fields_and_out_of_range_legacy_actions() {
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen("Farmer", Citizen::new(0.0).unwrap())
            .unwrap();
        let (world, field) = world.with_property(id, Location::Field).unwrap();
        for count in [0, 11, 13, 23, 25] {
            let mut invalid = world.clone();
            resize_fields(&mut invalid, count);
            assert!(matches!(
                Universe::from_save_str(&invalid.to_save_string().unwrap()),
                Err(SaveError::InvalidState(_))
            ));
        }
        let mut invalid = world.start_action(id, CitizenAction::Wait).unwrap();
        resize_fields(&mut invalid, 12);
        let AgentKind::Citizen(citizen) = &mut invalid.agents.get_mut(&id).unwrap().kind;
        citizen.active_action.as_mut().unwrap().action = CitizenAction::Plant {
            field,
            plot: 12,
            crop: CropKind::Wheat,
        };
        assert!(matches!(
            Universe::from_save_str(&invalid.to_save_string().unwrap()),
            Err(SaveError::InvalidState(_))
        ));
    }

    #[test]
    fn mid_forage_round_trip_resumes_the_same_random_stream() {
        let mut citizen = Citizen::new(0.0).unwrap();
        citizen.forage_rng = rand::rngs::Xoshiro256PlusPlus::seed_from_u64(42);
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen("Forager", citizen)
            .unwrap();
        let forest = world.map().public_place(Location::Forest);
        let world = world
            .start_action(id, CitizenAction::Travel(forest))
            .unwrap()
            .start_action(id, CitizenAction::Produce(Recipe::Forage))
            .unwrap()
            .advance(123_456)
            .unwrap();
        let mut original = world;
        let mut loaded = round_trip(&original);
        for _ in 0..8 {
            original = original.advance(crate::ACTION_DURATION_MS).unwrap();
            loaded = loaded.advance(crate::ACTION_DURATION_MS).unwrap();
            assert_eq!(loaded, original);
            original = original
                .start_action(id, CitizenAction::Produce(Recipe::Forage))
                .unwrap();
            loaded = loaded
                .start_action(id, CitizenAction::Produce(Recipe::Forage))
                .unwrap();
        }
    }

    #[test]
    fn mid_plan_round_trip_preserves_committed_actions_and_resume() {
        let (world, id) = Universe::with_map(
            Map::new(
                Position { x: 20.0, y: 0.0 },
                Position::default(),
                Position::default(),
            )
            .unwrap(),
        )
        .with_citizen(
            "Hungry",
            Citizen::with_needs(80.0, -100.0)
                .unwrap()
                .with_good(Good::BreadRoll, 10)
                .unwrap()
                .with_good(Good::Water, 400)
                .unwrap(),
        )
        .unwrap();
        let world = world.start_planning(id).unwrap().advance(1000).unwrap();
        let AgentKind::Citizen(citizen) = &world.agents[&id].kind;
        assert!(citizen.active_plan().is_some());
        let loaded = round_trip(&world);
        assert_eq!(
            loaded.advance(3_600_000).unwrap(),
            world.advance(3_600_000).unwrap()
        );
    }

    #[test]
    fn rejects_history_counter_overflow_and_inconsistent_tax_reservations() {
        use crate::taxation::ReservationStock;
        let (world, id) = Universe::with_map(Map::default())
            .with_citizen("Citizen", Citizen::new(0.0).unwrap())
            .unwrap();
        for counter in 0..3 {
            let mut invalid = world.clone();
            let history = &mut invalid.citizen_histories.get_mut(&id).unwrap().current;
            match counter {
                0 => history.activity.idle_ms = u64::MAX,
                1 => history.elapsed_ms = u64::MAX,
                _ => history.food_goal_ms = u64::MAX,
            }
            assert!(matches!(
                Universe::from_save_str(&invalid.to_save_string().unwrap()),
                Err(SaveError::InvalidState(_))
            ));
        }
        let home = match &world.agents[&id].kind {
            AgentKind::Citizen(citizen) => citizen.home,
        };
        let (world, rule) = world
            .with_tax_rule(
                home,
                "Income",
                TaxKind::Income {
                    rates: [(Good::BreadRoll, TaxRate::new(100).unwrap())]
                        .into_iter()
                        .collect(),
                },
            )
            .unwrap();
        let key = AccountKey {
            place: home,
            rule,
            payer: id,
            good: Some(Good::BreadRoll),
        };
        let mut valid = world.clone();
        let AgentKind::Citizen(citizen) = &mut valid.agents.get_mut(&id).unwrap().kind;
        citizen.inventory.insert(Good::BreadRoll, 2);
        citizen.tax_reserved.insert(Good::BreadRoll, 1);
        valid.taxation.accounts.insert(
            key,
            TaxAccount {
                goods_due_scaled: 10_000,
                ..TaxAccount::default()
            },
        );
        valid.taxation.reserved_accounts.insert(key, 1);
        valid.taxation.reservations.insert(
            ReservationStock::Carried(id, Good::BreadRoll),
            [(key, 1)].into_iter().collect(),
        );
        round_trip(&valid);
        let now = 31 * crate::marketplace::DAY_MS;
        for time_ms in [0, crate::marketplace::DAY_MS, now] {
            valid.taxation.record(crate::taxation::TaxReceipt {
                time_ms,
                rule,
                place: home,
                payer: id,
                good: Some(Good::BreadRoll),
                amount: crate::taxation::TaxAmount::Reservation {
                    assessed: 1,
                    reserved: 1,
                    outstanding: 0,
                },
            });
        }
        let mut unpruned = valid.clone();
        valid.taxation.prune_history(now);
        assert_eq!(valid.taxation.history.len(), 2);
        assert_eq!(
            valid.taxation.history.front().unwrap().time_ms,
            now - 30 * crate::marketplace::DAY_MS
        );
        let mut pruned = valid.clone();
        assert_eq!(
            pruned.settle_tax_reservations(now).unwrap(),
            unpruned.settle_tax_reservations(now).unwrap()
        );
        assert_eq!(pruned.agents, unpruned.agents);
        assert_eq!(pruned.taxation.accounts, unpruned.taxation.accounts);
        assert_eq!(
            pruned.taxation.reserved_accounts,
            unpruned.taxation.reserved_accounts
        );
        assert_eq!(pruned.taxation.reservations, unpruned.taxation.reservations);
        for case in 0..5 {
            let mut invalid = valid.clone();
            match case {
                0 => {
                    invalid.taxation.accounts.remove(&key);
                }
                1 => {
                    invalid.taxation.reserved_accounts.insert(key, 2);
                }
                2 => {
                    invalid.taxation.reservations.clear();
                    invalid.taxation.reservations.insert(
                        ReservationStock::Carried(
                            crate::AgentId(uuid::Uuid::new_v4()),
                            Good::BreadRoll,
                        ),
                        [(key, 1)].into_iter().collect(),
                    );
                }
                3 => {
                    invalid.taxation.reservations.clear();
                    invalid.taxation.reservations.insert(
                        ReservationStock::Carried(id, Good::BreadRoll),
                        [(AccountKey { good: None, ..key }, 1)]
                            .into_iter()
                            .collect(),
                    );
                }
                _ => {
                    invalid.taxation.accounts.get_mut(&key).unwrap().arrears = i64::MAX;
                    invalid.taxation.accounts.insert(
                        AccountKey {
                            good: Some(Good::Fish),
                            ..key
                        },
                        TaxAccount {
                            arrears: 1,
                            ..TaxAccount::default()
                        },
                    );
                }
            }
            assert!(matches!(
                Universe::from_save_str(&invalid.to_save_string().unwrap()),
                Err(SaveError::InvalidState(_))
            ));
        }
    }

    #[test]
    fn rejects_other_versions_before_decoding_the_universe() {
        for version in [0, SAVE_VERSION - 1, SAVE_VERSION + 1, u32::MAX] {
            let input = format!("(version: {version}, universe: invalid old schema)");
            assert!(matches!(
                Universe::from_save_str(&input),
                Err(SaveError::UnsupportedVersion(found)) if found == version
            ));
        }
    }

    #[test]
    fn save_envelope_accepts_either_field_order_and_rejects_invalid_fields() {
        let world = Universe::with_map(Map::default());
        let universe = ron::to_string(&world).unwrap();
        let reversed = format!("(universe: {universe}, version: {SAVE_VERSION})");
        assert_eq!(Universe::from_save_str(&reversed).unwrap(), world);
        for input in [
            format!("(version: {SAVE_VERSION})"),
            format!("(universe: {universe})"),
            format!("(version: {SAVE_VERSION}, version: {SAVE_VERSION}, universe: {universe})"),
            format!("(version: {SAVE_VERSION}, universe: {universe}, universe: {universe})"),
            format!("(version: {SAVE_VERSION}, extra: 1, universe: {universe})"),
            format!("(version: {SAVE_VERSION}, universe: {universe}) trailing"),
            format!("(version: {SAVE_VERSION}, universe: ())"),
            format!("(version: \"2\", universe: {universe})"),
        ] {
            assert!(
                matches!(Universe::from_save_str(&input), Err(SaveError::Format(_))),
                "accepted invalid envelope: {input}"
            );
        }
    }

    #[test]
    fn rejects_unsupported_malformed_and_invalid_states() {
        assert!(matches!(
            Universe::from_save_str("(version: 999, universe: ())"),
            Err(SaveError::UnsupportedVersion(999))
        ));
        assert!(matches!(
            Universe::from_save_str("not a save"),
            Err(SaveError::Format(_))
        ));
        let (mut world, id) = Universe::with_map(Map::default())
            .with_citizen("Bad", Citizen::new(0.0).unwrap())
            .unwrap();
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&id).unwrap().kind;
        citizen.hunger = f64::NAN;
        assert!(matches!(
            Universe::from_save_str(&world.to_save_string().unwrap()),
            Err(SaveError::InvalidState(_))
        ));
        let AgentKind::Citizen(citizen) = &mut world.agents.get_mut(&id).unwrap().kind;
        citizen.hunger = 0.0;
        citizen.active_action = Some(crate::ActiveAction {
            action: CitizenAction::Wait,
            duration_ms: 1,
            remaining_ms: 2,
            meal_duration_ms: 0,
            drink_good: Good::Water,
            drink_units: 0,
            meal_nutrition: 0.0,
            meal_units: [0; Good::COUNT],
            travel_origin: Position::default(),
            travel_destination: None,
        });
        assert!(matches!(
            Universe::from_save_str(&world.to_save_string().unwrap()),
            Err(SaveError::InvalidState(_))
        ));
    }
}
