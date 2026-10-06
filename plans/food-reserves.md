# Food reserves

Add a diminishing wellbeing bonus for carried edible food and let normal plan scoring choose replenishment. Listed food earns no reserve bonus. Preserve bonus-bearing food and production inputs when listing excess.

## Cohesive implementation milestone

- Add the shared food reserve curve and 300 nutrition cap; use the cap in excess and production reservations. Verify boundaries, mixed foods, listed exclusions, and eating.
- Add ReplenishReserves candidates for 100, 200, and 300 nutrition through existing acquisition/travel prediction, exact purchase baskets, full plan scoring, and goal labels. Verify affordable replenishment above a meal's nutrition.
- Report food demand only from the selected executing acquisition, including travel preceding its purchase. Preserve production input shortages. Verify request lifecycle and prediction isolation, update docs, and run simulation/desktop and realistic town checks.

Review the combined implementation and prior authorized production target draft, resolve medium/high findings, and validate before one cohesive commit. Implementation agent leaves staging and commits to the coordinating agent.

## Validation

The cohesive implementation is complete. Exact continuation caching retains separate first-goal reports and substitutes their prefix labels when reusing a suffix; a cache-disabled exhaustive reference and a targeted boundary test check equivalence. The simulation crate uses optimization level 2 in development builds while preserving the existing algorithm, assertions, worker deadlines, and search horizons.

The normal desktop suite passed all 30 tests. One instrumented three-day town completed in 6.76 seconds, with 352 ms initial planning, a 575 ms slowest simulation step, and 43 real trades including bread or pie. Random geometry and yields can produce more expensive plans: a separate optimized release sample took 12.15 seconds and had a 6.76 second slowest step. Temporary diagnostic instrumentation was removed.
