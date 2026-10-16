# Production economics: alternatives and independent critiques

Design analysis, 2026-10-17, against `b071e3c`. These are alternative plans, not ten changes to implement together. No simulation behaviour has been changed. The user-reverted acquisition-cost patch remains reverted.

Each plan has a separate independent reviewer using `gpt-6.1-sol`. Substantive findings were incorporated and the revised scope reviewed again. Some verdicts approve only a diagnostic investigation: they do not settle the design of a future scoring system. The review files retain the original findings followed by the fresh verdict, so an earlier request for changes is not the final status.

## What is actually going wrong?

Finn's situation combines several different mechanisms. Treating all of them as one bad reference price obscures which change would help.

1. **Buying uses available asking prices and real payments.** Imported blocks cost about 18.75 each in the investigated saves; reference value is about 12.50. Integer payment rounding also matters.
2. **Owned ingredients use reference value in recipe economics.** Buying can change an ingredient from an expensive missing input into a cheaper held input in a hypothetical forecast. The reverted acquisition-cost experiment addressed this inconsistency.
3. **A recipe's expected selling revenue and its output's inventory wealth differ.** Finn's garment selling estimate was about 147.41, while an unsold garment counted as about 139.22 of wealth.
4. **Restocking and evaluation cover different amounts of work.** Procurement targets roughly a day's work, while a production candidate normally targets two hours including preparation. A candidate can pay for many future batches and receive credit for only a few completed batches. Goal-pair scoring can extend the evaluated time, but does not guarantee that all purchased inputs get used.
5. **The choice is based on wellbeing, not just recipe profit.** Cash, reference-valued goods, needs, travel and elapsed time affect the whole candidate. A positive recipe label does not establish that its procurement-and-work sequence beats sleeping.

The day-14 example makes the third point concrete. Starting with one block, buying seven more costs 132 coins. Ignoring other fees and needs for this illustration:

- Expected selling margin: `147.414384 − 12.4995 − 132 = +2.914884` coins.
- Wealth change after assembly, with the garment unsold: `139.224696 − 12.4995 − 132 = −5.274804` coins.

Finishing that particular batch does not recover the measured wealth loss. A longer horizon alone is therefore not a reliable cure; a sale or different valuation could matter. Making the blocks from cloth is a different route with different costs, so this arithmetic does not prove that making garments is generally bad.

The reverted acquisition-cost experiment changed the proposed work but still produced zero blocks in its recorded 24-hour replay. That is evidence for separating accounting correctness from the inactivity problem. No new counterfactual simulation was run for these ten proposals. See [evidence and evaluation protocol](evidence.md) for the two distinct saves, exact observations and relevant code.

## The ten alternatives

| Plan | What it changes | Main criticism | Recommendation / readiness |
| --- | --- | --- | --- |
| [01 — Acquisition costs](01-acquisition-costs.md) · [review](review-01.md) | Preserves actual acquisition costs within one forecast instead of immediately repricing purchased inputs as cheap ingredients. | Does not change inventory wealth or fund the future work; the earlier experiment did not restore production. | Bounded correctness experiment. Keep separate from the inactivity intervention; do not automatically restore the reverted patch. |
| [02 — Short versus daily restock](02-restock-sizes.md) · [review](review-02.md) | Compares a production-block basket with the existing daily basket, retaining both through following-goal scoring. | The short basket may still buy blocks; extra trips and rounding can erase savings. More candidates have a measurable cost. | Preferred first causal experiment, beginning with a diagnostic comparison of the actual baskets. |
| [03 — Longer evaluation](03-longer-evaluation.md) · [review](review-03.md) | Evaluates delayed payback over a common twelve-hour interval using a bounded continuation policy. | Needs policy, repeated goals, exact boundaries and incomplete rollouts make this a substantial planner change. Unsold output may still be worth less than inputs cost. | Fixed-sequence diagnostics first; integrated scoring deferred. |
| [04 — Cheapest-ask reference](04-cheapest-reference.md) · [review](review-04.md) | Moves reference 25% daily towards the cheapest eligible listing, including imports. | A tiny offer can revalue all stock. Unsold imports can raise reference. Asking prices do not prove resale proceeds. | Coherent isolated experiment if we want replacement-oriented wealth; broad economic consequences must be intentional. |
| [05 — Supply-depth reference](05-depth-reference.md) · [review](review-05.md) | Uses the average ask needed for an explicit small quantity instead of the first unit alone. | Choosing the quantity adds policy; insufficient depth still lets one cheap unit drive the value. A small basket does not represent a daily restock. | Consider only if shallow listings demonstrably break plan 04. |
| [06 — Include completed import purchases](06-import-sale-reference.md) · [review](review-06.md) | Adds actual citizen purchases from caravans to reference-price evidence. | Cannot move the reference when nobody buys. A purchase shows willingness to pay, not the seller's guaranteed resale value. | Useful separate evidence improvement, insufficient to bootstrap Finn's current no-purchase state. |
| [07 — Useful-input reserve benefit](07-useful-reserve-value.md) · [review](review-07.md) | Gives a capped additional benefit to useful inputs whose replacement cost exceeds reference. | Consuming the supplies removes the bonus: it can encourage shopping followed by sleep. Shared/food inputs require careful allocation. | Offline component diagnostic only; reserve horizon, ceiling and score integration remain open. |
| [08 — Future-product value](08-future-product-value.md) · [review](review-08.md) | Investigates feasible value unlocked by finishing a batch. | Directly calling ingredients finished products grants unpaid labour. A terminal bonus can reward hoarding or postponement and double-count realised work. | Reject the direct shortcut. One-batch accounting diagnostic is defined; any scoring bonus remains deferred. |
| [09 — Make or buy](09-make-or-buy.md) · [review](review-09.md) | Compares buying missing inputs with making those the worker can produce, initially at one site. | Two routes still require finite-resource accounting and bounded construction. A profitable full chain can lose on its executed short prefix. | Useful later if route recognition is the demonstrated obstacle; diagnostic quotes first. |
| [10 — Persistent inventory cost basis](10-inventory-cost-basis.md) · [review](review-10.md) | Tracks historical acquisition/production cost for goods and investigates using it in wealth. | Requires ownership, transfer and work-in-progress accounting. Cost-only value can hide unsold value added by labour and discourage production further. | Defer. A shadow ledger is a research option, not the next Finn fix. |

