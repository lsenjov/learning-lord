# Nearest supplier shopping

1. Keep nearest-location shopping and price priority within each stop. Combine equal-price listings by seller before moving to another seller, and share nearest-stop selection with production input estimates. Preserve reserve food comparisons and inventory priorities. Add regression tests, run simulation/workspace checks, and obtain an independent review before committing.

Implemented shared basket routes for production estimates and actual shopping, equal-price seller grouping, and nearest equal-quote reserve food suppliers. Target affordability checks descending batch counts because larger baskets can change routes and lower costs.

Validation: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check` pass. Regression coverage includes fragmented sellers, purchase rounding and budgets, changing basket routes, insufficient supply estimates, snapshot preservation, caravan price priority, reserve supplier distance, and nonmonotonic target affordability. Independent review caught the nonmonotonic affordability issue; the fix and regression passed re-review with no remaining high, medium, or low findings.
