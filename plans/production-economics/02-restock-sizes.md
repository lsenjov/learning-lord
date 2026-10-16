# 02 — Compare immediate and daily restocking

## Hypothesis and scope

A profitable short work segment may lose only because it is bundled with a working day's acquisition. Compare two procurement sizes without changing reference valuation, scoring or the planning horizon. Immediate-only restocking is a simpler variant, but forces frequent trips even when bulk purchasing is desirable.

## Proposed algorithm

- At each productive location, derive a short-work requirement and the existing daily target using the same recipe projection and account ownership. The short option targets the production block, not a separate arbitrary meal-sized purchase.
- Generate at most two initial procurement alternatives: short and daily. Deduplicate equal baskets and no-purchase branches before simulating them. Existing covered-work thresholds still skip unnecessary buying.
- Preserve owned/stored/listed supply priority, incoming deliveries, affordability, finite stock, per-seller rounding and travel time. A short purchase must not trigger the cached daily target again inside the same candidate.
- Construct each candidate from its supplied state and score normally. Retain both through the existing following-goal comparison; choosing the shorter goal locally would reintroduce premature pruning.
- Keep household stock requirements shared, but avoid every member purchasing a full communal target independently. Short candidate quantities must be explicit; they cannot overwrite the daily target used for retention/demand globally.

## Critique

A smaller basket does not guarantee a cloth-only block-making plan. The short projection itself can include garment assembly and imported blocks. Restocking also changes ratios and can change subsequent work. Therefore this experiment tests procurement size, not complete route optimality.

Extra trips, transaction rounding, travel and cooldowns can outweigh the smaller initial loss. Both options can still lose to sleep. Carrying surplus stock may be desirable but undervalued by short scoring. Two alternatives are bounded, yet multiplication through goal pairs and multiple workplaces makes 'only twice the work' an unsafe promise.

## Implementation plan if chosen

1. Capture both projected baskets for Finn before code changes. Establish whether the short basket actually differs and whether its full candidate preserves feasible work.
2. Make procurement target explicit in existing helpers; keep cached daily requirements as the default outside the candidate. Add two alternatives, deduplication and following-goal integration without a new recipe search.
3. Add shared-workplace, covered-threshold, unavailable/budget-limited and cooldown tests. Measure full scoring and repeated shopping; review and replay.

## Evaluation and recommendation

This is the preferred first causal experiment: it changes a narrow decision instead of the town's wealth definition. Pass if both viable candidates are visible and fairly compared, with bounded measured overhead and no demand multiplication. Behavioural success would be useful production at acceptable shopping cost, not a hard-coded preference for the short branch. If both remain rejected, record that negative result before expanding scope. A short-only mode can serve as a temporary diagnostic, not the recommended permanent policy.

## Review resolution: shared targets and observability

- For cached workplaces the short target is the existing summed communal short requirement, not one member's requirement; daily target and retention remain unchanged. For uncached sites use that actor's short and daily recipe projections. The skip threshold remains the current communal stored-stock threshold.
- Reduce acquisition deficits by usable account-owned stock at the site, collectable carried/stored/listed inputs, and incoming delivery units counted once. Planning members in sequence must see the first member's deposited or incoming supply. Do not introduce new reservations here.
- Add explicit per-restock diagnostics (or a deterministic diagnostic export initially) recording target kind, full action trace, spending, first-goal score and following-goal score. The current single location forecast is insufficient because it is overwritten across variants. Preserve both variants through existing continuation evaluation; test a first-goal loser becoming the full-plan winner.
- Preserve per-good maximum blocked requests, never sum speculative alternatives. A rejected large basket may still publish more blocked demand under current rules; report that independently from committed purchasing.
- Deduplicate identical procurement targets only when handling and policy are identical; otherwise compare equivalent supplied states, including position, ownership, elapsed time and cooldowns. Inspect later preparation purchases as well as the initial basket when attributing observed savings.
