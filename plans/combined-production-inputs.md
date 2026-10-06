# Combined production inputs

1. Prepare each bounded production queue prefix as one input requirement, retaining actual batch actions, recipe profitability order, unavailable-recipe fallback, the four-hour preparation/start budget, and the final crossing batch. Use the same preparation for repeated edible-recipe reserve acquisition. Preserve withdrawals, self-production, purchases, stock/funds limits, and cooldowns. Document the behavior, obtain independent Sol-6.1 review, resolve findings, and commit the cohesive change.

Validation:
- Cover repeated and mixed recipe procurement, reserve batches and edible ingredient consumption, insufficient stock and fallback, cooldowns, skill advancement, and horizon boundaries.
- Run formatting, workspace tests, Clippy, and a bounded existing three-day town scenario.

Complete: implementation and five regressions are complete. Workspace tests pass, including the town acceptance test. A separate town run under a 60-second bound passed in 7.03 seconds with 43 trades. Formatting, diff checks and Clippy pass. Independent Sol-6.1 review found no high, medium or low issues.

Each candidate buys only its own required inputs. Daily production targets remain unchanged.
