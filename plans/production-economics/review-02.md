# Independent review — 02 restock sizes

Reviewed the proposal, `evidence.md`, current production/restock/household/planning code, and the code-review skill's architecture guide. Design review only; no implementation, builds, or counterfactual replays.

Verdict: revise before implementation. The procurement-size experiment is reasonably narrow, and retaining both alternatives through following-goal scoring is essential. No high findings. Two medium gaps need explicit design decisions.

## Medium — define the shared short target and threshold interaction

The plan says to derive a short requirement at each workplace, preserve covered-work thresholds, and avoid each household member buying a communal target independently. Those are not yet a complete rule. `refresh_workplace_supplies` sums each worker's two-hour and daily projections by `(economic_id, place)` and copies both sums to all account members (`households.rs:395`). The current cached restock branch tests communal stored stock against the summed short threshold and purchases the summed daily target, less incoming deliveries (`goals.rs:1771`). Retention elsewhere uses that daily target (`households.rs:385`).

Concrete failure: two workers share a workplace. Stored ingredients cover one worker's next block but fall below the communal threshold. If the short alternative uses an individual target with the existing communal trigger, it may repeatedly enter restocking without closing the communal shortage. If it instead treats the summed short requirement as an individual deficit or ignores incoming deliveries, sequential members can purchase the same communal shortage. Changing the cache to the selected short target would also release daily input reserves for collection/sale, changing a second policy variable.

Required revision: choose the shared short basket explicitly, state how stored/carried/listed/incoming units reduce it, and state whether the skip threshold remains communal. The simplest compatible interpretation is to reuse the cached communal short target for cached workplaces, preserve the cached daily target for retention, and retain the existing actor-specific projection for uncached sites. Validate two members planning in sequence after the first purchase/deposit or pending delivery, partial communal coverage, and one member carrying inputs. Do not introduce a new reservation system unless these existing rules prove insufficient.

## Medium — existing diagnostics cannot show both fair comparisons

`planning.rs:755–813` already evaluates every production prefix through `search_with_goals` before choosing a winner, so returning both variants there can preserve the intended comparison. But each workplace has only one `location.forecast`, overwritten for every viable variant. With short then daily alternatives, the displayed workplace forecast can show the daily loser even when short wins; recipe diagnostics likewise aggregate across variants. The plan requires both candidates to be visible, but its implementation steps do not address this information loss.

Required revision: include narrowly scoped candidate diagnostics or a deterministic diagnostic export carrying restock size, purchases, first-goal score, and continuation score for both variants. Preserve all viable alternatives until continuation scoring; do not locally choose between them in `production_at_site`, or use the existing single workplace forecast as evidence that both were fairly compared. A focused test should make the locally weaker candidate win after a following goal.

## Expected Finn impact and limits

A two-hour basket can lower upfront cash outlay and the reference-valued loss from imported blocks, allowing useful work to beat sleep. It can also still contain imported blocks and garment assembly: `projected_recipes_at` ranks a recipe mix, and `recipe_requirements_at` accounts for intermediates rather than selecting a cloth-only route. The plan correctly acknowledges this. The first evidence gate should record both short and daily projected orders, net baskets, and the full reranked production candidate for each named save. If the baskets match or both candidates still import blocks and lose, that is a valid negative result, not grounds to silently add recipe search or change valuation.

The two-hour projection measures recipe work; actual candidate time includes shopping/preparation, and a first finished job is allowed beyond the bound. Quantities are therefore a target, not a guarantee of two hours of post-shopping work. Following preparation can also acquire inputs for reranked recipes through `production_order_bounded`/`prepare_inputs`. Inspect the complete action trace for subsequent purchases, not just the initial basket, before attributing a result to acquisition size.

## Demand, deduplication, and performance

Current blocked requests are combined by per-good maximum (`goals.rs:1088`), not addition, across prediction branches. Preserve that rule and shared account attribution; avoid summing requests merely because there are two alternatives. An unavailable daily basket may still record more blocked demand than a feasible short basket. That is existing speculative-demand behavior and should be reported separately from actual selected purchases, rather than described as necessarily caused by duplicated household buying.

Deduplicate when the supplied predictions are equivalent, not merely because both alternatives issue no Buy action: owned collection, withdrawal, deposit, position, elapsed time, cooldowns, and ownership can still differ. Deduplicating identical requested baskets before simulation is safe only when their supply handling and subsequent policy are identical. A no-purchase branch should remain available once when truly identical.

Returning at most two initial states per workplace bounds initial breadth; it does not guarantee twice the total cost. Each state reranks/extends production and traverses following goals. Measure cold/warm same-snapshot candidate counts and median/p95 time, separately from replay workloads. Reuse existing helpers with an explicit target and a small alternative list; no generic procurement-policy framework is justified here.

After the two medium revisions, this is suitable as a first causal experiment, conditional on the pre-change basket capture. Implementation remains unauthorized by these documents and requires independent code review and the shared acceptance protocol.

## Fresh review after revision

Reviewed the added “Review resolution: shared targets and observability” against the same code paths. Both medium findings are resolved at the design level:

- Cached workplaces explicitly reuse the summed communal short target and existing communal stored-stock skip threshold. Uncached sites use actor-specific projections. Daily retention stays unchanged, and owned/incoming supplies must count once across sequential household decisions.
- Per-alternative diagnostics/export now include the full action trace, spending, first-goal score, and following-goal score. Both alternatives survive continuation evaluation, with an explicit regression scenario for a first-goal loser winning the full comparison.

The revision also addresses the supplied-state deduplication and maximum-demand concerns without adding a new reservation framework. No remaining high or medium design findings.

Verdict: suitable for the proposed evidence-first prototype if separately authorized. This is design approval, not evidence that shorter purchasing fixes Finn. Remaining prototype unknowns are whether the two baskets differ on either named save, whether later preparation purchases erase the savings, whether sequential shared-account supply behaves as specified, whether short beats sleep after full scoring, and measured cold/warm planning cost. These are explicit evaluation gates rather than unresolved design requirements. No code changes, builds, or replays were performed for this fresh review.
