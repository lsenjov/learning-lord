use super::{Recipe, crop_kind, output_value, usable_output};
use crate::{
    Citizen, Coins, Quantity,
    locations::PlaceId,
    marketplace::{Good, ShoppingList},
};

const MAX_ROUTE_BATCHES: usize = 128;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionRouteQuote {
    pub profit: f64,
    pub duration_ms: u64,
    pub purchase_coins: Coins,
    pub purchases: ShoppingList,
    pub intermediate_batches: Vec<(Recipe, Quantity)>,
}

impl ProductionRouteQuote {
    pub fn profit_per_hour(&self) -> f64 {
        self.profit * 3_600_000.0 / self.duration_ms as f64
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ProductionRoutes {
    pub site: Option<PlaceId>,
    pub buy: Option<ProductionRouteQuote>,
    pub make: Option<ProductionRouteQuote>,
    pub prefer_make: bool,
}

pub(crate) fn production_routes(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
) -> ProductionRoutes {
    method_routes(
        recipe,
        citizen,
        site,
        super::stock_fraction(recipe, citizen).is_some_and(|fraction| fraction >= 1.0),
    )
}

fn method_routes(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
    export: bool,
) -> ProductionRoutes {
    let site = site.or_else(|| citizen.production_place(recipe).ok());
    let buy = quote(recipe, citizen, site, export, false);
    let make = site
        .filter(|_| {
            recipe
                .inputs()
                .iter()
                .any(|&(good, _)| supplier(good, citizen, site).is_some())
        })
        .and_then(|_| quote(recipe, citizen, site, export, true))
        .filter(|route| !route.intermediate_batches.is_empty());
    let prefer_make = make.as_ref().is_some_and(|make| {
        buy.as_ref()
            .is_none_or(|buy| make.profit_per_hour() > buy.profit_per_hour())
    });
    ProductionRoutes {
        site,
        buy,
        make,
        prefer_make,
    }
}

pub(super) fn preferred_manufacturing(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
    owned: &[Quantity; Good::COUNT],
    export: bool,
) -> Option<ProductionRouteQuote> {
    let site = site.or_else(|| citizen.production_place(recipe).ok());
    if !has_missing_supplier(recipe, citizen, site, owned) {
        return None;
    }
    let routes = method_routes(recipe, citizen, site, export);
    routes
        .make
        .filter(|route| routes.prefer_make && route.profit > 0.0)
}

fn supplier(good: Good, citizen: &Citizen, site: Option<PlaceId>) -> Option<Recipe> {
    let site = site?;
    let mut producers = Recipe::ALL.into_iter().filter(|&recipe| {
        recipe.outputs().iter().any(|&(output, _)| output == good)
            && crop_kind(recipe).is_none()
            && recipe != Recipe::BrewBeer
            && recipe.duration_ms(citizen).is_ok()
            && citizen.validate_production_site(site, recipe).is_ok()
    });
    let recipe = producers.next()?;
    producers.next().is_none().then_some(recipe)
}

fn has_missing_supplier(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
    owned: &[Quantity; Good::COUNT],
) -> bool {
    recipe.inputs().iter().any(|&(good, units)| {
        units > owned[good as usize] && supplier(good, citizen, site).is_some()
    })
}

struct RouteBuilder<'a> {
    citizen: &'a Citizen,
    site: Option<PlaceId>,
    made: [Quantity; Good::COUNT],
    needed: [Quantity; Good::COUNT],
    tax_carry: [u64; Good::COUNT],
    stack: Vec<Recipe>,
    batches: Vec<Recipe>,
    duration_ms: u64,
}

impl RouteBuilder<'_> {
    fn consume(&mut self, good: Good, units: Quantity, make: bool) -> Option<()> {
        let index = good as usize;
        let mut remaining = units;
        let made = remaining.min(self.made[index]);
        self.made[index] -= made;
        remaining -= made;
        if remaining == 0 {
            return Some(());
        }
        if let Some(recipe) = make
            .then(|| supplier(good, self.citizen, self.site))
            .flatten()
        {
            while self.made[index] < remaining {
                let before = self.made[index];
                self.produce(recipe, make)?;
                // A fully taxed output cannot satisfy an ingredient, even after more work.
                if self.made[index] == before && self.tax_carry[index] == 0 {
                    return None;
                }
            }
            self.made[index] -= remaining;
        } else {
            self.needed[index] = self.needed[index].checked_add(remaining)?;
        }
        Some(())
    }

