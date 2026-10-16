# Independent critique: 09 — Make or buy

**Verdict: defer; revise before implementation.** This is a plausible bounded heuristic for recognising chains, but the current proposal leaves route feasibility and its interface with existing ranking ambiguous. It correctly says that it cannot establish a cure for Finn's inactivity. Reviewed against `evidence.md` and the production, inventory/site and planning code; no builds or counterfactual replays were run.

## Prioritised findings

### High — Single-output is insufficient to define executable dependencies

`Recipe::outputs()` is single-output today, but `GrowFlax` and `BrewBeer` are delayed processes. `recipe_requirements_at` explicitly excludes their future yield from supplies available during a work block. Recursive expansion based only on skill and an accessible facility would incorrectly permit future flax or beer to satisfy a current consumer. `production_stock` includes expected output for stock targeting, which is not a source of presently usable inventory.

Revise the initial supported set to immediate recipes at one explicitly chosen physical site. Reject delayed recipes as suppliers (buy their output instead), unless a separate time-aware scheduler is deliberately added later. Preserve farming care and beer collection priorities in `production_at_site`; a profitable chain must not displace time-sensitive care merely because it owns a route. Never credit future output both as available input and as terminal stock/revenue.

### High — A route quote can be affordable on paper yet impossible to execute

“Finite listings and inventory” does not specify a shared cash budget, intermediate net yield, or physical access. `usable_owned_units` combines carried, listed and stored supplies; gathering these needs actual withdrawal/collection actions. `production_place` chooses a deterministic facility, while `production_sites` exposes multiple physical sites. A facility kind alone is inadequate: fees, socage, travel and accessible storage depend on `PlaceId`.

Use one temporary ledger per route: cash, inventory by source, listing quantities, position, elapsed time and net production after cumulative tax rounding. Debit each owned unit once across branches; reduce purchases after owned allocation; reserve money for every purchase/fee before proceeding without funding it from speculative final sales. Aggregate per-seller purchase rounding according to executable actions. Keep account ownership and incoming workplace deliveries consistent with existing procurement. Reuse actual preparation/execution semantics rather than inventing a second approximate feasibility model. State explicitly how unaffordable routes fail or shrink; budget-driven shrinking is an extra search dimension unless bounded.

A concrete regression is Finn holding one block: seven imports can support a positive garment estimate in the day-14 evidence, whereas purchasing eight loses money. Owned allocation must precede both route cost and shopping construction. Likewise, making eight *gross* blocks is insufficient if socage leaves fewer than eight usable blocks; batch counts must use conservative net yields.

### Medium — Route choice could silently replace the output-stock policy

The proposal says profit/time first and stock preferences second. Existing `ranked_recipes_at` first compares `stock_fraction / profit_rate`, then rate and recipe identity. It also preserves trial eligibility and export thresholds. A new global profit-rate ordering changes more than make-or-buy procurement and confounds the experiment.

Choose A versus B for the same final output and quantity, then feed that route's cost/time into an explicitly preserved output-ranking rule. Specify whether downstream profit makes an otherwise rejected garment eligible: without this integration, better quotes may never produce candidates. Retain blocks as a separately eligible final product and avoid counting the same produced block once as standalone sale revenue and again inside garments. State the terminal surplus valuation numerically; crediting surplus at reference while final output uses estimated seller prices can otherwise change preferences through rounding. Only final unconsumed stock can receive terminal value, with initial consumed stock charged once.

### Medium — Two route labels are not a construction bound

The proposal acknowledges this concern but does not resolve it. Memoising a recipe/requirement expansion by recipe alone is unsound when owned quantities, remaining listing depth, cash, site and tax carry differ. Memoising complete ledger states can multiply the state space. A depth cap detects neither cyclic dependencies reliably nor large rounded batch expansion. The current catalogue has one producer per good, so a profitability-based supplier chooser is unnecessary initial complexity.

