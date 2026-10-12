# Batch pricing and locked canaries

## Status

- Implementation authorized after the agreed work clarification.
- Simulation and UI implementation complete; independent reviews and workspace verification passed after fixes.
- Supersedes the ordinary/trial pricing lifecycle in `individual-seller-pricing.md`.
- Preserve the unrelated user edit to `balanced-diet-fishing-cabbage.md`.

## Agreed behaviour

- Each economic seller has a pricing baseline per good and selling location, seeded from the reference value. Keep the persistent account adjustment rate of 1–10%.
- Each new ordinary saleable batch lists at baseline multiplied by one plus the adjustment rate. Listing or producing stock does not itself raise the baseline.
- At daily review, unsold ordinary listings move halfway towards the baseline, only downwards. Repricing and restocking do not reset existing stock's observation age.
- Raise the baseline only after half a batch sells locally at the tested price. Tiny purchases and exports cannot establish higher prices. Preserve historical sale evidence separately from the adjustable baseline.
- A listing may contain several production batches. Its acceptance threshold uses a recipe batch quantity, capped by the listed amount, rather than half of all accumulated surplus. Identically priced stock need not retain physical acquisition lots.
- Retain the existing 48-hour stock-movement comparison for lowering the baseline. Exports count as movement. Sustained accumulation must be able to lower the baseline itself, not merely remove listing premiums.
- For currently unprofitable commercial production, allow one speculative canary per economic account and good within Production. Its asking price is estimated break-even plus the account adjustment rate, locked against ordinary markdowns.
- When half the canary sells locally profitably, establish that selling price and permit ordinary production subject to current costs and stock limits. Unsold canaries prevent manufacturing replacement experiments indefinitely.
- Confirm canary acceptance during the daily review after sale taxes have settled, using local net receipts. Export receipts cannot make an otherwise loss-making local sale look profitable.
- Protect personal supplies and production inputs. Ingredient production and production for personal needs are independent of commercial experiment restrictions.
- Preserve current shopping routes, cheapest-first purchases at each location, quote ceilings, payment rounding, caravan policy and reference-based inventory valuation.
- Show locked speculative listings as **CANARY**, distinct from ordinary marked-up batches.

## Reproduced failure

- Save: `saves/universe-1791815770.736482101.ron.gz`, format 7.
- Replaying in one-minute steps fails at simulation time 1,336,800,000 ms when Dara finishes a meal and adopts a production plan.
- The plan collects/gathers berries and obtains other ingredients before baking a pie.
- Plan adoption claims the first production action, Forage, as a sale trial. The household already has Bram's berry trial, and further commercial foraging is capped. The claim rejects the otherwise valid ingredient-gathering prerequisite with `MissingInputs`.
- Correct the distinction between commercial output and prerequisites; do not merely suppress the error.

## Implementation steps

1. Implement ordinary batch pricing, seller baselines, locked canaries and production eligibility, including the ingredient-production regression. Add targeted tests and any required strict save-version change. Independently review, fix valid findings, verify and commit the simulation changes.
2. Update listing labels and documentation. Review the UI changes, run the relevant desktop tests and workspace checks, then commit.

## Verification

- Ordinary batches: no manufacturing-driven compounding; half-batch acceptance at a single quote; daily downward-only adjustment; baseline decline with slow stock; restock age; local/export distinction; save round trip.
- Canaries: fixed price, shared-account uniqueness, half-batch net profitability, changed input costs, delayed output, cancellation and reserved supplies.
- Reproduce Dara's ingredient plan in a focused regression and replay the available snapshot where the save schema permits a diagnostic import. No production save migration is in scope.
- Formatting, strict Clippy, build and appropriate simulation/desktop tests.
- Fresh independent review after valid high/medium findings are resolved.

## Verification results

- The ingredient-production regression retains another household member's berry canary while Dara plans berry gathering for pies. It passes without claiming that prerequisite as a sale experiment.
- A temporary diagnostic copy of the pre-error version-7 snapshot passed the original failure boundary and advanced another twelve simulation hours. The diagnostic initialized version-8 listing metadata, cleared obsolete ordinary-trial state and supplied empty intent markers for old plans; newly calculated plans use the actual commercial markers. Original save files were not modified, and this is not a save migration feature.
- Independent review found an ineligible stale commercial claim returning success after a failed canary. The claim now rejects with the normal replanning path; both failed-result and competing-worker regressions pass. Fresh review found no remaining high or medium issues.
- Existing fixed-price integration fixtures now use actual seller quotes or explicit controlled unit-test quotes, preserving their rounding, tax, affordability and stock assertions.
- Commercial profitability now includes selling-location income tax for both local and export output. A regression verifies that an established selling price does not authorize ordinary production once input costs exceed after-tax proceeds. A separate fresh review of this change and the integration fixtures found no issues.
- A randomized ten-day run exposed another `MissingInputs` failure. The instrumented rerun completed successfully, but revealed capped commercial candidates accepted by prediction and rejected during execution. Commercial prediction now rejects unavailable profit estimates, while ingredient production remains available. Claim/action startup is also transactional, so a failed start cannot leave an orphaned canary. Both regressions pass and a fresh independent execution review found no issues; temporary diagnostics were removed.
- A subsequent town failure was captured and reproduced deterministically: adoption claimed a canary before updating production reserves for the selected workplace. Thread needed for cloth was temporarily treated as excess, creating a ready-to-sell canary that blocked the first thread-production action. Reserve setup now matches prediction before claiming canaries. Future commercial claims are deferred when their prerequisites must first make them eligible; actual production still performs the strict transactional claim. Both cases have passing regressions, including a competing worker taking the deferred claim. Fresh independent review found no high or medium issues.
- Follow-up review found that newly appearing household surplus could bypass canary eligibility during action startup. Commercial startup now uses the same eligibility conditions as prediction before claiming anything; a regression verifies that rejection preserves inventory and market state and permits replanning. A fresh independent review found no remaining findings.
- The full workspace run passed 639 tests, including a fresh ten-day town; five manual benchmarks were ignored. After the final startup guard, all other workspace tests passed again (639 including the new regression), and the captured failing town was replayed separately with the final code. No tests were weakened to suppress the runtime failure.
- Formatting, strict workspace Clippy and the workspace build pass. Save format is version 8; older saves remain unsupported and untouched.
