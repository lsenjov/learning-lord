#[cfg(test)]
mod tests {
    use crate::{
        AgentKind, Citizen, CitizenAction, StartingRole, Universe,
        locations::ObjectKind,
        marketplace::{DAY_MS, Good},
        planning::{self, goals::Effect},
    };

    const HOUR: u64 = 3_600_000;

    #[test]
    fn homeless_members_only_reserve_their_own_replacement_clothing() {
        let (world, first) = Universe::default()
            .with_citizen("First", Citizen::new(0.0).unwrap())
            .unwrap();
        let household = worker(&world, first).household_id().unwrap();
        let (world, second) = world
            .with_household_member(household, "Second", Citizen::new(0.0).unwrap())
            .unwrap();
        let world = world.set_household_living_tier(household, 2).unwrap();
        for id in [first, second] {
            assert_eq!(worker(&world, id).spare_clothing_target(), 1);
        }
        let housed = world.with_member_accommodation(first).unwrap();
        for id in [first, second] {
            assert_eq!(worker(&housed, id).spare_clothing_target(), 2);
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }

    fn worker(world: &Universe, id: crate::AgentId) -> &Citizen {
        let AgentKind::Citizen(worker) = &world.agents()[&id].kind;
        worker
    }

    #[test]
    fn homeless_household_and_live_member_create_no_property_or_profession_assets() {
        let original = Universe::default();
        let map = original.map();
        let (world, first) = original
            .with_citizen("First", Citizen::new(0.0).unwrap())
            .unwrap();
        assert_eq!(world.map(), map);
        let world = world.advance(HOUR).unwrap();
        let map = world.map();
        let household = worker(&world, first).household_id().unwrap();
        let (world, second) = world
            .with_household_member(
                household,
                "Second",
                Citizen::new(0.0)
                    .unwrap()
                    .with_starting_role(StartingRole::Weaver),
            )
            .unwrap();
        assert_eq!(world.map(), map);
        assert_eq!(world.households()[&household].home, None);
        assert_eq!(worker(&world, second).home(), None);
        assert_eq!(worker(&world, second).current_time_ms(), HOUR);
        assert_eq!(worker(&world, second).designated_workplace(), None);
        assert!(world.guilds().is_empty());
        assert_eq!(
            world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn new_member_shares_home_and_supplies_but_has_no_bed() {
        let (world, first) = Universe::default()
            .with_citizen("First", Citizen::new(0.0).unwrap())
            .unwrap();
        let world = world.with_member_accommodation(first).unwrap();
        let home = worker(&world, first).home().unwrap();
        let account = worker(&world, first).household_id().unwrap();
        let world = world
            .with_stored_good(
                home,
                crate::storage::GoodsOwner::Household(account),
                Good::BreadRoll,
                20,
            )
            .unwrap();
        let map = world.map();
        let (world, second) = world
            .with_household_member(account, "Second", Citizen::new(50.0).unwrap())
            .unwrap();
        assert_eq!(world.map(), map);
        assert_eq!(worker(&world, second).home(), Some(home));
        assert_eq!(worker(&world, second).sleep_place(), None);
        let sleeping = world.start_action(second, CitizenAction::Sleep).unwrap();
        assert_eq!(
            worker(&sleeping, second)
                .active_action()
                .unwrap()
                .claimed_objects()
                .count(),
            0
        );
        assert_eq!(
            sleeping,
            Universe::from_save_str(&sleeping.to_save_string().unwrap()).unwrap()
        );
        let eating = world.start_action(second, CitizenAction::Eat).unwrap();
        assert!(
            worker(&eating, second)
                .active_action()
                .unwrap()
                .meal_nutrition
                > 0.0
        );
        assert_eq!(
            world
                .map()
                .place(home)
                .unwrap()
                .objects()
                .filter(|o| matches!(o.kind, ObjectKind::Bed { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn carried_food_and_water_are_consumed_without_accommodation() {
        let citizen = Citizen::new(60.0)
            .unwrap()
            .with_thirst(60.0)
            .unwrap()
            .with_good(Good::BreadRoll, 20)
            .unwrap()
            .with_good(Good::Water, 800)
            .unwrap();
        let original_food = citizen.units(Good::BreadRoll);
        let eating = citizen.start_action(CitizenAction::Eat).unwrap();
        assert!(eating.units(Good::BreadRoll) < original_food);
        let eating = eating
            .advance(eating.active_action().unwrap().remaining_ms())
            .unwrap();
        assert!(eating.hunger() < citizen.hunger());
        let drinking = eating.start_action(CitizenAction::Drink).unwrap();
        assert!(drinking.units(Good::Water) < eating.units(Good::Water));
        let drinking = drinking
            .advance(drinking.active_action().unwrap().remaining_ms())
            .unwrap();
        assert!(drinking.thirst() < eating.thirst());
        assert_eq!(drinking.home(), None);
    }

    #[test]
    fn sleep_has_normal_recovery_and_combined_discomfort_rate() {
        let outdoors = Citizen::with_needs(-50.0, 100.0).unwrap();
        let mut bed = outdoors.with_furnished_home().unwrap();
        bed.discomfort = 50.0;
        let mut outdoors = outdoors;
        outdoors.discomfort = 50.0;
        for hours in [1, 4, 8] {
            let outside = outdoors
                .start_action(CitizenAction::Sleep)
                .unwrap()
                .advance(hours * HOUR)
                .unwrap();
            let inside = bed
                .start_action(CitizenAction::Sleep)
                .unwrap()
                .advance(hours * HOUR)
                .unwrap();
            close(outside.tiredness(), inside.tiredness());
            close(
                outside.discomfort(),
                50.0 + hours as f64 * (25.0 / 8.0 - 10.0 / 24.0),
            );
            close(
                inside.discomfort(),
                50.0 - hours as f64 * (25.0 / 8.0 + 10.0 / 24.0),
            );
        }
        close(outdoors.advance(DAY_MS).unwrap().discomfort(), 40.0);
        let mut comfortable = outdoors.clone();
        comfortable.discomfort = 0.0;
        close(
            comfortable.personal_wellbeing().unwrap() - outdoors.personal_wellbeing().unwrap(),
            50.0,
        );
    }

    #[test]
    fn discomfort_clamps_net_rate_and_survives_interruption_and_partitioning() {
        for (furnished, initial) in [(false, 0.0), (false, 99.0), (true, 1.0), (true, 100.0)] {
            let mut citizen = Citizen::new(0.0).unwrap();
            if furnished {
                citizen = citizen.with_furnished_home().unwrap();
            }
            citizen.discomfort = initial;
            let sleeping = citizen.start_action(CitizenAction::Sleep).unwrap();
            let whole = sleeping.advance(10 * HOUR).unwrap();
            let mut split = sleeping.clone();
            for _ in 0..600 {
                split = split.advance(60_000).unwrap();
            }
            close(whole.discomfort(), split.discomfort());
            assert!((0.0..=100.0).contains(&whole.discomfort()));
            let mut interrupted = sleeping.advance(HOUR).unwrap();
            let value = interrupted.discomfort();
            interrupted.abandon_action().unwrap();
            close(
                interrupted.advance(HOUR).unwrap().discomfort(),
                (value - 10.0 / 24.0).max(0.0),
            );
        }
    }

    #[test]
    fn homeless_sleep_prediction_history_and_save_match_execution() {
        let citizen = Citizen::with_needs(-50.0, 100.0).unwrap();
        let plan = planning::plan(&citizen).unwrap();
        assert!(plan.actions().contains(&CitizenAction::Sleep));
        let predicted = citizen
            .start_action(CitizenAction::Sleep)
            .unwrap()
            .advance(8 * HOUR)
            .unwrap();
        let (world, id) = Universe::default()
            .with_citizen("Sleeper", citizen)
            .unwrap();
        let sleeping = world.start_action(id, CitizenAction::Sleep).unwrap();
        let mut split = sleeping.clone();
        for _ in 0..480 {
            split = split.advance(60_000).unwrap();
        }
        let finished = sleeping.advance(8 * HOUR).unwrap();
        close(worker(&finished, id).discomfort(), predicted.discomfort());
        close(
            finished
                .citizen_history(id)
                .unwrap()
                .current
                .worst_discomfort,
            25.0 - 10.0 / 3.0,
        );
        close(
            finished
                .citizen_history(id)
                .unwrap()
                .current
                .average_wellbeing()
                .unwrap(),
            split
                .citizen_history(id)
                .unwrap()
                .current
                .average_wellbeing()
                .unwrap(),
        );
        assert_eq!(
            sleeping,
            Universe::from_save_str(&sleeping.to_save_string().unwrap()).unwrap()
        );
        let encoded = finished.to_save_string().unwrap();
        assert!(encoded.contains("worst_discomfort"));
        assert_eq!(finished, Universe::from_save_str(&encoded).unwrap());
    }

    #[test]
    fn homeless_provisioning_keeps_supplies_carried() {
        let source = Citizen::with_needs(-50.0, -50.0)
            .unwrap()
            .with_coins(1000)
            .unwrap();
        let plan = planning::plan(&source).unwrap();
        let goal = plan
            .goals()
            .iter()
            .find(|goal| goal.goal == Effect::ReplenishReserves);
        assert!(goal.is_some(), "{plan:?}");
        assert!(
            !plan
                .actions()
                .iter()
                .any(|action| matches!(action, CitizenAction::DepositStored { .. }))
        );
    }
    #[test]
    fn homeless_starting_supplies_and_planned_replenishment_stay_portable() {
        let (world, seller) = Universe::with_map(crate::locations::Map::default())
            .with_citizen(
                "Seller",
                Citizen::new(0.0)
                    .unwrap()
                    .with_good(Good::BreadRoll, 200)
                    .unwrap()
                    .with_good(Good::Water, 10000)
                    .unwrap(),
            )
            .unwrap();
        let world = world
            .start_action(seller, CitizenAction::List(Good::BreadRoll, 200))
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        let world = world
            .start_action(seller, CitizenAction::List(Good::Water, 10000))
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        let citizen = Citizen::with_needs(-50.0, -50.0)
            .unwrap()
            .with_coins(1000)
            .unwrap()
            .with_good(Good::BreadRoll, 2)
            .unwrap()
            .with_good(Good::Water, 50)
            .unwrap();
        let (world, id) = world.with_citizen("Shopper", citizen).unwrap();
        let world = world.place_starting_supplies().unwrap();
        assert_eq!(worker(&world, id).units(Good::BreadRoll), 2);
        assert_eq!(worker(&world, id).units(Good::Water), 50);
        let world = world.start_planning(id).unwrap().advance(DAY_MS).unwrap();
        let citizen = worker(&world, id);
        assert_eq!(citizen.home(), None);
        assert!(citizen.hunger() < 100.0, "hunger {}", citizen.hunger());
        assert!(citizen.thirst() < 100.0, "thirst {}", citizen.thirst());
        assert!(citizen.units(Good::BreadRoll) > 2);
        assert!(citizen.units(Good::Water) > 50);
        assert!(
            citizen
                .maintenance()
                .replenishment
                .completed_at_ms
                .is_some()
        );
        let account = citizen.household_id().unwrap();
        let history = world.household_history(account).unwrap();
        let bought = history.current.bought.iter().copied().sum::<u64>()
            + history
                .completed
                .iter()
                .flat_map(|day| day.bought)
                .sum::<u64>();
        assert!(bought > 0);
        assert_eq!(
            world,
            Universe::from_save_str(&world.to_save_string().unwrap()).unwrap()
        );
    }

    #[test]
    fn save_rejects_invalid_discomfort_and_history_bounds() {
        let (world, id) = Universe::default()
            .with_citizen("Citizen", Citizen::new(0.0).unwrap())
            .unwrap();
        for value in [-1.0, 101.0, f64::INFINITY, f64::NAN] {
            let mut invalid = world.clone();
            let AgentKind::Citizen(citizen) = &mut invalid.agents.get_mut(&id).unwrap().kind;
            citizen.discomfort = value;
            assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        }
        for value in [-1.0, 101.0] {
            let mut invalid = world.clone();
            invalid
                .citizen_histories
                .get_mut(&id)
                .unwrap()
                .current
                .worst_discomfort = value;
            assert!(Universe::from_save_str(&invalid.to_save_string().unwrap()).is_err());
        }
    }
    #[test]
    fn history_integrates_discomfort_at_both_clamps_without_partition_bias() {
        for (furnished, discomfort) in [(false, 99.0), (true, 1.0)] {
            let mut citizen = Citizen::with_needs(-50.0, 100.0).unwrap();
            citizen.discomfort = discomfort;
            if furnished {
                citizen = citizen.with_furnished_home().unwrap();
            }
            let (world, id) = Universe::default()
                .with_citizen("Sleeper", citizen)
                .unwrap();
            let sleeping = world.start_action(id, CitizenAction::Sleep).unwrap();
            let whole = sleeping.advance(8 * HOUR).unwrap();
            let mut split = sleeping;
            for _ in 0..480 {
                split = split.advance(60_000).unwrap();
            }
            let whole_day = &whole.citizen_history(id).unwrap().current;
            let split_day = &split.citizen_history(id).unwrap().current;
            close(
                whole_day.average_wellbeing().unwrap(),
                split_day.average_wellbeing().unwrap(),
            );
            close(whole_day.worst_discomfort, split_day.worst_discomfort);
            close(
                worker(&whole, id).discomfort(),
                worker(&split, id).discomfort(),
            );
        }
    }
}