    fn produce(&mut self, recipe: Recipe, make: bool) -> Option<()> {
        if self.stack.contains(&recipe) || self.batches.len() >= MAX_ROUTE_BATCHES {
            return None;
        }
        self.stack.push(recipe);
        for &(good, units) in recipe.inputs() {
            self.consume(good, units, make)?;
        }
        self.stack.pop();
        self.duration_ms = self
            .duration_ms
            .checked_add(recipe.duration_ms(self.citizen).ok()?)?;
        if self.batches.len() >= MAX_ROUTE_BATCHES {
            return None;
        }
        self.batches.push(recipe);
        if self.stack.is_empty() {
            return Some(());
        }
        for &(good, units) in recipe.outputs() {
            self.made[good as usize] = self.made[good as usize].checked_add(usable_output(
                recipe,
                good,
                units,
                self.citizen,
                self.site,
                &mut self.tax_carry,
            ))?;
        }
        Some(())
    }
}

fn quote(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
    export: bool,
    make: bool,
) -> Option<ProductionRouteQuote> {
    citizen.validate_production_site(site?, recipe).ok()?;
    let mut builder = RouteBuilder {
        citizen,
        site,
        made: [0; Good::COUNT],
        needed: [0; Good::COUNT],
        tax_carry: [0; Good::COUNT],
        stack: Vec::new(),
        batches: Vec::new(),
        duration_ms: 0,
    };
    builder.produce(recipe, make)?;
    let purchases = ShoppingList::new(
        Good::ALL
            .into_iter()
            .map(|good| (good, builder.needed[good as usize])),
    )
    .ok()?;
    let fees = builder
        .batches
        .iter()
        .map(|&recipe| {
            citizen.workshop_fee_cost(site, recipe, recipe.duration_ms(citizen).unwrap_or(0))
        })
        .sum::<f64>();
    let purchase_coins = finite_purchase_cost(citizen, purchases)?;
    let surplus_value = Good::ALL
        .into_iter()
        .map(|good| citizen.prices().value(good, builder.made[good as usize]))
        .try_fold(0.0, |sum, value| Some(sum + value?))?;
    let profit =
        output_value(recipe, citizen, site, export)? + surplus_value - purchase_coins as f64 - fees;
    if !profit.is_finite() {
        return None;
    }
    builder.batches.pop();
    let intermediate_batches = Recipe::ALL
        .into_iter()
        .filter_map(|recipe| {
            let count = builder
                .batches
                .iter()
                .filter(|&&produced| produced == recipe)
                .count() as Quantity;
            (count > 0).then_some((recipe, count))
        })
        .collect();
    Some(ProductionRouteQuote {
        profit,
        duration_ms: builder.duration_ms,
        purchase_coins,
        purchases,
        intermediate_batches,
    })
}

