# Cheapest-first purchasing

## Agreed policy

- Use available owned supplies, including household listings, before purchasing replacements.
- For each good, buy from the lowest available asking price first, taking the needed quantity or available stock, then proceeding to the next cheapest supply.
- Break equal-price ties by distance. Use stable identifiers only for remaining ties.
- Include travel and transaction time in the plan's wellbeing score.
- Apply the same source-selection policy to production, food, drink, clothing, reserves and purchasing cost estimates. Do not buy a more expensive good merely because its seller is at a stop needed for another good.
- Preserve ownership, rounding, spending limits, quoted-price protections, availability limits and existing replanning behaviour.

## Evidence

- Save `universe-1792074789.170434254.ron.gz` has 12 kg of wheat listed at Ada's field around 45.20 coins/kg, 10.8 kg at Iris's field around 46.12, and 2.482 kg at the caravan market for 97.50.
- Bram's production forecast buys the caravan stock first and fills the remainder at Ada's field, costing about 293 coins for 3.6 kg instead of about 163 coins entirely from Ada.
- Shopping alternatives are generated in location-ID order; workplace restocking accepts the first feasible result. That expensive input purchase makes a profitable milling opportunity score below sleeping.

## Work unit

1. Implement consistent cheapest-first acquisition and estimates, update documentation, add regressions, independently review, resolve findings, verify against the reported save, and commit.

## Verification

- Farther cheap supply wins over nearby expensive supply; equal prices prefer nearer sources.
- Exhaust cheap stock before moving to the next quote, including multiple quotes at the same location.
- Multi-good baskets respect the policy independently for each good; owned goods remain first.
- Estimates and predicted purchases agree on source selection and rounded cost.
- Existing purchase ceilings, partial affordability, travel limits and saved-state behaviour remain covered.
- Relevant simulation and desktop tests, formatting, strict Clippy and build; inspect Bram's forecast from the exact saved universe without changing the original file.

## Status

- Implemented shared cheapest-first source selection for shopping, cost estimates and production forecasts; removed starting-seller permutations.
- Independent Sol review found no actionable correctness or regression issues.
- Workspace tests: 648 passed, 7 ignored manual tests; locked workspace build, strict Clippy and formatting checks passed.
- Exact-save replay: Bram's next wheat restock chooses 3.6 kg from Ada for 163 coins, then mills flour. Over the next simulated day he produces 3,360 g of flour. Replay and save round-trip validation passed without changing the source save.
- Preserve the unrelated user edit to `balanced-diet-fishing-cabbage.md`.
