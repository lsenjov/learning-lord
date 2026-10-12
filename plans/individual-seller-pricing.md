# Individual seller pricing and production trials

## Status

- The ordinary/trial pricing lifecycle below is superseded by `batch-pricing-and-canaries.md`; retain this document as the preceding design record.
- Simulation and desktop implementation complete; independent pricing, trial lifecycle and UI reviews complete with findings resolved. Verification is complete.
- Businesses and employment remain deferred in `businesses-and-employment.md`.
- Cost accounting clarified: freeze an estimated batch cost, not the asking price. Trial asks still follow the halfway markdown rule.
- Production integration clarified: when Production chooses a good with no active trial, its first eligible batch is a trial. There is no separate trial goal or special needs threshold.
- Preserve the unrelated user edit to `balanced-diet-fishing-cabbage.md`.

## Agreed market behaviour

- Keep individual sell orders with owner, good, quantity, location and asking price. Do not add standing buy orders.
- Remove the daily overwrite of all local asking prices by a central price.
- Assign each economic account a stable uniformly random whole-number adjustment rate from 1% through 10%, persisted in saves. Household workers share their account's pricing policy.
- Maintain seller pricing history per account, good and location. Adding stock must not reset it.
- Ordinary daily markdown: `rate * max(0, 1 - sales_in_previous_48_hours / remaining_listed_stock)`. Skip empty stock and give a new seller a full observation period.
- Count exports as stock movement for markdowns, but use local sales to support upward price trials.

## Price trials

- Every producible good can have one eligible trial per economic account. Recorded unmet demand and low stock are not prerequisites: the limited batch tests willingness to pay.
- Use existing output before manufacturing a new trial batch.
- Initial trial price is `established_selling_price * (1 + rate)`; use the reference if the seller has no established price.
- New speculative production uses `max(established_price, minimum_viable_production_price) * (1 + rate)`.
- Test a fixed quantity; additions must not enlarge a trial or reset its observation period.
- After 48 hours, local sales of at least half the trial quantity establish acceptance of that tested price. A partial sale alone does not immediately increase it.
- If movement is insufficient, move the remaining trial price halfway towards the current known sale/reference price, provided that lowers the ask, then observe again.
- The halfway rule replaces ordinary percentage markdowns for trial stock.
- Keep price acceptance distinct from profitable replenishment: selling below viability does not justify making more.

## Standing production experiment

- Maintain at most one active trial per producible good and economic account, using existing output where possible. Ordinary profitable production and speculative production use this same trial lifecycle.
- When there is no active trial for a good, its first eligible production batch becomes the trial; this does not force every good ahead of normal goal selection.
- Permit an initial speculative batch even when the reference price does not currently make its recipe profitable. Subsequent speculative batches retain the profitability/retry eligibility rule below.
- At most one trial batch may be in preparation, awaiting delayed production, or offered. Shared workers must claim it atomically.
- When the batch clears profitably, allow another trial; an unprofitable result stops replenishment until costs or observed selling conditions improve.
- Only saleable surplus becomes trial stock. Personal consumption reserves and goods needed as production inputs must remain available for those purposes.
- Personal consumption and ordinary ingredient production must remain possible independently of sale experiments.
- A batch lock must survive plan replacement, crop growth and fermentation. Non-sale depletion must resolve the trial rather than leave a permanent lock.
- Freeze estimated batch cost at commitment, using the existing recipe cost model, and compare actual net sale receipts with that estimate. New batches get fresh cost estimates.
- Production remains in competition with needs and other goals through normal wellbeing scoring; do not add a hard needs gate or fictitious wealth bonus.

## Reference values, buyers and caravans

- At 04:00, update reference value using 75% old reference and 25% the preceding day's volume-weighted accepted local listing prices.
- Exclude caravan trades, whole-coin rounding and seller taxes from this quoted-price statistic.
- No local trades means no reference change; preserve configured initial-price overrides.
- Sellers cannot inflate inventory wealth by raising their own asks.
- Compare actual rounded purchase costs and travel, retaining multi-listing purchases and per-seller/per-good rounding.
- Planned purchases carry a quote ceiling and spending budget; execution fills within those bounds and uses existing replan/failure rules when requirements are no longer met.
- Imports use unmet demand affordable at the actual caravan price, replacing the global reference-price gate.
- Exports filter individual orders against tariff-adjusted thresholds while retaining export permissions, minimum town stock and existing allocation rules.
- Demand remains an observation, not a funded order or guarantee of purchase.

## Market UI

- Show accepted local transaction prices on the chart, distinct from the reference estimate. No-trade days carry the last observed price with a dashed indication; before the first trade distinguish the initial reference.
- Preserve traded-volume bodies and demand/supply wicks, including existing export/import volume colours.
- Mark exports below the wick in green and import deliveries above it in red; hover separates deliveries from actual caravan purchases, prices and tariffs.
- Show seller asks, quantities, locations, adjustment rates and trial status in listings. Price-sorted cumulative supply should make the available price range understandable.

## Architecture findings

- Stock is aggregated by owner/place/good, with no acquisition-cost lots. Current recipe calculations use reference opportunity values for owned inputs and quoted purchase costs for missing inputs, plus relevant fees and tax effects. Exact historical batch costing would introduce a much larger inventory/accounting change.
- Estimated profitability is deliberately not historical acquisition-lot accounting.
- Merely adding a trial target cannot guarantee execution. Recipe builders reject reference-unprofitable work, and the planner can prune it or prefer another goal. A second goal in a pair is forecast only and may never execute.
- Trial eligibility must be integrated into Production without a separate goal. Shared-account claims and fixed batch quantities must survive replanning and delayed output.

## Implementation stages

1. Simulation: seller state, independent quotes, reference statistics, quote-aware purchases/caravans, and account-level trial lifecycle with shared production claims and delayed outputs. These share market/action schema and settlement hooks, so land together in a compiling backend commit. Test, independently review the pricing and trial portions, fix findings and commit. Bump the strict save version with the schema.
2. Market chart/listing diagnostics and documentation. Independently review and commit after workspace tests, formatting, strict Clippy and build.

## Implementation limits

- Shopping compares each possible first supplier, then continues through nearby affordable suppliers. It does not exhaustively search every ordering of stops.
- Trial costs are estimates frozen at commitment, not historical inventory-lot costs.
- Save format is version 7; earlier saves remain untouched and are rejected without migration.

## Verification

- Independent reviews covered pricing and reference evidence, quote bounds, caravan supply accounting, shared trial claims, delayed output, tax effects, chart history retention and regression fixture changes.
- Focused regressions pass, including trial persistence, household formation, snapshot isolation and guild debt settlement.
- Formatting, strict workspace Clippy and workspace build pass.
- Workspace test targets pass, including the ten-day town simulation. The full run exposed a rate-dependent asset-tax fixture: independent markdowns enabled exports before assessment. The corrected fixture disables exports for that branch and verifies escrow remains; all 14 tax tests pass on rerun, with a fresh independent review finding no issues.
