use super::{ProductionPrices, Recipe, crop_kind, usable_output};
use crate::{
    Citizen, Coins, Quantity,
    locations::PlaceId,
    marketplace::{Good, ShoppingList},
};

const MAX_ROUTE_BATCHES: usize = 128;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ProductionRouteQuote {
    pub profit: f64,
    #[serde(default)]
    pub net_revenue: f64,
    pub duration_ms: u64,
    pub purchase_coins: Coins,
    pub purchases: ShoppingList,
    pub intermediate_batches: Vec<(Recipe, Quantity)>,
    #[serde(default)]
    pub ordered_batches: Vec<Recipe>,
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
        &ProductionPrices::snapshot(citizen),
    )
}

pub(super) fn method_routes(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
    export: bool,
    prices: &ProductionPrices,
) -> ProductionRoutes {
    method_routes_with_owned(recipe, citizen, site, export, prices, &prices.owned)
}

pub(super) fn method_routes_with_owned(
    recipe: Recipe,
    citizen: &Citizen,
    site: Option<PlaceId>,
    export: bool,
    prices: &ProductionPrices,
    owned: &[Quantity; Good::COUNT],
) -> ProductionRoutes {
    let site = site.or_else(|| citizen.production_place(recipe).ok());
    let buy = quote(recipe, citizen, site, export, false, prices, owned);
    let make = site
        .filter(|_| {
            recipe
                .inputs()
                .iter()
                .any(|&(good, _)| supplier(good, citizen, site).is_some())
        })
        .and_then(|_| quote(recipe, citizen, site, export, true, prices, owned))
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

struct RouteBuilder<'a> {
    citizen: &'a Citizen,
    site: Option<PlaceId>,
    made: [Quantity; Good::COUNT],
    needed: [Quantity; Good::COUNT],
    owned: [Quantity; Good::COUNT],
    owned_value: f64,
    prices: &'a ProductionPrices,
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
        let owned = remaining.min(self.owned[index]);
        self.owned[index] -= owned;
        remaining -= owned;
        self.owned_value +=
            self.prices.inputs[index] * owned as f64 / good.units_per_price_unit() as f64;
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
    prices: &ProductionPrices,
    owned: &[Quantity; Good::COUNT],
) -> Option<ProductionRouteQuote> {
    citizen.validate_production_site(site?, recipe).ok()?;
    let mut builder = RouteBuilder {
        citizen,
        site,
        made: [0; Good::COUNT],
        needed: [0; Good::COUNT],
        owned: *owned,
        owned_value: 0.0,
        prices,
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
    if purchases
        .items()
        .any(|(good, units)| !prices.purchasable[good as usize] && units > 0)
    {
        return None;
    }
    let fees = builder
        .batches
        .iter()
        .map(|&recipe| {
            citizen.workshop_fee_cost(site, recipe, recipe.duration_ms(citizen).unwrap_or(0))
        })
        .sum::<f64>();
    let purchase_coins = finite_purchase_cost(prices, purchases)?;
    let surplus_value = Good::ALL
        .into_iter()
        .map(|good| {
            let units = builder.made[good as usize];
            if units == 0 {
                return Some(0.0);
            }
            let producer = supplier(good, citizen, site)?;
            let output_units = producer
                .outputs()
                .iter()
                .find(|(output, _)| *output == good)?
                .1;
            let cap = super::stock_cap_batches(producer, citizen)? * output_units as f64;
            let stock = prices.stock.as_ref().map_or_else(
                || super::production_stock(good, citizen),
                |stock| stock[good as usize],
            );
            let local_units = (cap - stock).clamp(0.0, units as f64);
            let export_price = if citizen.market().caravan_policy().exports[good as usize].allowed {
                citizen
                    .market()
                    .effective_export_threshold(good)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let value = (local_units * prices.outputs[good as usize]
                + (units as f64 - local_units) * export_price)
                / good.units_per_price_unit() as f64;
            Some(
                value
                    * (1.0
                        - f64::from(
                            citizen
                                .sale_income_basis_points
                                .get(&(citizen.selling_place(), good))
                                .copied()
                                .unwrap_or(0),
                        ) / 10_000.0),
            )
        })
        .try_fold(0.0, |sum, value| Some(sum + value?))?;
    let net_revenue = prices.revenue(recipe, citizen, site, export)? + surplus_value;
    let profit = net_revenue - purchase_coins as f64 - builder.owned_value - fees;
    if !profit.is_finite() {
        return None;
    }
    let ordered_batches = builder.batches.clone();
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
        net_revenue,
        duration_ms: builder.duration_ms,
        purchase_coins,
        purchases,
        intermediate_batches,
        ordered_batches,
    })
}

