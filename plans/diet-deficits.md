# Diet deficits

Implement and review one coherent step: extend the four dietary categories to -100 through 100 with 25 per day decay and cumulative deficit penalties, consistently updating consumption, forecasts, history, inactive tier handling, save validation and existing display. Keep alcohol, hunger, nutritional values, prices and reserve targets unchanged.

Validate threshold continuity, clamps, partition independence, inactive tier transitions, save bounds and a neglected sweets meal regression. Run simulation tests and desktop checks before independent review and commit.

Validation complete: `cargo test -p learning-lord-simulation` passed 753 tests with 7 ignored; library portion passed 567 with 6 ignored. The focused desktop diet display regression passed, `cargo check -p learning-lord` passed, and formatting and diff checks passed. Threshold continuity, analytic forecast integrals, partition independence, negative history, save bounds, tier demotion/reactivation and unchanged alcohol behavior are covered.

Read-only validation of `saves/universe-1794375554.755380027.ron.gz` showed Arlo chooses fish plus cabbage with sweets at zero, then 78 g berries plus cabbage when sweets alone is changed to -25, -50, -75 or -100. Prices, stocks and other needs were held constant. User saves were not modified.

Independent production and fresh final reviews found no high or medium issues. Existing endpoint-score tests now isolate inactive dietary tiers and use decision scores; construction deadline coverage supplies full dietary variety and accepts preserved history after a valid later departure.
