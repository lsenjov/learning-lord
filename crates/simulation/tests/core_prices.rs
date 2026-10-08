use learning_lord_simulation::{
    marketplace::{Good, Prices},
    production::Recipe,
};

#[test]
fn core_prices_cover_every_recipe_with_labour_and_recursive_material_markup() {
    let prices = Prices::core();
    assert_eq!(prices, Prices::default());
    for recipe in Recipe::ALL {
        let &[(good, quantity)] = recipe.outputs() else {
            panic!("single output expected")
        };
        let labour = recipe.base_duration_ms() as f64 / 3_600_000.0 * 10.0;
        let materials: f64 = recipe
            .inputs()
            .iter()
            .map(|&(input, units)| prices.value(input, units).unwrap())
            .sum();
        let expected = (labour + materials) * 1.3;
        assert!((prices.value(good, quantity).unwrap() - expected).abs() < 1e-10);
        assert!(good.core_price().is_finite() && good.core_price() > 0.0);
    }
    assert_eq!(Good::Berries.core_price(), 650.0);
    assert_eq!(Good::Wheat.core_price(), 65.0);
    assert!((Good::FlaxGarment.core_price() - 136.4948).abs() < 1e-10);
}

#[test]
fn core_thresholds_do_not_follow_local_price_changes() {
    let local = Prices::default().with_price(Good::BreadRoll, 0.2).unwrap();
    assert_eq!(local.price(Good::BreadRoll), Some(0.2));
    assert_eq!(
        Good::BreadRoll.export_threshold(),
        Good::BreadRoll.core_price() * 0.5
    );
    assert_eq!(
        Good::BreadRoll.import_threshold(),
        Good::BreadRoll.core_price() * 1.5
    );
}

#[test]
fn bread_roll_batches_preserve_food_and_production_economics() {
    assert_eq!(Recipe::BakeRolls.outputs(), &[(Good::BreadRoll, 10)]);
    assert_eq!(Good::BreadRoll.weight_grams(), 20);
    assert_eq!(Good::BreadRoll.nutrition_per_unit(), Some(10.0));
    assert!((Good::BreadRoll.core_price() * 5.0 - 20.055208333333333).abs() < 1e-10);
    assert_eq!(
        Recipe::BakeRolls.inputs(),
        &[(Good::Flour, 100), (Good::Wood, 25), (Good::Water, 100)]
    );
}
