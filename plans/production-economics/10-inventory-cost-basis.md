# 10 — Persist purchase-cost inventory valuation

## Hypothesis and scope

If buying supplies transfers cash into an equal-value inventory asset, the immediate wealth penalty disappears. This differs from plan 01: historical cost changes persistent inventory wellbeing, not merely profitability within one forecast.

## Proposed research variant

- Keep a quantity/cost pool per economic owner and good across carried, stored and listed goods. Purchases add actual rounded cost; transfers do not change it; sales and consumption remove proportional basis.
- Produced goods capitalise consumed input costs, with an explicit policy for labour and fees. Do not silently assign reference value to outputs while valuing inputs at cost: that creates arbitrary profit recognition at production rather than sale.
- Show acquisition-cost wealth separately from current market value initially. Do not use the historical pool as an ingredient opportunity cost when deciding whether to process stock; actual future alternatives remain relevant.
- Define impairment for obsolete or unsaleable goods, treatment of gifts/taxes/state transfers, starting stock, shared household ownership, crop loss and consumption. Preserve actual coin settlement regardless of accounting policy.

## Critique

This neutralises purchases in book wealth even when the agent pays far too much. Without impairment, losses stay hidden indefinitely. With impairment, the question of reliable current value returns. Excluding unpaid labour means profitable work can show little inventory gain until a sale, making production less attractive in another way. Capitalising labour invents a wage and can reward work no buyer wants.

Ownership moves, partial units, levies and retained history make this a real accounting subsystem with schema and invariant costs. Sunk historical cost must not dictate future recipe decisions: a costly mistake can rationally be processed into a lower-value product to recover something. That separation creates two price systems instead of simplifying the existing one.

## Implementation plan if chosen

1. Agree on the meaning of displayed wealth and recognition of production profit before implementation. Specify treatment of every ownership transfer and source of goods.
2. Implement and validate a parallel accounting ledger without changing decisions; version saves explicitly if schema changes. Reconcile all balances against real inventory movement.
3. Only then experimentally use it in wellbeing, with overpayment, worthless stock, impairment, tax, gifting, household, crop/fermentation and sale tests. Obtain accounting-invariant and behavioural review.

## Evaluation and recommendation

Reject as the next solution to Finn. It solves the visible purchase penalty by changing the objective and can hide bad purchases. High complexity and migration burden compared with the narrow forecast context. Retained here because purchase-cost valuation was discussed; this plan is a design gate and research path, not an implementation-ready accounting specification.

## Review resolution: accounting gate and no premature behaviour claim

- Purchase neutrality can remove production value added: cloth → blocks at material cost preserves the cloth's basis instead of recognising labour's contribution. True no-input production (foraging/water collection, not the actual cloth-consuming block recipe) may create zero-basis output. Test unsold production versus sleeping and actual selling before treating purchase neutrality as progress.
- A future consumer table must separately specify UI, personal/shared wellbeing, recipe opportunity costs, seller output estimates and tax assessment. Initial shadow ledger affects none of them; deciding which score consumes it is a later explicit policy decision. One authoritative economic-account ledger supplies isolated prediction clones; household copies cannot double-count it.
- Within-owner carried/stored/listed/reserved/work-in-progress moves preserve aggregate basis. A sale removes seller basis and creates buyer basis at paid cost. Gifts, levies and household formation require separately chosen transfer rules. Inputs move into work-in-progress at action start; completion, cancellation, spoilage, crop failure and brewing losses remove/reallocate exactly once. No silent reference fallback for difficult assets.
- Use bounded quantity/aggregate-cost pools, not per-transaction history. Deterministic fractional basis removal must empty the cost pool at zero units and reconcile full liquidation within a stated numerical tolerance. Versioning cannot recover missing history: restrict the first shadow-ledger experiment to fresh worlds, or label any future opening basis as synthetic. Existing Finn saves cannot prove historical-cost accuracy.
- These requirements resolve the plan's research scope, not the deferred choice of labour capitalisation or impairment policy. No integrated cost-basis wellbeing is implementation-ready, and this remains rejected as the next Finn intervention.
