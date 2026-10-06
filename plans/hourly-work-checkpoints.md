# Hourly work checkpoints

One implementation step: compare outer Production and Increase wealth goals at hourly checkpoints through the four-hour horizon, completing the first crossing job and retaining shorter exhausted paths. Keep primitive execution, quantity-driven prerequisite acquisition, queue ordering, targets and commit boundaries unchanged. Add focused regression coverage and update README rules; compare the same exported Finn diagnostic fixture before and after, then run workspace tests, formatting and strict Clippy. Obtain an independent review, resolve high/medium findings and documentation findings, and commit the completed step.

## Validation and measurement

The focused goal suite passes 40 tests, including hourly forage with travel, skill-dependent batch durations, one batch satisfying several checkpoints, early caps and input exhaustion, exact combined baskets for buying and gathering paths, and unchanged quantity-driven prerequisite orders. Strict workspace Clippy (`--all-targets -- -D warnings`) passes.

The Finn fixture was extracted from the previously generated diagnostic module for `debug/universe-1791287397.611676701.txt`. Both versions ran the same reconstructed map, current orders, carried inventory, prices and calculated targets under the same optimized development/test build. The snapshot does not reconstruct active/history requests or work accounts; this is an approximate idle reconstruction, not an exact replay of the live town. Temporary instrumentation counted continuation-search calls and was removed afterward.

| Finn tailoring skill | Baseline time / search calls | Hourly time / search calls |
| --- | --- | --- |
| 24.6 | 4.188 s / 100,043 | 1.283 s / 13,335 |
| 1 (override after target calculation, same targets) | 1.608 s / 46,725 | 0.610 s / 12,662 |

At skill 24.6, first-goal Production alternatives fell from 17 to 4 and wealth alternatives from 8 to 4. The selected plan changed from 23 to 22 actions; coarser candidate lengths can change choices. These are individual local measurements, not a latency guarantee.

The full workspace suite passes all 235 tests, and `cargo fmt --all -- --check` passes. Independent Sol-6.1 review reports no high, medium or low documentation issues.
