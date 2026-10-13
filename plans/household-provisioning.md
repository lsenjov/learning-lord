# Household provisioning and consumption at home

One implementation step, followed by independent review, behavioral validation, a fixed-universe five-day performance comparison, and a commit.

## Agreed behavior

- Eating and drinking occur only at home, using usable household home storage. A citizen can drink alongside eating. Carried supplies must be delivered before consumption.
- Replenishment acquires and delivers household supplies. Scale quantities by household size, including one-person households. Respect ownership, tax/work reservations, existing stock, and committed delivery; retain coordination so household members do not independently buy the same shortfall.
- Food triggers below 100 nutrition per member and targets 300. Drink triggers below 100 thirst relief per member (400 grams) and targets 300 (1,200 grams).
- Diversity triggers when fewer than three categories have 25 stored dietary points per member. Choose an affordable basket bringing three categories toward 75 points per member, using actual shortfalls and all contributions of mixed foods and beer. The fourth category has no minimum. The nutrition target does not prevent diversification of a pantry already full of one food.
- One shortage initiates provisioning; consider other household shortfalls on the same trip. Prefer basic food/drink coverage before extra reserves, diversity, or a spare garment. Accept partial deliveries when supplies or money are limited. Reassess category choices during replenishment rather than creating continuous price-driven trips.
- Include one shared spare garment as an opportunistic purchase during provisioning. Immediate clothing need remains distinct from the spare.
- Ensure urgent shortages can recover despite the maintenance cooldown. Evaluate useful acquisitions through delivery and preserve meaningful travel costs. Consolidate acquisition behavior instead of retaining a parallel hunger-shopping policy.
- Shopping uses pantry composition; individual meal selection still uses hunger and dietary levels. Adapt forecasting to home supplies, depleting stock between meals, without a persistent cache or changes to dietary decay.

## Implementation and verification

1. Inspect the current acquisition, maintenance, consumption, and forecasting paths; implement the cohesive behavior and update existing fixtures affected by the intentional rule change. Add focused tests for home-only consumption, combined eating/drinking, household scaling, ownership/reservations, three-category choice, expensive-category exclusion, partial delivery, urgent recovery, shared coordination, and the optional spare.
2. Run workspace tests, strict Clippy, formatting, and whitespace checks. Obtain a new independent review; resolve valid high/medium findings and request a fresh review after fixes. Fix low documentation issues and report remaining low issues.
3. Reuse the saved fresh universe from `debug/consumption-availability-2026-10-13/fresh.ron` for five-day measurements. Record hunger/thirst, dietary variety, travel/productive activity, delivery behavior and CPU cost where existing observations permit. Final state need not equal the previous version because behavior intentionally changes. Document limitations and results, then commit this completed implementation step.

## Results

Implemented home-only eating and drinking, shared pantry replenishment, and home-stock food forecasts. Reserve wellbeing now counts usable home supplies plus active consumption; carried stock retains its wealth value but gains reserve value on delivery. This makes delivery useful to the planner. Removed the previous separate hunger acquisition/replenishment policy instead of retaining parallel implementations.

Provisioning compares the four category triples after actual acquisition and delivery, counts eligible owned stock before purchase costs, and ranks fulfillment of each intended triple. It preserves demand from the selected basket, including when no delivery is possible, without publishing the omitted fourth category. Partial gathering/purchases remain deliverable. Optional spare garments are limited to an existing acquisition stop. Excess listing retains pantry variety. Existing saves can finish active consumption; queued away-from-home consumption replans.

Independent reviews identified and resolved incomplete gathering, unattainable basket selection, owned-stock costing, garment detours, missing selected demand, and the no-delivery demand case. The final independent review approved the implementation and subsequent compatibility/cleanup changes. Two stale README descriptions were corrected. No outstanding review findings.

