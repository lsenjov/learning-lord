# Shared production stock ratios

- Replace demand-driven production targets and crop allocation with a shared stock policy.
- Compare stock fractions of twelve-hour capacity targets, weighted by profit per working hour.
- Amortise staged crop labor across its growth days; count recoverable expected harvest once.
- Use local output prices below each target and permitted effective export prices above it. Inputs retain local replacement costs.
- Keep preparation and production searches bounded and retain future crop labor checks.
- Verify stock balancing, export fallback, crop pipeline accounting, and ingredient affordability; run existing simulation and desktop checks.
- Obtain independent review, fix findings, and commit the completed step.

Baseline: existing seven-profession ten-day probe completed in 12.34 seconds (12,327 ms internal measurement), producing 192 fish and 40 cabbages; consumed 102 fish and five cabbages.

## Implementation

- One shared ordering covers gathering, workshop batches, and new crop planting. It chooses the smallest `(stock / capacity cap) / profit per working hour`, breaking ties by higher profit. Existing crop care stays ahead of new production: tending and harvesting have no additional material cost to recover the investment already made.
- Stock caps use fractional capacity at current skill. Crop caps amortise total cycle work over growth days. Owned carried stock (including still-owned Socage reservations), owned listings, active output, and recoverable planted harvests count once. Active inputs are deducted from projected stock.
- Planting counts promised output immediately. Remaining crop care promises only yield achievable after travel and skill-adjusted work before deadlines; active tending uses its remaining duration. Crops with no recoverable tending promise no harvest.
- Local output reference prices apply below the cap; permitted tariff-adjusted export thresholds apply above it. Full-batch input replacement costs retain actual accessible purchase quotes and seller rounding, regardless of ingredients already owned. Positive export margins allow further production; export eligibility does not promise actual caravan purchases.
- Ingredient budgets remain finite and refresh at replanning. Where exports cannot profitably continue, purchases are limited to the remaining local stock shortfall. Alternative recipes continue to share the maximum ingredient requirement, rather than summing every recipe's budget.
- The production planner builds one sequence through its two-hour boundary and finishes the crossing job. Consecutive workshop batches combine purchases; crop work ends the current purchasing segment. Infeasible recipes fall back to the next ranked option. Goal-pair evaluation and prerequisite duration limits remain intact.
- Production diagnostics include crop work as well as ordinary batches. Crop stock caps do not masquerade as ingredient batch budgets.

## Validation

- Added direct regressions for capacity-normalised ratios across whole fish and grams of water, local/export cap crossings, export tariffs and disabled goods, ingredient replacement costs, amortised crop caps, active planting counted once, missed tending deadlines/travel/active tending, shared crop-versus-forage ordering, and same-day ingredient-budget refresh.
- Updated previous demand/floor tests to the shared policy. Combined ingredient purchasing, exact action replay, bounded final jobs, and stock-shortage fallback remain covered.
- The dietary-shopping settlement fixture now uses cheap fish and cabbage so that it tests actual purchase-request settlement independently of commercial production competition; all exact basket, travel and clearing assertions are retained.
- Targeted production-target integration suite: 18 passed. Targeted dietary-shopping settlement regression: passed. Workspace checks and ten-day performance comparison are recorded after independent review below.
- Full simulation unit, integration and documentation tests pass; all 104 desktop tests pass. Workspace formatting, strict Clippy and whitespace checks pass.
- The ten-day town probe completed in 10.89 seconds (10,874 ms internal measurement), versus the 12.34-second baseline. It produced 166 fish and 40 cabbages, consuming 93 fish and 21 cabbages. This single-run comparison includes changed simulated behaviour, rather than isolating planner throughput.
- Independent review is complete with no remaining high or medium findings. Both crop-yield deadline findings were fixed and regression-tested; stale documentation was corrected.
- Workspace build passes. The implementation step and its validation are complete.
