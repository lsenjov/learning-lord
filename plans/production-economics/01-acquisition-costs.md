# 01 — Preserve acquisition costs within forecasts

## Hypothesis and scope

Correct the switch from actual buying costs to cheaper reference costs after hypothetical acquisition. This is recipe-profitability consistency, not a change to inventory wellbeing, reference prices or search breadth. The reverted experiment already demonstrated the narrower correction without resolving Finn's inactivity.

## Proposed algorithm

- Start each planning forecast with existing usable goods at reference opportunity value. Missing goods are quoted against executable, cheapest-first supply with actual per-seller rounding.
- Retain quantity and paid cost per good for purchases within that forecast. Original owned stock is consumed first; purchased stock uses a documented pooled cost. Newly produced goods retain the existing reference opportunity value.
- Preserve that context through deposit, collection and own-listing transfers, and consume it through production, needs and sales. Snapshot cloning cannot mutate the real citizen; context is not persisted.
- Reuse the same calculation in recipe ranking, projected requirements and trial eligibility/claim pricing. Do not double-charge actual cash in wellbeing.
- Exclude negative ordinary recipe margins, retaining the explicit limited trial path. Positive mixed owned/imported batches remain eligible.

## Critique

This stops expensive ingredients becoming magically cheap inside a candidate. It does not prevent buying many ingredients justified by a long projected mix, nor make their reference-valued inventory worth the cash spent. At the next real plan, purchases become ordinary owned stock: the discontinuity still exists across replans. That is consistent with opportunity-cost decisions but cannot be described as permanent cost accounting.

Pooled purchase costs smooth expensive and cheap lots. This is simple but can misrank marginal batches if the first few cheap units matter. Transfers, reservations, multi-input recipes and speculative trials are more dangerous than the arithmetic itself. Context consumption must not recycle the same original cheap stock for several batches.

## Implementation plan if chosen

1. Reconstruct the reverted patch against current code; first preserve the failing before/after diagnostic and define cost consumption semantics.
2. Implement transient context and shared costing, with regression tests for costs through storage, mixed inputs, rounding, produced intermediates, trial limits and save isolation.
3. Replay Finn and report remaining bulk-purchase loss explicitly; measure same-state planning overhead and obtain fresh review.

## Evaluation and recommendation

Pass: recipe margins remain consistent through hypothetical purchases, while profitable alternatives remain eligible. Do not require zero imports or claim success merely because block-making appears in a rejected candidate. Existing experimental results support this as a contained correctness fix, but not the primary next experiment for inactivity. Provisional cost: small search overhead, moderate bookkeeping complexity. See `evidence.md` for shared tests.

## Review resolution: forecast lifetime

- Exactly one acquisition context originates at the root prediction. Every sibling candidate clones it; nested projections and extended-order reconstruction inherit isolated copies and never reinterpret purchased units as original stock.
- Shared consumers explicitly include `eligible_sale_replenishment`, `eligible_production_trial`, `claim_production_trial`, recipe input quoting, and predicted commercial-action checks. Real `Plan::claim_commercial_action` still uses current actual ownership opportunity values after the prediction is discarded; this is intentional and must not be advertised as persistent acquisition accounting.
- Pooled paid cost is the chosen approximation. Eligibility refers to that pool, not every possible cheap marginal prefix. Produced intermediates return to reference, so end-to-end chain profitability and cross-replan cost persistence are expressly outside the pass criterion.
- Add sibling isolation, nested restock → rerank → reconstructed-order, and non-reused original-stock regressions. Prefer an explicit prediction costing object where signatures permit; a skipped field on the cloned citizen is acceptable only if tests prove it never escapes into live state. No general accounting subsystem is required.
