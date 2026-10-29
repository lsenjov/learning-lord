# Production market snapshots

## Scope

One implementation step: aggregate listed ownership and eligible purchase/listing quotes in one market traversal per citizen. Share immutable prices and production stock between short and daily workplace supply projections; each projection keeps independent mutable inventory, crop/brewing capacity and tax state. Preserve refresh timing, access rules, arithmetic, and route selection. No persistent caches or save-load changes.

## Implementation and review

Implemented in marketplace.rs, production.rs and households.rs. Scalar-equivalence tests cover own/foreign/home listings, missing quotes, household stock, reserved stock and existing production-stock fixtures. Both horizons are compared against independent snapshots, including repeated reuse. Independent review approved without findings. One diagnostic build-script flag mismatch was corrected before measurement so both profiling harnesses use frame pointers.

## Verification

- Simulation: 448 passed, 6 ignored.
- Desktop production tests: 6 passed, including the ten-day town simulation.
- Formatting and whitespace checks pass.
- Day-31 version-16 save benchmark: 24 simulated hours took 28.689 seconds before and 19.676 seconds after (31.4% less time, about 1.46x throughput). A second baseline took 29.168 seconds. Timing excludes loading, saving and rendering; no competing build/simulation workload ran during timed advancement.
- User-mode CPU sampling after the change: sell-order iteration accounts for 15.71% of samples, down from 29.66% in the preceding profile. Sampled advancement took 19.185 seconds; percentages are sampled self CPU, not inclusive call-path costs.
- Full-state comparison was investigated rather than normalized: original-vs-original runs already differ in derived market demand budgets, requests, affordable requests, and the daily Wood unmet-demand count. Original-vs-optimized differs in those same fields. Canonical orders, trades, prices, actual citizen state/actions/plans, inventories, coins, map, financial histories, construction, taxes and all other fields compare exactly. Consequently no strict whole-Universe equality claim is made.
- Both profiling builds use the same temporary diagnostic loader outside the repository, bypassing only the known stale construction-context equality rejection without changing loaded state. All other validation remains. The user explicitly deferred the save-load bug; production loading code and the original save are untouched.

## Artifacts

Temporary profiling artifacts are in `/tmp/ll-day31-profile`: `before-profile`, `after-profile`, `before-final.ron`, `before-repeat.ron`, `after-final.ron`, `compare-repeat.log`, `compare-detail.log`, `after-leaf.perf`, and `after-cpu-summary.json`. The saved fixture is not committed.