Validation: `cargo test --workspace --no-fail-fast` passed 637 tests, with 7 manual workloads ignored. Strict workspace/all-target Clippy passed. Formatting and whitespace checks passed. The final equivalent iterator cleanup was independently approved; all 310 simulation unit tests passed again on the final source before the profiling build.

The initial fixture SHA-256 is `9ec393f090993b37a1d55a35aa57b3d7e727e798eeac265da850231815e52072`. Baseline is commit `862e7af`, after the previous availability optimization: three normal development-build runs had median five-day advancement 53.089914 seconds and median process user CPU 123.761916 seconds. Benchmarks use the same fixture and build settings; changes in simulated behavior are intentional, so equality is checked between new runs rather than against the old final state.


### Five-day comparison

Three final development-build runs took 13.216596, 13.208911 and 13.240242 seconds of advancement. Median: **13.216596 seconds**, a **75.1% reduction** from 53.089914 seconds. Median process user CPU fell from 123.761916 to **28.006522 seconds**, a **77.4% reduction**. Both repeat runs matched the first new final Universe exactly. Builds, tests and sampling were kept separate from timing runs.

This is a behavior-changing comparison, not an isolated microbenchmark: it includes less acquisition search, home-only meal forecasts, and different simulated actions. It covers one fixed ten-citizen fresh town for five days in the development build; it does not establish performance for every save or a release build.

| Five-day observation | Before | After |
| --- | ---: | ---: |
| Worst hunger in recorded histories | -6.14 | 1.08 |
| Worst thirst in recorded histories | 12.39 | -0.02 |
| Worst tiredness in recorded histories | 16.09 | 8.06 |
| Production, citizen-hours | 650.57 | 651.83 |
| Travel, citizen-hours | 104.86 | 111.90 |
| Trading, citizen-hours | 33.25 | 32.17 |
| Mean final meat dietary points | 6.35 | 32.39 |
| Mean final carbs dietary points | 26.17 | 37.40 |
| Mean final fibre dietary points | 4.96 | 3.66 |
| Mean final sweets dietary points | 13.20 | 13.32 |

All seven activity categories sum to 1,200 citizen-hours in both snapshots. Travel rose by about 6.7%; production stayed essentially unchanged. The pantry diversity target does not guarantee three sustained personal dietary bars: meals and decay still determine those, and one category may be omitted. Final dietary points are a snapshot, not a time average. No severe hunger or thirst developed in this workload.

Artifacts (ignored local debug output): `debug/household-provisioning-2026-10-13/` contains timing logs/JSON, final saves, behavioral measurements and validation logs. Baseline fixture, saves, timing runs and profile remain in `debug/consumption-availability-2026-10-13/`.


The final frame-pointer profile used `perf record -e cpu-clock:u -F 99 --call-graph fp`, with inlining disabled only in report symbol expansion, and preserved the new expected final Universe. It captured 2,715 samples with no lost samples. Inclusive food-forecast CPU fell from approximately 43.04 to 6.16 seconds (34.68% to 22.47% of samples). Remaining large inclusive paths are production-prefix planning (40.07%, approximately 10.99 CPU seconds) and workplace-supply refresh (37.09%, approximately 10.17 seconds), with usable-owned-stock queries at 31.71%. These paths overlap and must not be added. Sampling is approximate; small or inlined functions may not appear separately. The next performance investigation should examine workplace-supply/production queries, not add a persistent meal cache. The final profile, collapsed stacks, comparison JSON and flamegraph are in the household debug artifact directory.

Reproduce the timed workload with the normal desktop test binary and `LL_PROFILE_FIXTURE` set to the saved fresh fixture, running `simulation::tests::fresh_universe_five_day_profile --exact --ignored --nocapture`. `LL_PROFILE_OUTPUT` writes a new final save; set `LL_PROFILE_EXPECTED` to the first new final save on repeat runs. `benchmark.py` and `metrics.py` in the artifact directory preserve the measurement and extraction commands.
