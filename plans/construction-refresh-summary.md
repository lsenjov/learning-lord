# Construction refresh summary

1. Implement and commit a reviewed temporary per-refresh summary for account escrow, quote wood commitments, wood reserves, prices, and current carpenter contracts. Keep wide wood totals until contract exclusion and invalidate available wood when sequential refresh changes a carpenter's context. Compare with the original scalar calculation for household quotes, bidding and active work, sequential access changes, and overflow. Run simulation tests and formatting, complete independent review and profiling, resolve findings, then commit.

No persistent cache, save format changes, construction rule changes, or deferred save-load fix.

## Completed

Implemented ephemeral account/carpenter summaries. Preserves per-account quote traversal order, subtracts complete contract contributions from wide wood totals before saturation, and invalidates available wood after a carpenter context changes. The original scalar calculation remains test-only.

Validation: 453 simulation unit tests passed, 6 ignored. Equivalence fixtures cover household multiple quotes, bidding/active contracts, sequential storage access changes, saturated wood allocations, and escrow/reserve overflow. Formatting and diff checks pass.

Independent review found one medium issue: eagerly summing escrow for accounts without citizens could introduce an overflow error. Restricted sums to represented citizen economic accounts and added a regression case. Fresh independent review found no issues.

## Profile

Same day-31 save, 24 simulated hours per run, opt-level 2 simulation and harness, loading/saving excluded. Two quiet baseline runs: 16.968 and 16.634 seconds. Two optimized runs: 13.595 and 13.187 seconds. Mean 16.801 to 13.391 seconds, a 20.3% reduction. An initial run overlapping compilation was excluded.

Both temporary profiling builds use the same diagnostic bypass of only the known stale construction-context equality load rejection. Production loading code and the source save are unchanged. This is a simulation advancement benchmark, not UI frame timing.

Artifacts: /tmp/ll-construction-profile, including before-profile, after-profile, save.ron, before-clean.log, before-repeat.log, after-clean.log, after-repeat.log, and build logs.
