# Reuse market affordability calculations

- [x] Implement, independently review, and profile affordability reuse; run simulation tests and formatting before committing.

Keep purchase settlement unchanged. Reuse account results when its party, request, and budget match and the shared order root, fallback prices, and caravan purchase eligibility have not changed. Immutable order snapshots cover inserts, removals, quantity changes, and quote changes without maintaining mutation hooks. Cache state stays outside serialization and logical equality; clones retain safe immutable snapshots. Validate invalidation, removal, speculative clones, and save roundtrips, then compare simulation profiles.

Implementation and validation completed: cache reuse and invalidation tests pass, including per-account changes, shared market inputs, cloned speculative markets, and save roundtrips. Formatting and diff checks pass. Simulation suite: 638 passed, 6 ignored, 1 failed. The existing `empty_universe_advances_and_rejects_clock_overflow` test fails during `advance(0)` in `compact_construction` at `day + DAY_MS`; construction source is byte-identical to HEAD, and the failure precedes affordability refresh. No unrelated overflow fix included. Independent review found no issues. Two independent reviews found no issues, including a fresh pass after the final test assertions.

## Profiling

Same day-31 save, 24 simulated hours, identical optimized diagnostic build settings. Two quiet baseline runs: 22.638 and 22.658 seconds. Two optimized runs: 19.676 and 19.387 seconds. Mean 22.648 to 19.532 seconds: 13.8% less time. Loading and saving excluded. An additional baseline run overlapped state-comparison loading and was excluded.

Both builds retain the same diagnostic-only bypass for the deferred stale construction-context save validation failure; the original save and production loader are unchanged. End-state comparison matches trades, prices, inventories, coins, actions/plans, construction, and all other canonical fields. Differences are limited to requests, affordable requests, market history, and citizen market requests/cached markets, the same derived-demand fields already known to vary between baseline runs. No whole-state equality claim.

Artifacts: `/tmp/ll-day31-profile/{before,after}-affordability-profile`, corresponding timing logs and final RON snapshots, `compare-affordability.log`, and `affordability-stacks.txt`.

Fresh GDB sampling: affordability refresh appeared in 3/150 main-thread stacks, versus 20/150 before. These are approximate inclusive call-path observations, not precise CPU percentages. Construction context appeared in 28/150 and wellbeing averaging in 13/150; both remain outside this change.