fn finite_purchase_cost(citizen: &Citizen, mut remaining: ShoppingList) -> Option<Coins> {
    let mut market = citizen.market().clone();
    let mut position = citizen.position();
    let mut cost: Coins = 0;
    while let Some((place, basket)) = market.next_purchase_stop(
        citizen.market_party(),
        &citizen.map(),
        position,
        remaining,
        None,
    ) {
        for (good, units) in basket.items() {
            let purchase = market
                .purchase(
                    citizen.market_party(),
                    place,
                    (good, units),
                    Coins::MAX,
                    citizen.current_time_ms(),
                    false,
                )
                .ok()?;
            if purchase.units != units {
                return None;
            }
            cost = cost.checked_add(purchase.coins)?;
        }
        remaining = ShoppingList::new(
            remaining
                .items()
                .map(|(good, units)| (good, units.saturating_sub(basket.units(good)))),
        )
        .ok()?;
        position = citizen.map().position(place);
    }
    remaining.items().next().is_none().then_some(cost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentId, StartingRole, locations::Location, marketplace::Prices, production::Skill,
    };

    #[test]
    fn supplier_matches_global_lookup_across_skills_and_site_access() {
        let citizen = Citizen::new(0.0).unwrap();
        let foreign_owner = AgentId(uuid::Uuid::new_v4());
        let mut map = citizen.map().clone();
        for owner in [citizen.id(), citizen.id(), foreign_owner] {
            for kind in [
                Location::Field,
                Location::Mill,
                Location::Bakery,
                Location::Weavery,
                Location::Tailory,
                Location::Brewery,
            ] {
                map = map
                    .with_place(kind, citizen.position(), Some(owner), "Workshop")
                    .unwrap()
                    .0;
            }
        }
        let public = map.public_place(Location::Market);
        map = map
            .with_building(public, Location::Tailory, citizen.id(), "Public workshop")
            .unwrap()
            .0;
        let (next_map, town) = map
            .with_place(Location::Warehouse, citizen.position(), None, "Town")
            .unwrap();
        map = next_map
            .with_building(town, Location::Tailory, citizen.id(), "Town workshop")
            .unwrap()
            .0;
        let citizen = citizen.with_map(map).unwrap();
        let mut sites: Vec<_> = citizen.map().places().keys().copied().map(Some).collect();
        sites.extend([None, Some(PlaceId(uuid::Uuid::new_v4()))]);
        let mut unskilled = citizen.clone();
        for skill in Skill::ALL {
            unskilled = unskilled.with_skill_practice_ms(skill, None).unwrap();
        }
        let mut workers = vec![unskilled.clone()];
        for skill in Skill::ALL {
            for practice in [0, super::super::MAX_SKILL_PRACTICE_MS] {
                workers.push(
                    unskilled
                        .with_skill_practice_ms(skill, Some(practice))
                        .unwrap(),
                );
            }
        }
        let mut guild_worker = citizen.clone();
        guild_worker.guilds.insert(
            foreign_owner,
            crate::guilds::GuildAccess {
                workshops: sites.iter().flatten().copied().collect(),
                hourly_fee: 2,
            },
        );
        for skill in Skill::ALL {
            guild_worker = guild_worker.with_skill_practice_ms(skill, Some(0)).unwrap();
        }
        workers.push(guild_worker);
        for (worker_index, citizen) in workers.iter().enumerate() {
            for &site in &sites {
                for good in Good::ALL {
                    let previous = site.and_then(|site| {
                        let mut producers =
                            citizen.available_production_recipes().filter(|&recipe| {
                                crop_kind(recipe).is_none()
                                    && recipe != Recipe::BrewBeer
                                    && citizen.validate_production_site(site, recipe).is_ok()
                                    && recipe.outputs().iter().any(|&(output, _)| output == good)
                            });
                        let recipe = producers.next()?;
                        producers.next().is_none().then_some(recipe)
                    });
                    assert_eq!(
                        supplier(good, citizen, site),
                        previous,
                        "worker {worker_index}, {site:?}, {good:?}"
                    );
                }
            }
        }
        assert_eq!(supplier(Good::Berries, &unskilled, Some(public)), None);
        assert_eq!(
            supplier(
                Good::Berries,
                &unskilled,
                Some(unskilled.map().public_place(Location::Forest))
            ),
            Some(Recipe::Forage)
        );
        assert_eq!(
            supplier(Good::FlaxBlock, workers.last().unwrap(), Some(public)),
            Some(Recipe::MakeClothingBlock)
        );
        assert_eq!(
            supplier(Good::FlaxBlock, workers.last().unwrap(), Some(town)),
            None
        );
    }

    fn tailor() -> (Citizen, PlaceId) {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Tailor);
        let (map, site) = citizen
            .map()
            .with_place(
                Location::Tailory,
                citizen.position(),
                Some(citizen.id()),
                "Tailory",
            )
            .unwrap();
        let citizen = citizen
            .with_map(map)
            .unwrap()
            .with_coins(1000)
            .unwrap()
            .with_prices(
                Prices::default()
                    .with_price(Good::Cloth, 40.0)
                    .unwrap()
                    .with_price(Good::FlaxBlock, 10.0)
                    .unwrap()
                    .with_price(Good::FlaxGarment, 100.0)
                    .unwrap(),
            );
        (citizen, site)
    }

    fn listed(citizen: Citizen, good: Good, units: Quantity, price: f64) -> Citizen {
        let mut market = citizen.market().clone();
        market
            .list_at_quote(
                AgentId(uuid::Uuid::new_v4()),
                citizen.selling_place(),
                good,
                units,
                price,
            )
            .unwrap();
        citizen.with_market(market)
    }

    #[test]
    fn method_estimates_ignore_inventory_and_cash_but_use_complete_batches() {
        let (citizen, site) = tailor();
        let citizen = listed(
            listed(citizen, Good::Cloth, 1000, 40.0),
            Good::FlaxBlock,
            1000,
            20.0,
        );
        let expected = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert_eq!(expected.buy.as_ref().unwrap().profit, -60.0);
        let make = expected.make.as_ref().unwrap();
        assert_eq!(make.profit, 92.0);
        assert_eq!(make.duration_ms, 9 * 30 * 60_000);
        assert_eq!(make.purchase_coins, 8);
        assert_eq!(
            make.intermediate_batches,
            vec![(Recipe::MakeClothingBlock, 8)]
        );
        assert!(expected.prefer_make);
        for blocks in [0, 4, 8] {
            for cloth in [0, 200] {
                for coins in [0, 1000] {
                    let varied = citizen
                        .with_good(Good::FlaxBlock, blocks)
                        .unwrap()
                        .with_good(Good::Cloth, cloth)
                        .unwrap()
                        .with_coins(coins)
                        .unwrap();
                    assert_eq!(
                        production_routes(Recipe::AssembleGarment, &varied, Some(site)),
                        expected
                    );
                }
            }
        }
        let full = citizen
            .with_good(Good::FlaxBlock, 8)
            .unwrap()
            .with_coins(0)
            .unwrap();
        assert!(
            super::super::ranked_recipes_at(&full, Some(site)).contains(&Recipe::AssembleGarment)
        );
        let produced = full
            .start_action(crate::CitizenAction::ProduceAt {
                place: site,
                recipe: Recipe::AssembleGarment,
            })
            .unwrap()
            .advance(30 * 60_000)
            .unwrap();
        assert_eq!(produced.coins(), 0);
        assert_eq!(produced.available_units(Good::FlaxGarment), 1);
    }

    #[test]
    fn prices_finite_depth_with_rounded_seller_payments_and_groups_route_leaf_purchases() {
        let (citizen, site) = tailor();
        let citizen = listed(
            listed(
                listed(citizen, Good::FlaxBlock, 1, 1.0),
                Good::FlaxBlock,
                7,
                20.0,
            ),
            Good::Cloth,
            200,
            1.1,
        );
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert_eq!(routes.buy.unwrap().purchase_coins, 141);
        let make = routes.make.unwrap();
        assert_eq!(make.purchase_coins, 1);
        assert_eq!(make.purchases.units(Good::Cloth), 200);
        assert_eq!(make.profit, 99.0);
        let (split, site) = tailor();
        let split = listed(listed(split, Good::Cloth, 100, 1.0), Good::Cloth, 100, 1.0);
        assert_eq!(
            production_routes(Recipe::AssembleGarment, &split, Some(site))
                .make
                .unwrap()
                .purchase_coins,
            2
        );
    }

    #[test]
    fn method_estimates_require_full_finite_supply_but_actual_shortfalls_use_cash() {
        let (citizen, site) = tailor();
        let citizen = listed(citizen, Good::FlaxBlock, 4, 10.0)
            .with_good(Good::FlaxBlock, 4)
            .unwrap()
            .with_coins(40)
            .unwrap();
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert!(routes.buy.is_none());
        assert!(routes.make.is_none());
        assert!(!routes.prefer_make);
        assert!(
            super::super::ranked_recipes_at(&citizen, Some(site))
                .contains(&Recipe::AssembleGarment)
        );
        let basket = ShoppingList::new([(Good::FlaxBlock, 4)]).unwrap();
        let bought = citizen
            .start_action(crate::CitizenAction::BuyAt {
                place: site,
                list: basket,
            })
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert_eq!(bought.coins(), 0);
        assert_eq!(bought.available_units(Good::FlaxBlock), 8);
        let poor = citizen.with_coins(39).unwrap();
        let bought = poor
            .start_action(crate::CitizenAction::BuyAt {
                place: site,
                list: basket,
            })
            .unwrap()
            .advance(crate::TRADE_DURATION_MS)
            .unwrap();
        assert!(bought.available_units(Good::FlaxBlock) < 8);
        let enough = listed(citizen, Good::Cloth, 200, 40.0)
            .with_coins(0)
            .unwrap();
        assert_eq!(
            production_routes(Recipe::AssembleGarment, &enough, Some(site))
                .make
                .unwrap()
                .purchase_coins,
            8
        );
    }

    #[test]
    fn intermediate_taxes_add_work_and_skill_changes_total_work() {
        let (citizen, site) = tailor();
        let mut citizen = listed(citizen, Good::Cloth, 400, 40.0);
        citizen.site_socage_rates.insert(
            (site, Good::FlaxBlock),
            crate::taxation::TaxRate::new(5000).unwrap(),
        );
        let route = production_routes(Recipe::AssembleGarment, &citizen, Some(site))
            .make
            .unwrap();
        assert_eq!(
            route.intermediate_batches,
            vec![(Recipe::MakeClothingBlock, 16)]
        );
        assert_eq!(route.duration_ms, 17 * 30 * 60_000);
        assert_eq!(route.profit, 84.0);
        let skilled = citizen
            .with_skill_practice_ms(Skill::Tailoring, Some(super::super::MAX_SKILL_PRACTICE_MS))
            .unwrap();
        assert_eq!(
            production_routes(Recipe::AssembleGarment, &skilled, Some(site))
                .make
                .unwrap()
                .duration_ms,
            route.duration_ms / 2
        );
        citizen.site_socage_rates.insert(
            (site, Good::FlaxBlock),
            crate::taxation::TaxRate::new(10_000).unwrap(),
        );
        assert!(
            production_routes(Recipe::AssembleGarment, &citizen, Some(site))
                .make
                .is_none()
        );
    }

    #[test]
    fn other_workplaces_and_workers_do_not_supply_a_route() {
        let (citizen, site) = tailor();
        let citizen = citizen
            .with_skill_practice_ms(Skill::Weaving, Some(0))
            .unwrap()
            .with_good(Good::Thread, 200)
            .unwrap();
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
        assert!(
            production_routes(Recipe::AssembleGarment, &citizen, Some(site))
                .make
                .is_none()
        );
        let baker = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_skill_practice_ms(Skill::Milling, Some(0))
            .unwrap()
            .with_good(Good::Wheat, 300)
            .unwrap();
        let (map, bakery) = baker
            .map()
            .with_place(
                Location::Bakery,
                baker.position(),
                Some(baker.id()),
                "Bakery",
            )
            .unwrap();
        let (map, _) = map
            .with_place(Location::Mill, baker.position(), Some(baker.id()), "Mill")
            .unwrap();
        let baker = baker.with_map(map).unwrap();
        assert!(
            production_routes(Recipe::BakeRolls, &baker, Some(bakery))
                .make
                .is_none()
        );
    }

    #[test]
    fn preferred_manufacturing_defers_shopping_until_regular_work_supplies_it() {
        let (citizen, site) = tailor();
        let citizen = listed(
            listed(citizen, Good::Cloth, 10000, 40.0),
            Good::FlaxBlock,
            1000,
            20.0,
        )
        .with_good(Good::Berries, 10000)
        .unwrap()
        .with_good(Good::Water, 10000)
        .unwrap();
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert!(routes.prefer_make);
        assert!(
            !super::super::ranked_recipes_at(&citizen, Some(site))
                .contains(&Recipe::AssembleGarment)
        );
        let recipes = super::super::projected_recipes_at(
            &citizen,
            Some(site),
            super::super::DAILY_CAPACITY_MS,
        )
        .unwrap();
        let first_garment = recipes
            .iter()
            .position(|&recipe| recipe == Recipe::AssembleGarment)
            .unwrap();
        assert!(first_garment > 0);
        let requirements =
            super::super::recipe_requirements_at(&citizen, Some(site), &recipes).unwrap();
        assert!(requirements.units(Good::Cloth) > 0);
        // Any bought blocks must belong to the actual ordered work, never a hypothetical route.
        let mut balance = 0;
        let mut required = 0;
        for &recipe in &recipes {
            if recipe == Recipe::MakeClothingBlock {
                balance += 1;
            }
            if recipe == Recipe::AssembleGarment {
                balance -= 8;
                required = required.max(-balance);
            }
        }
        assert_eq!(requirements.units(Good::FlaxBlock), required as Quantity);
        assert_eq!(requirements.units(Good::FlaxBlock), 0);
        assert!(first_garment >= 8);
        let later = super::super::recipe_requirements_at(
            &citizen,
            Some(site),
            &[Recipe::AssembleGarment, Recipe::MakeClothingBlock],
        )
        .unwrap();
        assert_eq!(later.units(Good::FlaxBlock), 8);
    }

    #[test]
    fn owned_partial_inputs_reduce_actual_work_without_changing_the_method() {
        let (citizen, site) = tailor();
        let citizen = listed(
            listed(citizen, Good::Cloth, 10000, 40.0),
            Good::FlaxBlock,
            1000,
            20.0,
        )
        .with_good(Good::FlaxBlock, 4)
        .unwrap()
        .with_good(Good::Cloth, 100)
        .unwrap()
        .with_coins(0)
        .unwrap();
        assert_eq!(
            production_routes(Recipe::AssembleGarment, &citizen, Some(site))
                .make
                .unwrap()
                .intermediate_batches,
            vec![(Recipe::MakeClothingBlock, 8)]
        );
        let recipes = super::super::projected_recipes_at(
            &citizen,
            Some(site),
            super::super::DAILY_CAPACITY_MS,
        )
        .unwrap();
        assert_eq!(
            recipes
                .iter()
                .position(|&recipe| recipe == Recipe::AssembleGarment),
            Some(4)
        );
        let requirements =
            super::super::recipe_requirements_at(&citizen, Some(site), &recipes[..5]).unwrap();
        assert_eq!(requirements.units(Good::Cloth), 100);
        assert_eq!(requirements.units(Good::FlaxBlock), 4);
        assert!(
            requirements
                .items()
                .all(|(good, units)| units <= citizen.usable_owned_units(good))
        );
    }

    #[test]
    fn short_projection_starts_ordinary_blocks_without_buying_a_longer_chain() {
        let (citizen, site) = tailor();
        let citizen = listed(
            listed(citizen, Good::Cloth, 10000, 40.0),
            Good::FlaxBlock,
            1000,
            20.0,
        );
        let recipes = super::super::projected_recipes_at(
            &citizen,
            Some(site),
            super::super::PRODUCTION_BLOCK_MS,
        )
        .unwrap();
        assert_eq!(recipes, vec![Recipe::MakeClothingBlock; 4]);
        let requirements =
            super::super::recipe_requirements_at(&citizen, Some(site), &recipes).unwrap();
        assert_eq!(requirements.units(Good::FlaxBlock), 0);
        assert_eq!(requirements.units(Good::Cloth), 100);
    }

    #[test]
    fn infeasible_ordinary_upstream_work_falls_back_to_profitable_buying() {
        let (citizen, site) = tailor();
        let prices = citizen.prices().with_price(Good::FlaxBlock, 0.1).unwrap();
        let mut citizen = listed(
            listed(citizen, Good::Cloth, 200, 40.0)
                .with_good(Good::Cloth, 200)
                .unwrap()
                .with_prices(prices),
            Good::FlaxBlock,
            100,
            12.0,
        );
        assert!(production_routes(Recipe::AssembleGarment, &citizen, Some(site)).prefer_make);
        assert!(
            super::super::recipe_profit_for_site(Recipe::MakeClothingBlock, &citizen, Some(site))
                .unwrap()
                < 0.0
        );
        citizen
            .claim_production_trial(Recipe::MakeClothingBlock, Some(site))
            .unwrap();
        citizen
            .start_trial_production(crate::CitizenAction::ProduceAt {
                place: site,
                recipe: Recipe::MakeClothingBlock,
            })
            .unwrap();
        assert!(!citizen.eligible_production_trial(Recipe::MakeClothingBlock, Some(site)));
        assert!(
            super::super::ranked_recipes_at(&citizen, Some(site))
                .contains(&Recipe::AssembleGarment)
        );
        let recipes = super::super::projected_recipes_at(
            &citizen,
            Some(site),
            super::super::PRODUCTION_BLOCK_MS,
        )
        .unwrap();
        assert!(!recipes.contains(&Recipe::MakeClothingBlock));
        assert!(recipes.contains(&Recipe::AssembleGarment));
        assert!(
            super::super::recipe_requirements_at(&citizen, Some(site), &recipes)
                .unwrap()
                .units(Good::FlaxBlock)
                > 0
        );
    }

    #[test]
    fn equal_profit_rates_prefer_direct_buying() {
        let (citizen, site) = tailor();
        let citizen = citizen.with_prices(
            citizen
                .prices()
                .with_price(Good::FlaxGarment, 80.0)
                .unwrap(),
        );
        let citizen = listed(
            listed(citizen, Good::Cloth, 200, 40.0),
            Good::FlaxBlock,
            8,
            9.0,
        );
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert_eq!(
            routes.buy.as_ref().unwrap().profit_per_hour(),
            routes.make.as_ref().unwrap().profit_per_hour()
        );
        assert!(!routes.prefer_make);
    }

    #[test]
    fn workshop_fees_reduce_profit_without_requiring_upfront_cash() {
        let (citizen, site) = tailor();
        let mut citizen = listed(citizen, Good::Cloth, 200, 40.0)
            .with_coins(0)
            .unwrap();
        let guild = AgentId(uuid::Uuid::new_v4());
        let place = citizen.map.places_mut().get_mut(&site).unwrap();
        place.owner = Some(guild);
        for building in &mut place.buildings {
            building.owner = guild;
        }
        citizen.guilds.insert(
            guild,
            crate::guilds::GuildAccess {
                workshops: vec![site],
                hourly_fee: 2,
            },
        );
        let make = production_routes(Recipe::AssembleGarment, &citizen, Some(site))
            .make
            .unwrap();
        assert_eq!(make.purchase_coins, 8);
        assert_eq!(make.profit, 83.0);
    }

    #[test]
    fn unavailable_make_route_preserves_existing_direct_demand_estimate() {
        let (citizen, site) = tailor();
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert!(routes.buy.is_none());
        assert!(routes.make.is_none());
        assert!(
            super::super::recipe_profit_for_site(Recipe::AssembleGarment, &citizen, Some(site))
                .is_some()
        );
        assert!(
            super::super::ranked_recipes_at(&citizen, Some(site))
                .contains(&Recipe::AssembleGarment)
        );
    }
}
