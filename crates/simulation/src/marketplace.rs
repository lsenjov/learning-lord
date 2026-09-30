#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Good {
    Berries,
    Pebbles,
}

pub fn coins_per_kg(good: Good) -> f64 {
    match good {
        Good::Berries => 1.0,
        Good::Pebbles => 2.0,
    }
}

pub fn value(good: Good, grams: f64) -> f64 {
    grams / 1000.0 * coins_per_kg(good)
}