fn finite_purchase_cost(prices: &ProductionPrices, remaining: ShoppingList) -> Option<Coins> {
    let cost: f64 = remaining
        .items()
        .map(|(good, units)| {
            prices.inputs[good as usize] * units as f64 / good.units_per_price_unit() as f64
        })
        .sum();
    (cost.is_finite() && cost >= 0.0 && cost < Coins::MAX as f64).then_some(cost.ceil() as Coins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentId, StartingRole, locations::Location, marketplace::Prices, production::Skill,
    };

    #[test]
    fn surplus_is_included_in_route_margin_revenue_and_pays_income_tax() {
        let citizen = Citizen::new(0.0)
            .unwrap()
            .with_starting_role(StartingRole::Baker)
            .with_skill_practice_ms(Skill::Milling, Some(0))
            .unwrap();
        let (map, site) = citizen
            .map()
            .with_place(
                Location::Bakery,
                citizen.position(),
                Some(citizen.id()),
                "Bakery",
            )
            .unwrap();
        let mut citizen = citizen.with_map(map).unwrap();
        let room = &mut citizen.map.places_mut().get_mut(&site).unwrap().buildings[0].rooms[0];
        room.object_capacity += 1;
        room.objects.push(crate::locations::Object::new(
            crate::locations::ObjectKind::for_recipe(Recipe::MillFlour),
        ));
        let mut prices = ProductionPrices::snapshot(&citizen);
        prices.inputs = [0.0; Good::COUNT];
        prices.outputs = [0.0; Good::COUNT];
        prices.owned = [0; Good::COUNT];
        prices.purchasable = [true; Good::COUNT];
        prices.minimum_margin = 0.1;
        prices.inputs[Good::Wheat as usize] = 950.0 / 0.3;
        prices.inputs[Good::Flour as usize] = 900.0 / 0.14;
        prices.outputs[Good::Flour as usize] = 900.0 / 0.14;
        prices.outputs[Good::BreadRoll as usize] = 10.0;
        let route = quote(
            Recipe::BakeRolls,
            &citizen,
            Some(site),
            false,
            true,
            &prices,
            &prices.owned,
        )
        .unwrap();
        assert!((route.net_revenue - 1000.0).abs() < 1e-8);
        assert!(route.profit / route.net_revenue < prices.minimum_margin);
        assert!(
            prices
                .preferred_route(
                    Recipe::BakeRolls,
                    &citizen,
                    Some(site),
                    false,
                    &prices.owned
                )
                .is_none()
        );
        citizen
            .sale_income_basis_points
            .insert((citizen.selling_place(), Good::Flour), 5000);
        let taxed = quote(
            Recipe::BakeRolls,
            &citizen,
            Some(site),
            false,
            true,
            &prices,
            &prices.owned,
        )
        .unwrap();
        assert!((taxed.net_revenue - 550.0).abs() < 1e-8);
        let mut stock = [0.0; Good::COUNT];
        stock[Good::Flour as usize] = f64::MAX;
        prices.stock = Some(stock);
        let mut policy = citizen.market.caravan_policy();
        policy.exports[Good::Flour as usize].allowed = false;
        citizen.market.set_caravan_policy(policy).unwrap();
        let capped = quote(
            Recipe::BakeRolls,
            &citizen,
            Some(site),
            false,
            true,
            &prices,
            &prices.owned,
        )
        .unwrap();
        assert!((capped.net_revenue - 100.0).abs() < 1e-8);
        prices.inputs[Good::Wheat as usize] = 1.0;
        assert!(
            prices
                .preferred_route(
                    Recipe::BakeRolls,
                    &citizen,
                    Some(site),
                    false,
                    &prices.owned
                )
                .is_some()
        );
    }

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
    fn route_costs_value_owned_inputs_and_make_only_missing_batches() {
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
                let varied = citizen
                    .with_good(Good::FlaxBlock, blocks)
                    .unwrap()
                    .with_good(Good::Cloth, cloth)
                    .unwrap();
                let routes = production_routes(Recipe::AssembleGarment, &varied, Some(site));
                assert_eq!(routes.buy.as_ref().unwrap().profit, -60.0);
                if blocks == 8 {
                    assert!(routes.make.is_none());
                } else {
                    let make = routes.make.as_ref().unwrap();
                    assert_eq!(
                        make.profit,
                        100.0 - blocks as f64 * 20.0 - (8 - blocks) as f64
                    );
                    assert_eq!(
                        make.intermediate_batches,
                        vec![(Recipe::MakeClothingBlock, 8 - blocks)]
                    );
                }
                assert_eq!(
                    production_routes(
                        Recipe::AssembleGarment,
                        &varied.with_coins(0).unwrap(),
                        Some(site)
                    ),
                    routes
                );
            }
        }
        let full = citizen
            .with_good(Good::FlaxBlock, 8)
            .unwrap()
            .with_coins(0)
            .unwrap();
        assert!(
            !super::super::ranked_recipes_at(&full, Some(site)).contains(&Recipe::AssembleGarment)
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
    fn method_prices_use_lowest_snapshot_quote_without_consuming_market_depth() {
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
        assert_eq!(routes.buy.unwrap().purchase_coins, 8);
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
            1
        );
    }

    #[test]
    fn method_estimates_allow_partial_supply_but_actual_purchases_use_cash() {
        let (citizen, site) = tailor();
        let citizen = listed(citizen, Good::FlaxBlock, 4, 10.0)
            .with_good(Good::FlaxBlock, 4)
            .unwrap()
            .with_coins(40)
            .unwrap();
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert_eq!(routes.buy.as_ref().unwrap().purchase_coins, 40);
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
            4
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
            vec![(Recipe::MakeClothingBlock, 4)]
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
    fn short_projection_finishes_its_first_qualified_chain() {
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
        let mut expected = vec![Recipe::MakeClothingBlock; 8];
        expected.push(Recipe::AssembleGarment);
        assert_eq!(recipes, expected);
        let requirements =
            super::super::recipe_requirements_at(&citizen, Some(site), &recipes).unwrap();
        assert_eq!(requirements.units(Good::FlaxBlock), 0);
        assert_eq!(requirements.units(Good::Cloth), 200);
    }

    #[test]
    fn reference_valuation_does_not_create_an_unlisted_buy_route() {
        let (citizen, site) = tailor();
        let citizen = listed(citizen, Good::Cloth, 1, 40.0);
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert!(routes.buy.is_none());
        assert!(routes.make.is_some());
        assert!(routes.prefer_make);
        assert_eq!(routes.make.unwrap().purchase_coins, 8);
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
    fn no_purchase_route_leaves_reference_valuation_but_no_production() {
        let (citizen, site) = tailor();
        let routes = production_routes(Recipe::AssembleGarment, &citizen, Some(site));
        assert!(routes.buy.is_none());
        assert!(routes.make.is_none());
        assert!(
            super::super::recipe_profit_for_site(Recipe::AssembleGarment, &citizen, Some(site))
                .is_some()
        );
        assert!(
            !super::super::ranked_recipes_at(&citizen, Some(site))
                .contains(&Recipe::AssembleGarment)
        );
    }
}
