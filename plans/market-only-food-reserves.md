# Market-only food reserves

## Agreed behaviour

- Replenish reserves counts currently usable carried food toward the existing 100, 200 and 300 nutrition targets.
- First reclaim the agent's own listed food, then purchase the remaining shortfall from available market listings, including caravans.
- Prefer lower actual coin cost per nutrition when purchasing; account for whole items, rounded payments, stock, affordability and the actual selling location.
- Allow mixtures of foods and partial replenishment when available supplies, money or the preparation window prevent completing the target.
- Travel and transactions contribute to duration-weighted wellbeing as usual. Finish the final transaction if it begins within the existing preparation window.
- Replenish reserves cannot gather, produce, obtain ingredients or sell goods to finance its purchases. Other goals retain their existing acquisition behaviour.
- Stored property goods remain unavailable without retrieval. Existing reserved goods remain unavailable for use, and caravan import restrictions remain effective.

## Implementation

1. [x] Replace recursive resource acquisition for reserves with ordered withdrawal and purchasing. Documentation and six focused regressions added; independent review approved. Workspace validation: 368 tests passed, two manual benchmarks ignored; strict Clippy, formatting and diff checks passed. The existing 24-citizen, one-day spatial benchmark also completed successfully.

## Validation

- Carried food reduces the target; targets already met produce no reserve goal.
- Own listed food is withdrawn before purchases, with travel and actual site stock respected.
- Cheap bread wins over expensive berries for purchasing; fractional quotes, whole loaves/pies, mixtures and rounding are handled correctly.
- Short supply, low budgets and preparation limits preserve useful partial replenishment without gathering or production.
- Hunger and production still acquire ingredients and forage normally; snapshots and authoritative inventory/coin conservation remain intact.
