# Backward goal planning and trading

- Finish the pending five-minute Buy berries and Sell pebbles actions against the unlimited fixed-price marketplace.
- Describe actions by resource inputs and effects. Build hunger alternatives by recursively satisfying berry, coin, and pebble requirements, using expected gathering yields.
- Search from hunger, tiredness, and wealth goals. Forward-simulate alternatives and retain the highest average completion-wellbeing variant for each goal before exploring subsequent goals.
- Bound each backward goal construction to four hours, allowing the action crossing the limit but no further prerequisites. Reject unsatisfied branches.
- Start goals while the combined plan is under four hours and complete the entire final goal, even beyond the horizon.
- Keep primitive completion scoring, execution, the two-hour commitment and food replan check. Preserve immediate smaller meals when available.
- Prediction-local cooldowns run from completion: Eat four hours, Buy berries and Sell pebbles separately two hours. Each new plan resets all cooldowns.
- Accept local goal-variant selection as a deliberate heuristic rather than global optimality.

## Implementation step

1. Finish trading, replace exhaustive primitive search with resource-driven goal construction and goal sequencing, update documentation and tests, run checks and performance probes, review using code-review-skill with an independent agent, fix findings, and commit the combined change.

## Progress

- Plan recorded before implementation. This supersedes the unfinished primitive-search work in `plans/trading.md`.
- Implemented resource-driven goal construction, local variant selection, goal sequencing, prediction-local trade cooldowns, and five-minute trading. Removed the temporary primitive-search cache.
- All 75 workspace tests pass; formatting, Clippy with warnings denied, workspace build, and diff whitespace checks pass.
- Local debug benchmarks take approximately 0.2–1.9 ms per plan for empty and stocked citizens, compared with 50–400 ms for the interim primitive trading search.
- Independent code-review-skill review identified one medium rounding issue in resource requirements. Added a scaled floating-point tolerance and regressions, keeping exact no-borrowing execution. Re-review found no remaining high, medium, or low issues.
