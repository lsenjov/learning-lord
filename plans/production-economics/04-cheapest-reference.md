# 04 — Follow the cheapest available ask

## Hypothesis and scope

A reference anchored to actual available supply may avoid stale low inventory values when only more expensive imports exist. This deliberately changes reference semantics toward advertised replacement price, not a guaranteed resale price. The market has no buyer bids proving that Finn could sell stock back at that price.

## Proposed algorithm

- At the existing daily 04:00 update, select the cheapest non-empty, purchasable listing for each good across locations and seller types, including caravans and ordinary/speculative local batches. Use the effective advertised per-unit buyer price; do not multiply it by tiny-purchase coin rounding.
- Update `reference = 0.75 * previous + 0.25 * cheapest_ask`. With no eligible supply, retain the prior reference. Filter inaccessible/nonmarket listings using the existing public-market rules.
- Replace local-sales-driven reference updates in this experiment; continue retaining completed trade prices separately for charts and seller learning.
- Sample a frozen closing order book before local repricing and before new caravan deliveries. Existing imported stock participates. This gives a deterministic one-day lag for newly repriced/newly delivered supply and avoids an order-dependent loop.
- Preserve seller markdowns, caravan thresholds, supported tariffs, exports and transaction rules. Explicitly inspect every reference consumer because this policy changes wealth, input opportunity costs, output estimates, trial baselines, reserves and affordability estimates together.

## Critique

The cheapest quote is the price of acquiring the first unit, not meaningful inventory depth. One cheap block can undervalue forty blocks. Conversely, when that one listing sells, everyone's inventory can revalue upward despite no change in productive capacity.

Import-only unsold stock makes the reference converge upward to the fixed caravan ask. Local unsold-stock markdowns can pull it down, but quantity alone does not. A seller could influence town-wide wealth with a token quote; ordinary batch price feedback can also oscillate. Smoothing slows these effects without correcting the underlying evidence weakness. An ask need not have ever attracted a buyer.

Taxes are especially awkward: buyer replacement cost and seller receipts diverge. Import tariffs can raise buyer replacement cost without raising local seller receipts. Negative subsidies are not implemented in this checkout and are outside this experiment. It is a policy experiment, not an accounting truth.

## Implementation plan if chosen

1. Extract and test a pure eligible-cheapest-ask calculation with empty, trial, unavailable tariff and no-supply fixtures. Enumerate reference-dependent callers.
2. Wire the 25% daily adjustment at the documented snapshot point; preserve local transaction history and label the chart reference as supply-based.
3. Test day-boundary ordering, disappearance of cheapest stock, import-only stability, supported tariff changes and self-listing feedback. Run the shared multi-day protocol and review.

## Evaluation and recommendation

Very low branching cost: one pass/minimum per good, ideally one pass over all orders. Behavioural blast radius is large. Test separately from procurement changes. It may reduce Finn's wealth penalty after several days; it cannot guarantee production or make all resale opportunities real. Best valuation experiment if the user prioritises simplicity over a market-depth model; not the preferred first causal test of bulk restocking.

## Review resolution: exact offer eligibility and wider effects

- Eligible means positive units, an existing non-home place, and an offer purchasable under current caravan-good and tariff rules. Include trial offers deliberately: the simplest cheapest-advertised-price policy treats them as public offers too. Test the feedback from a rejected/unsold trial; changing to exclude them would be a different policy.
- This is a global index with no buyer identity. Own-account and household listings participate because other accounts can buy them. Explicitly accept that an owner can influence the valuation of their own stock; do not describe it as an executable quote for every individual.
- Use stored effective quotes once, with no additional tariff uplift. Current market update lacks the full place filter, so supply a read-only eligible book from Universe/map rather than assuming `purchasable` implements routing eligibility.
- Audit asset-tax obligations and dietary/drink/clothing valuation in addition to production and wealth. Add a Sunday tax-boundary fixture, consumption-choice comparison, and established-versus-new seller baseline checks. Raising block reference can worsen garment margins even while reducing purchase wealth loss.
- Snapshot after actions completing at 04:00 have settled, before trial/local repricing and caravan processing. Test a listing sold or created at that instant. No persistent book snapshot is needed: one per-good minimum array suffices.
