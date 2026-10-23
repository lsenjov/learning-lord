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
- Profiling and implementation commit are coordinated by the root agent.
- Fresh replay baseline median: 12.419088 seconds (three runs).
