# Production restock price snapshots

## Agreed policy

Production planning uses fixed competitive prices. Inputs use the lowest purchasable quote excluding own offers, outputs the lowest listed quote, with reference valuation when absent. Owned inputs use market valuation. Internally produced missing ingredients use their complete production route costs and work time, without also charging their market value. A profitable combined route may qualify even when its individual finished recipe or prerequisites fail standalone margin. Each agent's existing 1–10% adjustment sets both minimum net-revenue profit margin and input purchase tolerance. Commercial trials must meet that margin.

A twelve-hour stock-aware production schedule retains recipe stock soft caps and export pricing above those caps. Supply acquisition uses fixed ceilings, buys cheapest first, permits partial fulfillment, and ends when complete or no eligible purchases remain. A new restock recomputes prices. Unlisted inputs have reference valuation but no purchase ceiling. Household shopping remains unchanged.

## Steps

1. Implement and review fixed-price production route planning and committed restock acquisition together. Complete routes may qualify despite unprofitable individual components, so flattening those routes and trusting their qualification during acquisition/execution must land together for runnable behavior. Include standalone market helpers, bounds, historical-trial handling, and integration tests. Commit after independent review and checks.
2. Run full simulation checks, replay benchmarks and CPU profile against a fresh baseline. Record outcomes and commit plan completion.

## Validation

Targeted tests cover reference fallbacks, own listing exclusion for input buying, output competitive asks, margin boundaries with tax/fees, commercial trials, stock caps/export pricing, intermediate inputs, exhausted offers, fixed ceilings across stops, and partial restock completion. Run the simulation suite and workspace compilation. Review all valid medium/high findings, fix them, and request a fresh independent review before each implementation commit.

## Progress

- Initial architecture inspection: projections currently mutate a cloned market; workplace restock has cached-target and recipe-prefix branches; purchase bounds already serialize in primitive actions.
- Implementation complete and independently reviewed: fixed quote snapshots, complete-route margins and work schedules, partial acquisition, and persisted purchase bounds.
- Reviewed fixes include unavailable buy-route detection, owned-input valuation without duplicate manufacturing cost, surplus revenue in the margin denominator, stock-aware export pricing and income tax for surplus, and shared-workplace incoming/own supply accounting with save migration.
- Full simulation suite passed: 573 tests (386 unit, 187 integration), 6 ignored. Workspace checks including all targets passed.
- No outstanding high or medium review findings. The low documentation finding for historical trial eligibility was fixed.
- Implementation committed as `f1c65a5`; the reviewed snapshot-reuse correction is `29f6b2f`.
- Fresh replay baseline median: 12.419088 seconds (three runs).

- Initial implementation replay median: 12.639605 seconds versus 12.419088 seconds baseline. Profiling found repeated complete price snapshots from historical sale-replenishment checks. A reviewed follow-up reuses the existing projection snapshot for batch costs and eligibility, preserving historical trial and invalid-cost behavior. Follow-up validation passed all 387 unit tests; independent review found no issues.

## Final replay and profile

Both plan steps are complete. Release builds use debug symbols and frame pointers. The replay loads the same 30-citizen save (`saves/universe-1792711378.877687606.ron.gz`, decompressed for the harness), advances 24 simulated hours from 122400000 ms, and excludes save loading and initial cloning from timing.

| Version | Three elapsed times (seconds) | Median |
| --- | --- | --- |
| Baseline `ee1c40f` | 12.448758, 12.419088, 12.392095 | 12.419088 |
| Final `29f6b2f` | 7.027865, 7.066181, 7.101955 | 7.066181 |

The final median is 43.1% lower. This is a same-save workload comparison: the new production policy changes subsequent decisions, so it is not an isolated microbenchmark of identical actions.

A final 199 Hz user CPU profile over two replays confirms the repeated eligibility snapshot hotspot is gone. Snapshot construction still accounts for approximately 29% inclusive samples; market-order iteration accounts for approximately 23% self samples. Further quote aggregation or indexing is a separate possible work unit, not part of this change. Raw local replay/profile artifacts are under `target/restock-profile/`.

Validation: the full simulation suite passed 573 tests before the cost-reuse extraction; afterward all 387 unit tests passed, including the additional frozen-cost regression. Workspace all-target checks passed, the final release build is warning-free, and independent review cleared the implementation and performance correction.
