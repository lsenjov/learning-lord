# Integer economy migration

Approved model: goods use whole u64 units and currency uses signed i64 coins. Bulk goods have gram units; bread and pie have counted units with weights 100g and 125g. Quoted prices remain positive f64 in coins/kg for bulk goods and coins/item for counted goods. Purchases aggregate actual costs per seller and good before rounding up, and transfer no money for zero stock.

## Completed integrated step: simulation and desktop economy

- Add per-good unit, weight, nutrition, eating duration and price conversion metadata.
- Convert inventories, recipes, market quantities, trades, reserves and production targets to integer units; use checked transfers.
- Share settlement calculations across purchasing and affordability, aggregate seller rounding, preserve raw mark-to-market wealth.
- Reserve whole meals when eating starts, deliver nutrition progressively, and spoil the remaining meal on interruption.
- Preserve planning schedules, independent recipe alternatives, and goal ordering while rounding food requirements to whole units.
- Migrate simulation tests and add deterministic rounding, conservation, meal and overflow regressions.
- Integrate the desktop changes below, review, fix findings, validate, then commit.

Desktop presentation:

- Format bulk goods in g or kg as appropriate and counted goods as loaves/pies throughout inventories, production diagnostics and market views.
- Show integer coins and price units; update history volume/price chart conversions.
- Validate workspace formatting, clippy, tests and bounded simulation performance; review and commit.

Meal selection first uses available inventory, choosing the cheapest quoted value per nutrition among single foods individually able to provide at least 50 nutrition; ties use catalogue order. A pie supplies 60 nutrition. Explicit eating with insufficient food preserves a partial meal fallback. Revisit this selection when additional needs are introduced. Reserved meal nutrition and derived market value remain part of wellbeing and wealth until progressively consumed; its goods are unavailable immediately.

## Validation

- `cargo test --workspace --locked`: 216 tests passed, including `initial_town_runs_production_and_real_market_transactions` (three simulated days).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- Native desktop smoke checked integer starting coins and berries/kg, bread/loaf, and pie/pie price and chart labels; the test process was stopped after capture.

Independent Sol-6.1 code review passed with no unresolved high or medium findings. Documentation findings were corrected.
