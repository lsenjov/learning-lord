# 05 — Quantity-aware supply reference

## Hypothesis and scope

Estimate the cost of obtaining a useful quantity rather than just the first unit. This addresses token cheap listings while retaining the replacement-cost interpretation and its weaknesses.

## Proposed algorithm

- Define reviewed positive constants Q in native units: propose one meal/drink for consumables and one item for garments; ingredient constants initially informed by single-recipe requirements (blocks 8 units, cloth 25 grams). Store the constants explicitly; do not derive them live from catalogue or demand changes. They represent small baskets, not daily restocks.
- At each daily closing snapshot, traverse eligible listings cheapest-first until Q units are accumulated. With filled=min(Q, eligible depth), calculate sum(filled_i * effective_quote_i)/filled in existing quote units (coins/kg or coins/item). Exclude payment rounding from the index; actual shopping still rounds payments.
- Record coverage filled/Q in [0,1]. Use the available average for any positive native quantity, including one gram; retain the old reference with no supply. Undercovered cases have no claimed protection against shallow listings; report them separately and reject the policy as insufficient if they drive failure.
- Move 25% toward that index. Keep completed trade prices separate. Use the same timing and eligible-order rules as plan 04.
- Do not optimise Q per agent or infer Q from current prices; that would make the reference a different value for every purchase and create a feedback loop.

## Critique

There is no objectively correct Q. A baker buying daily ingredients and a household buying one meal face different marginal costs. Largest recipe quantity changes when new recipes are added, potentially repricing the entire world without new market activity. It also mixes grams and discrete units, so display/tests must use per-good units correctly.

With only one cheap unit available, the available-average rule falls back to the same misleading cheap reference. Coverage warnings do not fix that. Alternatives include freezing below a minimum coverage or a conservative blend, but each introduces another heuristic and stale-value risk. Unsold expensive import supply still pushes values upward. This index estimates procurement, not sale proceeds.

## Implementation plan if chosen

1. Write down Q for every current good and review the economic consequences before coding. Do not silently bake magic quantities into market update logic.
2. Build a pure supply-depth index returning price, filled units and coverage; share sorting where practical with acquisition estimation without changing account-specific shopping.
3. Integrate daily smoothing and diagnostics; add small-cheap/deep-expensive, insufficient-depth, no-supply, discrete-unit and recipe-catalogue tests. Compare sensitivity at Q/2 and 2Q on both saves.

## Evaluation and recommendation

Sorting costs O(order_count log order_count) unless existing sorted quotes can be reused; no extra plan branches. Moderate implementation and tuning complexity. Prefer only if the simplest cheapest-ask experiment visibly fails because of shallow listings. The user recently preferred the cheapest rule; this remains a documented alternative, not an assumed acceptance of quantity weighting.

## Review resolution: explicit quantity semantics and limited claim

- Catalogue constants are reviewed and stored explicitly, never derived live from changing recipes. Proposed examples are Q(blocks)=8 units and Q(cloth)=25 grams. They do not represent Finn's 28–34 block or 350–400 g cloth bulk purchase. Add a fixture where cheap supply exactly covers Q but the larger restock is expensive; report this limitation rather than scaling Q to Finn's order implicitly.
- Require Q>0. `filled=min(Q, eligible native-unit depth)`, `coverage=filled/Q` in [0,1], and `index=sum(filled_i * effective_quote_i)/filled`, returned in the existing quote unit (coins/kg or coins/item). Native units cancel in the weighted average; do not divide a kg quote by 1000 again. Report total depth separately if needed.
- Follow plan 04's exact eligibility and snapshot policy, including trial and self-account participation, finite units and quotes already including tariff effects. No unlimited caravan supply is invented.
- Any positive native quantity allows an update, including one gram. Undercovered observations explicitly have no claimed protection against shallow-listing distortion. Report covered versus undercovered outcomes separately; if undercoverage drives failures, reject the option as insufficient rather than silently adding a freeze/blend rule.
