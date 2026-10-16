# Independent review: 01 — acquisition costs

Reviewed against current `production.rs`, `planning/goals.rs`, `planning.rs` and `production_trials.rs`, the shared evidence, and the architecture review guide. No build or simulation was run; the prior replay is evidence from `evidence.md`, not a new result.

Verdict: request a small design revision before implementation. This is a plausible local correctness experiment, but it is already experimentally disqualified as a sufficient fix for Finn's inactivity. The plan is appropriately candid about that limitation.

## High findings

None. The plan does not claim the previous failed replay succeeded, and it explicitly preserves the distinction between recipe profitability and whole-candidate wellbeing.

## Medium findings

### M1 — Define the context lifetime and its owner across nested forecasts

“Start each planning forecast” is too ambiguous for the existing call structure. `production_at_site` calculates production targets, restocks, reranks from `state.citizen`, and repeatedly reconstructs an extended order from `segment_start`. `projected_recipes_at` independently clones the citizen and carries its own hypothetical inventory. Trial and replenishment functions take only a citizen. Initializing fresh acquisition context at any of these helper boundaries would turn the same hypothetical purchase into reference-valued original stock again. Conversely, sharing mutable context across sibling candidates would make candidate order affect results.

Concrete failure: a restock candidate buys eight expensive blocks. Reranking or replenishment initializes a new context from the resulting citizen, labels all eight blocks “original”, and approves a garment at reference input cost. That recreates the defect inside one decision despite the new ledger.

Revision: explicitly assign one context to each root prediction and clone it with every branch; nested projections derive isolated working copies from that context. Enumerate the actual profitability consumers, including `eligible_sale_replenishment`, `eligible_production_trial`, `claim_production_trial`, and the commercial-action checks in planning. State what real `Plan::claim_commercial_action` does after the transient context has been discarded. Its use of ordinary reference opportunity costs may be intentional, but it cannot silently be described as the same acquisition-aware calculation. Add a regression covering restock → rerank → extended-order reconstruction and sibling branch isolation.

## Low findings and limitations

- The statement that positive mixed owned/imported batches remain eligible needs a precise boundary. Pooled paid cost can reject a profitable cheap prefix when a large expensive restock is already in the pool. The plan recognizes this but should make the acceptance case explicit: owned units at reference plus the selected pooled purchased cost, rather than every profitable executable prefix. If marginal cheap-prefix eligibility is required, pooling is the wrong policy.
- Produced intermediates deliberately return to reference opportunity value. A purchased-input → produced-intermediate → final-output chain therefore does not retain total acquisition cost. Likewise, a real replan resets costs. These are acceptable scope choices for local recipe ranking, but the pass criterion should expressly exclude end-to-end production-chain profitability and persistence across decisions.
- The prior patch was approximately 300 changed lines across simulation and adjacent files. “Small search overhead” is an unmeasured hypothesis; bookkeeping complexity is material relative to a fix known not to make Finn produce. Keep this wording provisional and preserve the failed replay as an expected limitation, not a target to tune away during this experiment.

## Simpler alternative

First isolate a single candidate-local costing object with a small read-only API for recipe input estimates and explicit consumption after successful predicted actions. Keep persistence, inventory wealth, reference prices, restock size and recipe ranking policy unchanged. Do not add acquisition fields to the general serialized citizen model merely to make existing function signatures convenient. A context threaded through prediction and its production/trial quotation helpers is easier to audit than introducing a second general inventory accounting system.

If the actual objective is getting Finn working, the simplest next step is a narrower procurement experiment that changes only the amount bought before productive work, evaluated against the failed acquisition-only replay. This proposal should remain an independent optional correctness fix rather than a prerequisite unless another experiment truly depends on it.

## Performance and validation

Per-good fixed arrays can make normal cost lookup and consumption inexpensive. The expensive risks are cloning context throughout prediction branches, repeating executable supply quotes for every recipe, and adding quoting to trial/replenishment gates already called inside the projection loop. Reuse the current quoted market and existing purchase results; avoid a second market simulation solely to recover paid prices. Measure on identical snapshots before interpreting longer-run workload changes.

The shared protocol is strong. Add explicit assertions that a consumed original unit cannot subsidize two batches; original, purchased and newly produced quantities sum to actual usable stock after each transfer; nested quotes cannot deplete sibling inventory; and the same candidate does not change classification merely because a helper starts a fresh projection. Retain per-seller rounding, account ownership, tax output and trial-limit coverage.

## Recommended revisions

1. Specify root-prediction context lifetime and branch isolation, with the concrete caller list above.
2. Define pooling and produced-intermediate semantics in the pass criterion.
3. Keep the inactivity verdict explicit: the evidence supports only one aspect of the economics defect, while Finn's whole candidate still loses to sleep.

After those clarifications, approve as a bounded correctness experiment if the user chooses that objective. Do not recommend it as the primary inactivity experiment.

## Fresh review after revision

Re-read the current plan and shared evidence after the added “Review resolution: forecast lifetime” section.

**M1 resolved.** The plan now anchors context at the root prediction, specifies isolated inheritance through sibling branches and nested projections, names the trial/replenishment consumers, and explicitly separates real runtime opportunity valuation from transient acquisition-aware prediction. The named regressions directly exercise the previously ambiguous failure path.

The low semantic issues are also resolved: pooling is an intentional approximation, cheap marginal-prefix eligibility is not promised, and produced intermediates and later real replans are explicitly outside end-to-end acquisition accounting. The alternative of a skipped citizen field is acceptable only under the newly stated live-state isolation condition; an explicit prediction object remains preferable.

**Fresh verdict: approve the revised design as a bounded correctness experiment. No remaining high or medium design finding.** There is no remaining design gate requiring a new policy decision for this plan. Implementation still requires the documented isolation/conservation tests, same-state performance measurements, independent implementation review, and the shared replay protocol. These are verification requirements, not evidence that the design already works.

The recommendation is unchanged: do not choose this as the primary Finn inactivity experiment. The known acquisition-only replay still produced zero blocks because the whole candidate lost to sleep; this revision clarifies accounting semantics without supplying evidence of an inactivity fix. No new simulation, build or code change was performed for this fresh review.
