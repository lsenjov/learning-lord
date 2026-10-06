# Unique goals within a planning window

The daily production floor should maintain saleable stock for twelve hours of skill-adjusted production, after carried/listed goods, personal food reserves and active production are accounted for. That approved change remains part of this delivery; its broader target queues exposed excessive repeated production branching.

## One implementation milestone

Deliver the stock floor and bounded goal search together. Verification subitems:

- Keep the stock-floor calculation and its regression coverage. Generate production candidates as feasible prefixes of a queue ordered once at the goal's start by current profit per production time. Read remaining target counts from each predicted state; include repeated batches, travel and all existing input acquisition alternatives. Include the first prefix reaching or crossing four hours and stop extending that path.
- Allow each named top-level goal at most once per forecast. Recursive Food/Coins prerequisites remain resources, and primitive actions may repeat within a goal. Reset availability with each new planning call. Include used goals in both continuation-cache hashing and collision equality. Preserve complete four-hour forecasts, full duration-weighted scoring, eight-hour sleep completion, two-hour execution commitments and existing meal/trade cooldowns and listing priority.
- Validate cache reuse against exhaustive search under different prefix labels, collision equality, unique goals and replan reset, production recipe ordering/caps/repeated batches/crossing, existing stock-floor tests and the full workspace. Measure the existing three-day town smoke test with simulation opt-level 2. Obtain a fresh Sol-6.1 review, fix medium/high findings and repeat review until clean. Report remaining low findings before continuing. Commit this milestone after review and performance validation.

Implementation preserves the preceding approved stock-floor changes. No horizon shortening or heuristic pruning is introduced.

Milestone complete: all 180 workspace tests pass, including 15 stock-floor regressions and the added goal/cache/production coverage. Formatting and `cargo clippy --workspace --all-targets -- -D warnings` pass. A standalone three-day town smoke test passed in 9.95 seconds under a 60-second timeout, with 49 trades and 39 orders. Fresh independent Sol-6.1 review found no high, medium or low issues.
