# Independent review — 05 quantity-aware supply reference

Reviewed `05-depth-reference.md`, `evidence.md`, plan 04, and the relevant goods, quote-filling and recipe code. Design review only; no builds or counterfactual simulations. Recommendation: retain as an alternative, but revise the following medium findings before treating it as implementation-ready. There are no high findings.

## Medium — The proposed Q measures one recipe, while the motivating failure involves bulk restocking

`Recipe::AssembleGarment` consumes eight blocks and `MakeClothingBlock` consumes 25 g cloth. The proposed largest-single-recipe rule therefore gives blocks Q = 8 and cloth Q = 25 g, even though the day-14 observations involve 28 or 34 purchased blocks and 350 or 400 g cloth. Eight cheap blocks followed by expensive imports would completely determine the block index at the cheap price while leaving most of Finn's restock expensive. That is a legitimate small-basket reference, but it does not resolve the motivating bulk-depth mismatch. The day-16 caravan stock of 25 blocks covers this Q and still supplies no evidence of buyers or resale proceeds.

**Revision:** explicitly state those example Q values and the quantities they fail to represent. Add a fixture with cheap supply equal to Q and expensive supply needed for the observed restock, alongside the tiny-cheap fixture. Define success as improved reference behavior for the chosen basket; do not imply it establishes profitable Finn production. Keep the cheapest-ask experiment as the simpler first choice unless replay evidence specifically attributes its failure to shallow supply. Increasing Q to Finn's current restock would couple this policy to a second policy variable and should not be slipped into this experiment.

## Medium — The units and coverage result need an executable mathematical definition

The code stores flour/cloth quotes per kg but quantities in grams; blocks and garments use quotes per item. “Quote-weighted cost per unit” does not specify whether the returned value is coins per native unit or per existing price unit. Feeding a coins-per-gram result into `Prices` would divide cloth wealth by 1000. The `available / Q` description is also ambiguous: total eligible depth can exceed Q, while filled depth cannot. Finally, “at least one unit” means one gram for cloth, not one meaningful consumer purchase.

**Revision:** define `filled = min(Q, eligible depth)` and the index in existing quote units as `sum(filled_i * effective_quote_i) / filled`, with Q and filled expressed in the same native quantity units. Require Q > 0. Define coverage as `filled / Q` in [0, 1], and report total depth separately only if useful. State that any positive native quantity permits an update, including one gram, and add per-kg versus per-item numerical fixtures. Store explicit reviewed constants rather than deriving Q during each update from a changing recipe catalogue; document how intentional catalogue revisions alter them.

## Medium — Low coverage remains an acknowledged failure mode without an evaluation decision

The plan correctly admits that one cheap unit still drives the entire index when coverage is low. Its current recommendation nevertheless lacks a rule for interpreting that outcome. With Q = 8 blocks, one cheap block and no other stock produces exactly the cheapest-ask target and a 25% update despite only 12.5% coverage. Raising Q does not change that target. A coverage diagnostic alone cannot substantiate the hypothesis that this option fixes shallow-listing distortions.

**Revision:** retain the simple available-average rule as the explicit experimental policy, and declare low-coverage cases outside its claimed improvement. Require separate reporting of fully covered and undercovered updates on both saves. If undercoverage is frequent or drives the observed failure, reject this policy as insufficient rather than adding a freeze/blend during the same experiment. Freezing is a separate alternative with stale-reference consequences and should be compared only if later authorized.

## Effective prices, finite stock and Finn's outcome

Plan 04 supplies the right buyer-price semantics, but make the dependency explicit in step 2: traverse actual finite eligible listing units at their effective advertised quotes, including existing imported inventory; do not substitute an import threshold for unlimited depth or gross up a quote twice. Reuse eligibility and snapshot semantics from plan 04. Do not reuse account-specific fill logic wholesale: it excludes the buyer's own listings, applies budgets and rounds seller payments, whereas this is a town reference with no single buyer. Add an exact finite-depth/tariff fixture to the planned tests.

The expected Finn effect is uncertain. More expensive supply can raise held-stock wealth and input opportunity costs together, and can change seller baselines; these effects need not improve the work-versus-sleep comparison. Purchased inputs already use actual listings and rounding. No binding buyer bids prove that valued output can be sold. Keep the shared 24-hour/seven-day outcome checks and the distinction between the two saves; neither a higher reference nor a positive recipe margin is acceptance evidence by itself.

## Performance and simplest alternative

This should remain a daily market-level calculation, not a calculation per citizen, recipe or planning branch. Group eligible orders by good once, sort each group once, and stop filling at Q. Its cost is bounded by O(N log N) time and O(N) temporary storage for the closing book; sharing a sorted view is optional and should not require a new cache invalidation system. Q/2 and 2Q sensitivity runs can consume the same frozen book. The cheapest-ask alternative needs a linear scan and no per-good Q catalogue, so its lower maintenance cost is material unless depth-aware behavior demonstrates a useful difference.

## Fresh review after revision

Reviewed the appended “Review resolution: explicit quantity semantics and limited claim” against the original findings. All three medium findings are resolved: the explicit Q examples and bulk-restock fixture bound the claim; the native-unit weighted formula preserves existing quote units and bounds coverage; and undercoverage now has a stated reporting and rejection rule. Finite eligible stock and effective tariff-inclusive quotes are explicit. No high or medium findings remain. No implementation or simulation was performed.

Low documentation issue: the earlier algorithm still says `available / Q` and “quote-weighted cost per unit,” while the resolution supplies the precise definitions. Before implementation, fold the resolution into those original bullets to leave one authoritative algorithm. This does not require another review pass.