## What happens to prices if stock does not sell?

Seller asking prices and the market reference are separate mechanisms. Existing local seller markdowns can lower asks when stock is not moving. A cheapest-ask reference would then follow those lower asks gradually. It would not itself instruct sellers to lower prices.

Caravan asks do not follow local seller markdowns. If only fixed-price caravan stock remains, plans 04 and 05 can keep the reference near those asks despite zero sales. Plan 06 instead holds without completed purchases. These are different policies: neither automatically discovers a clearing price.

Reference changes also reach beyond the displayed wealth figure. They affect held-input opportunity cost, asset taxation, consumption choices and reference-dependent initialization of seller/trial prices. Existing seller baselines need not move with reference. Raising the block reference may reduce the apparent loss on buying blocks while also making a block-consuming recipe look less profitable. It is not a one-directional incentive to work.

There are no binding buyer bids guaranteeing that an agent can immediately sell its purchase back at the cheapest ask. Calling that ask a replacement-price estimate is defensible; calling it guaranteed resale value is not.

## Recommended sequence

1. **Capture the two procurement sizes and their full candidates on both saved states.** Record initial and later purchases, cash spent, actual work, wealth changes, needs and following-goal score. Check whether the short and daily baskets really differ. Preserve the distinct day-14 and day-16 universes.
2. **If the comparison exposes useful short work, run plan 02 as the sole behaviour experiment.** Preserve household shared requirements and daily retention, keep both alternatives through the existing goal-pair comparison, and avoid summing speculative demand. Do not also change references, ratios or horizon.
3. **Evaluate actual production and trading, not just recipe labels.** Compare identical-snapshot decision cost and 24-hour/seven-day behaviour. If both candidates still lose, keep that result and identify whether reference loss, delayed work, needs or route choice dominates. Do not silently expand the patch until Finn works.
4. **Choose broader policy only with that evidence.** Plan 04 is a valid alternative if replacement-oriented wealth is the intended model regardless of Finn. Plan 06 improves observed-price evidence but cannot solve zero trading alone. Plans 03 and 09 should follow evidence of delayed payback or missed chains respectively.

The smallest simplification is to stop treating this as a requirement to make every profitable recipe win. First give the existing planner a feasible choice with less upfront purchasing and observe its reasons. If it still chooses sleep, that narrows the problem instead of concealing it with an extra reward.

## Verification and review status

All ten documents have implementation stages, critiques and acceptance criteria. Their substantive review findings are either addressed within the proposed scope or retained as explicit barriers to a deferred integration. The low documentation ambiguities identified in plans 05, 08 and 09 were corrected. This is not a claim that all ten ideas are ready for production implementation.

The shared protocol requires real cash/goods conservation, prediction isolation, shared-account behaviour, finite listing depth, per-seller rounding, taxes, save compatibility where relevant, and measured planning overhead. Current code supports unsigned tariffs, not the negative subsidies discussed earlier in the conversation; the plans do not claim tests of nonexistent subsidy behaviour.

These documents were checked for complete plan/review coverage and valid local links. No code was changed and no new simulation tests or replays were run for this analysis. Any chosen implementation requires its own tests and fresh code review.
