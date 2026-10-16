# 06 — Include completed import purchases in reference evidence

## Hypothesis and scope

Current reference updates ignore completed import purchases, so they can miss the prices citizens actually pay. Adding those transactions improves evidence quality without treating an unsold ask as a successful sale. It cannot bootstrap a market in which no one buys.

## Proposed algorithm

- At 04:00 use completed local and citizen-from-caravan purchases for the selected good. Exclude caravan deliveries (unsold arrivals), exports, own-stock transfers and hypothetical planning transactions.
- Compute volume-weighted effective quoted buyer price for those purchases, including supported tariff effects already embodied in the quote. Do not let ceil-to-one-coin micro-purchases dominate a per-unit market index. Retain actual rounded paid prices separately in the UI.
- Update the reference 25% toward that combined observation; no qualifying trades means hold the previous value.
- Keep local-only accepted-price history for local seller learning and existing chart semantics; add an explicitly labelled combined reference observation rather than rewriting local history to pretend imports were local trades.
- Derive import quote value from completed trade records or accumulate it in the period counters. Do not reuse `imported_quoted_value`, which represents arrivals. If new persistent counters/history fields are necessary, handle save-version policy explicitly; do not promise a schema-free implementation.

## Critique

A wealthy buyer's urgent purchase can raise every citizen's inventory wealth. Imports may dominate volume and hold the reference near caravan asks even when locals cannot sell at those prices. Paying an import tariff demonstrates buyer expenditure, not a higher local seller receipt. This policy needs an explicit buyer-value meaning.

Most importantly, Finn's newer save has zero block purchases. This rule alone leaves the block reference unchanged, preserving the same deadlock. A no-trade supply fallback is possible but becomes plan 04/05, with their weaknesses; it should be tested as a separate combination later.

## Implementation plan if chosen

1. Audit trade retention and daily update ordering to determine whether reliable quote aggregation can be derived without new saved fields. Verify fractional rounding and supported tariff semantics against actual settlement.
2. Add pure combined-observation tests and integrate daily smoothing without rewriting local sale acceptance evidence or caravan policy; reference-dependent seller initialisation is allowed to respond.
3. Test import-only completed trades, deliveries without sales, no trades, mixed local/import volume and exports-only days. Review schema needs, replay and measure.

## Evaluation and recommendation

Good correction if reference is intended to describe observed acquisition prices, but not an independent solution to Finn's no-purchase state. Performance can be bounded by the current period; never scan a year's history for every recipe. Prefer factual transaction evidence over asks when meaningful volume exists, but combining them needs a separately specified policy.

## Review resolution: indirect baselines and historical data

Preserve local-only `accepted_local_price`, `last_local_prices`, `retained_local_prices` and trial local-sale records. This isolates evidence, not all seller behaviour: `known_trial_price` falls back to reference for new sellers. Accept and test that indirect effect against an already-established seller; do not introduce a second baseline policy here.

For the smallest experiment, derive the daily import quote value from one reverse scan of current-period trades and persist no new combined-price series. Existing before/after reference history remains available, and the graph's accepted-local-price line stays local. An extra combined observation can be a diagnostic for retained raw-trade periods only (30 days); do not promise reconstructable 365-day combined history. A later full combined chart requires explicitly versioned daily data. Trades use effective `quoted_price` once; deliveries and exports remain excluded. Test 04:00 boundary inclusion and per-seller rounding separately from the quote index.
