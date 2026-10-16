# Production economics: evidence and experiment protocol

Design analysis only, 2026-10-17. None of these proposals authorizes implementation. Baseline code is `b071e3c`; the acquisition-cost experiment `24c9899` was reverted by the user. Each numbered document is an alternative experiment, not a sequence to implement together.

## Established evidence

- `universe-1792160133.850952010.ron.gz` (day 14, Finn) exposes profitable block-making, expensive imported blocks and rejected tailory production. Earlier diagnostics found roughly 12.50 reference value per block, 18.75 import ask, and 147.41 expected seller value per garment. The garment's general reference value was about 139.22: seller revenue and wealth valuation are not identical.
- The saved candidate bought 350 g cloth and 28 blocks before three garment assemblies. A fresh decision on a diagnostic clone bought 400 g cloth and 34 blocks. These are different observations, not interchangeable quantities.
- The original 12-hour projection generated 17 blocks and seven garments: 425 g cloth plus 39 purchased blocks net. One owned block valued at 12.4995 plus seven imports costing 132 can support a garment sale estimate of 147.414384 at a small positive margin. Buying all eight costs 150 and loses money. Imported inputs are not categorically unprofitable.
- The reverted acquisition-cost experiment changed the fresh candidate to block → garment → block, but a 24-hour replay still produced zero blocks. Its whole candidate scored about 303.30 versus sleep 314.19. That demonstrates a partial correctness fix, not a successful inactivity fix.
- Newer `universe-1792191428.549461424.ron.gz` (day 16, a different universe) contains 25 caravan blocks, a rejected candidate purchasing 22, zero block requests/trades/unmet demand, and Finn sleeping. Do not mix its identifiers or scores with the day-14 universe.
- No new counterfactual simulation was run for the unimplemented options in this analysis. Expected effects below are hypotheses.

## Relevant code

- `crates/simulation/src/production.rs`: `input_value`, `output_value`, `projected_recipes_at`, `ranked_recipes_at`, `recipe_requirements_at`. Held ingredients use reference; missing ingredients use actual listings/rounding. Ratio ranking divides capacity-normalised output stock by profit per production time. Zero-stock positive-profit outputs can win even with low hourly margin.
- `crates/simulation/src/planning/goals.rs`: `production_restock_at` stocks projected requirements before `production_at_site` reranks recipes. `PRODUCTION_DURATION_MS` bounds the production candidate, including preparation, with at least one finished job. `WeightedWellbeing` uses duration-weighted endpoint averages.
- `crates/simulation/src/households.rs`: `refresh_workplace_supplies` combines workplace workers' short thresholds and daily targets. Acquisition policies must preserve account ownership and incoming deliveries.
- `crates/simulation/src/lib.rs`: `economic_wealth` values goods at reference, not seller asking prices or acquisition cost. `WEALTH_WELLBEING_PER_COIN` is 0.1.
- `crates/simulation/src/marketplace.rs`: the daily reference moves 25% towards completed local quote VWAP; without local trades it stays unchanged. Seller markdowns are separate, depend on stock versus recent movement after an initial 48 hours, and ordinary asks move halfway down towards their seller baseline. Caravan asks do not use these local markdowns.
- `record_blocked_purchase` records unsuccessful proposed supply; available but rejected alternative purchases do not automatically create demand. There are no binding buyer bids establishing immediate resale value.

## Shared acceptance protocol

1. Use both named saves, preserving originals; record commit, seed, initial time and policy. Cold versus warm planning measurements must be separated.
2. Capture candidate actions, purchase quantities, cash spent, input-cost estimates, wealth before/after shopping, working time, chosen goal, sleep, actual production, sales and imports. Showing a profitable label is insufficient.
3. Run deterministic scenario tests: imports only; tiny cheap listing plus expensive depth; no supply; exhausted supply; seller self-listing; mixed owned/purchased/produced ingredients; shared workplace; equal prices; per-seller rounding; taxes and supported tariff changes; competing food needs.
4. Compare identical snapshots, plus 24-hour and seven-day replays. Check cash/goods conservation, source immutability, save round-trip, no repeated failed procurement and no unexplained demand amplification.
5. Report median/p95 decision time, candidate count and allocations where measurable. A provisional investigation trigger is >2x p95 or candidate count on the same fixture, not a guaranteed performance budget. More correct profitable activity can change later world workloads; distinguish same-state overhead from longer-run behaviour.
6. Choose one policy variable per experiment. Keep reference, restock size, scoring and ratio changes separate unless a prerequisite is explicitly justified. Avoid using forced sleep restrictions to mask production economics.
7. Independent implementation review and meaningful tests are required before any future commit. Plans/reviews here do not substitute for that review.

## Baseline policy correction

- Current `CaravanPolicy` has unsigned 0–100% import/export tariffs. Negative subsidies discussed in earlier conversation are not supported by this checkout; do not claim subsidy-exhaustion simulation tests. A negative table fixture is not implemented economic policy.
- Reference revaluation also affects asset taxes and food/drink choices. Existing local seller states need not rise with reference, while new seller/trial baselines can inherit it. Thus raising references can reduce acquisition loss and simultaneously worsen input opportunity costs or increase cash obligations.
