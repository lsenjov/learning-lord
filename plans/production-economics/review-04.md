# Independent review: 04 — Cheapest available ask

Reviewed against the current marketplace, production, trial, wealth, diet and taxation code and `evidence.md`. This is a design review only; no counterfactual replay or build was run.

## Verdict

Request revisions before implementation. The mechanism is small and its acknowledged economic limitations are accurate. Two specification gaps still make materially different implementations possible. This remains a valuation experiment, with weak evidence that it addresses Finn's inactivity.

## Findings

### Medium — Define eligibility precisely, including trial listings and account ownership

The algorithm includes ordinary/speculative batches but the implementation step asks for a “trial” fixture without saying whether trial orders participate. Current `Market::purchasable` only checks caravan eligibility and the 100% import-tariff prohibition (`marketplace.rs:712`); it does not filter homes, missing places, trials or self orders. Route procurement separately requires an existing non-home place (`marketplace.rs:985`). A “public-market rules” helper with all the proposed properties does not currently exist.

Concrete failure: a cheap trial offer can become the town reference before any buyer accepts it, feeding its own experiment into inventory wealth, held-input costs and the baseline of the next trial. A listing at a Home can depress the reference even though route procurement never visits it. A global reference also cannot exclude each owner's own listing: the cheapest ask may be impossible for Finn to buy because Finn's account owns it, while still valuing all of Finn's stock.

Revision: state an exact predicate and explicitly choose trial inclusion or exclusion. Match route procurement's existing-place/non-home rule and nonzero units; apply caravan eligibility/tariff checks. Describe this as a public offer available to other accounts, with owner self-influence intentionally retained. Test household ownership as well as individual ownership. Excluding trial offers is the simpler conservative choice, but it is a design choice, not an existing market rule.

### Medium — Expand the reference-consumer audit to actual obligations and consumption

The plan lists wealth, production and affordability but omits material consumers already present in code. Asset taxation values stored and listed stock using reference (`taxation.rs:925`); diet choice and drink/clothing consumption valuation also use reference (`diet.rs:190,407`; `lib.rs:726–791`). An import-driven appreciation can increase an owner's assessed tax without sales or cash receipts. It can also change consumption choices, and therefore Finn's activity, independently of production economics.

Revision: explicitly include these consumers in the audit and replay reporting. Add an asset-tax boundary fixture and consumption-cost comparison. Distinguish fixed caravan/export reserve policy from any computed reserve or trial cost that really uses reference; do not imply every reserve quantity depends on it. This does not require changing those policies, only making the experiment's effects observable.

## Actual expected effect on Finn

With only fixed-price block imports at 18.75 and a starting reference of 12.50, successive closes produce approximately 14.0625, 15.2344 and 16.1133. The gap halves after about 2.41 daily closes; it never immediately disappears. Buying 28 blocks has a reference gap of approximately 175 coins initially and 131.25 after the first close, before rounding and any other costs. This is an accounting-gap illustration using the day-14 purchase observation, not a predicted candidate score or replay outcome.

The same appreciation increases the opportunity cost of the held block in the garment recipe. The evidence's favorable one-owned-plus-seven-imports calculation therefore deteriorates: the held block rises by 1.5625 after the first close, while imports still cost the same. Existing sellers' established prices do not simply track reference upward, so garment receipts need not offset that rise. Newly created seller states and trial baselines can respond differently. A lower shopping wealth penalty and a weaker garment recipe ranking can happen together; the net effect on the chosen goal is unknown. The day-16 save has no block demand, so new imports are not guaranteed; existing caravan stock is sufficient to test the valuation hypothesis while it remains listed.

## Ordering, feedback and policy checks

The proposed snapshot fits the current daily path: actions and trades finish, `refresh_market` runs, then `market.update` runs, followed by caravan exports/imports (`lib.rs:2597`). Capture asks before `update_production_trials` and ordinary repricing inside `update`, as well as before caravan processing. A listing sold at exactly 04:00 is absent; a listing created by a finishing action may be present. State and test those cases. Calling this a closing book is reasonable, provided it means the book at this exact phase rather than the previous tick.

The minimum can sample a local offer about to be exported immediately afterwards. That is consistent with the chosen snapshot, but the offer may not exist during the next day. Local markdowns remain a downward feedback path; a frozen snapshot removes same-call order dependence, not day-to-day self-quote feedback. Existing import asks are fixed by goods and tariffs, so import-only unsold stock converges upward regardless of demand. The plan correctly acknowledges this.

Use the stored quote in price units; do not gross up an already tariff-adjusted caravan quote again. At a 100% import tariff, exclude retained caravan orders. The current policy uses unsigned tariff basis points with a 0–100% validation range (`marketplace.rs:164–184`), so negative import subsidies and “subsidy exhaustion” are not supported simulation scenarios. The desktop's negative-subsidy table fixture is not a simulation policy. Replace that acceptance case with supported tariff changes, or explicitly label subsidies as future policy semantics outside this experiment.

## Simpler option and performance

The simplest implementation is one temporary `[Option<f64>; Good::COUNT]` array filled by one pass over eligible orders at the existing close point. No persisted order-book snapshot, per-buyer reference, sorting, or depth model is required. Retain local price/trade history separately as proposed. A small read-only diagnostic showing eligible minimum and current reference on both saved states would expose whether the desired direction exists before committing to a replay experiment.

The extra calculation is O(number of orders), with constant memory per good, once daily. Avoid using route planning or rounded purchase simulations to discover a minimum: those add buyer-specific travel, budgets and allocations to a global quote statistic. Existing daily seller repricing has repeated scans; do not attribute that existing cost to this experiment. The shared cold/warm and same-state timing protocol is adequate.

After the eligibility and consumer-audit revisions, this is suitable for an isolated valuation experiment. It should not be described as evidence that bulk restocking or productive inactivity is fixed.

## Fresh review of revised plan

Both medium findings are resolved. Eligibility now explicitly includes trials and self/household offers, requires an existing non-home place, and applies supported caravan/tariff restrictions. Including unsold trials is an acknowledged policy choice with a feedback test, rather than an accidental eligibility leak. The wider audit now includes asset tax, consumption and established-versus-new seller baselines. Unsupported subsidy scenarios have been removed and exact 04:00 inclusion is specified.

No remaining high or medium findings. Approve the revised document as a design for an isolated experiment, subject to its future implementation tests and review. This approval does not establish a beneficial outcome for Finn or authorize implementation.