Precompute the unique supplier relation and reject cycles using an active recursion stack or graph validation. For the initial experiment, fix site, final quantity and supplier mapping; build precisely two routes with independent ledgers. Bound total expanded batches/actions and stop with an explicit unsupported result when exceeded. Aggregate repeated requirements before expanding where semantics permit. Cache immutable recipe topology, not state-dependent quote results. Document deterministic tie-breaking for equal market prices and any later multiple suppliers. Measure quote work and allocations, not only the final candidate count: two candidates can conceal thousands of recipe steps or cloned predictions.

### Medium — A chain quote does not guarantee a useful executed prefix

`production_at_site` reranks after restocking, and the preparation-inclusive production horizon is two hours with at least one productive action. If that existing reranking is left intact, purchased chain inputs can lead to a different recipe than the quote. If it is replaced, that is an execution-policy change requiring an explicit design. A whole-chain ratio also does not make the early block-only prefix beat sleep under `WeightedWellbeing`.

Specify that the bounded prefix follows the quoted order within the candidate, while the next real decision remains free to replan. Compare route endpoint estimates with actual prefix wealth/score and record abandonment across repeated decisions. Do not import future garment profit into current block wellbeing or guarantee eventual assembly. A diagnostic-only quote is a useful first step precisely because it separates recognition from this execution/scoring problem.

## Expected effect on Finn

Both clothing recipes use Tailoring at the Tailory. Route B can therefore quote cloth → blocks → garment without requiring Finn to weave cloth. With no owned blocks it must make eight usable blocks per garment; at base durations that is four hours of block work plus half an hour assembly, before procurement, travel and taxes. Practice changes those times, and an owned block changes the batch requirement. A two-hour candidate commonly finishes blocks without finishing the garment. This can improve chain recognition while Finn still chooses sleep.

The day-14 owned-block example means A must sometimes remain competitive; imported blocks are not categorically unprofitable. Day-16's 25 caravan blocks and rejected purchase of 22 establish neither profitable chain execution nor sales demand. Neither save supports a claimed production increase from this unimplemented option. Require measured output, imports, sales and sleep, not just a positive quote.

## Smallest useful revision

Keep the recommendation to defer behind the narrow procurement experiment. If chain recognition is later demonstrated necessary, first add diagnostic quotes for the immediate same-Tailory block/garment chain, with fixed quantity and unique suppliers. Compare A and B using the same initial inventory, finite quotes and net taxes, leaving selection untouched. This avoids general recipe selection, multi-site scheduling and speculative caching until evidence requires them.

Before enabling selection, add cases for one owned block, zero cash, one cheap listing followed by expensive depth, per-seller rounding, tax-induced extra batches, partial cloth ownership, surplus consumed by a sibling dependency, cyclic/unsupported suppliers, remote listed/storage goods, and care priority. Follow `evidence.md` for both saves and same-state cold/warm median/p95 measurements. A fixed maximum quote-work budget and explicit rejection diagnostics should accompany the existing provisional 2× investigation trigger.

## Fresh review of the revised plan

**Verdict: the high and medium design findings above are resolved for the proposed diagnostic research gate.** The revision narrows suppliers to immediate recipes at one physical site, uses unique producer topology with cycle rejection, and adds independent execution-backed ledgers plus explicit batch/action caps. It preserves between-output stock/rate ranking, charges initial goods once, credits only unconsumed surplus, and states the necessary prefix-order change without inserting future profit into present wellbeing. The diagnostic-first gate is appropriate; this is not approval to implement or a finding that Finn will become active.

No remaining high or medium finding blocks that diagnostic stage. Execution-backed preparation must actually respect the primitive-action cap during construction, rather than counting actions after unbounded preparation; this is an implementation acceptance condition. Likewise, integration still requires the already-requested partial-owned, budget, tax/rounding, source-location and interrupted-prefix cases and both-save performance evidence. These are verification requirements for a future change, rather than unresolved reasons to broaden the design now.

One low documentation issue remains: the earlier critique still discusses supplier selection and a greedy supplier choice, although those are excluded from the initial scope. Label them explicitly as limitations of a possible later extension. The implementation list's step 2 should also repeat that selection integration is gated after diagnostic evidence, matching the final resolution. The final resolution otherwise settles the substantive ambiguities.
