# Independent critique: 10 — Inventory cost basis

**Verdict: retain as a deferred research gate; reject as the next Finn intervention.** The document correctly recognises the accounting burden and does not claim an implementation-ready specification. Its recommendation is sound. The revisions below clarify the gate for any future decision to open it; their absence is not an instruction to build the subsystem now. Reviewed against `evidence.md`, `Citizen::economic_wealth`, household accounting, production valuation and `WeightedWellbeing`. No builds or counterfactual simulations were run.

## Prioritised findings

### High — Purchase neutrality can remove the reward for the very work being encouraged

Capitalising only consumed input basis makes a completed production batch book-wealth neutral before fees, losses and taxes; it does not recognise the value added by labour. Actual block-making consumes 25 units of cloth and would retain that material basis without recognising labour's contribution. True no-input recipes such as foraging and water collection can instead produce zero-basis output when unpaid labour is excluded. Conversely, buying expensive blocks preserves their purchase cost. Under duration-weighted wellbeing, a production candidate that finishes goods without selling them can therefore compare worse with sleep even after its shopping penalty disappears. This is a concrete adverse mechanism, not just a concern about displayed accounting.

The current code values held goods at reference in `economic_wealth`, while recipe `output_value` estimates seller revenue. Replacing the first with cost does not reconcile the two. Keep these recognition choices explicit before enabling historical wealth in decisions. Require unsold cloth-to-block and no-input gathering candidates, a purchased-block garment candidate and an actual sale candidate to be compared with sleep. A positive realised margin or a neutral purchase is insufficient evidence of improved action selection.

### Medium — “Do not change decisions” requires a separate ledger and explicit consumer boundary

Step 2 sensibly calls for parallel accounting, but step 3 should name the boundary being crossed. Household members' `wealth()` excludes shared economic assets; `decision_score()` separately weights `economic_wealth()` by household allegiance. `economic_wealth()` also includes other members' carried stock and consumption, worn clothing, crops and brewing batches. Updating only ordinary inventory valuation produces an inconsistent hybrid; summing member ledgers can double-count shared assets.

For the research gate, require a table of consumers and valuations: display, decision score, recipe input opportunity value, output revenue estimate and tax assessment. State which consumer would change in the experiment. Preserve market/input opportunity values independently of historical basis. Identify one authoritative account ledger using existing economic ownership, and define how predictions receive its isolated state. Do not attach independent copies to every household member and reconcile them later.

### Medium — Transfer and work-in-progress invariants need to be acceptance criteria

The plan already lists difficult cases, but “transfers do not change it” is ambiguous across ownership changes. A storage deposit within one owner preserves total basis; a sale between owners removes the seller's proportional basis and establishes the buyer's actual acquisition cost. Gifts, household formation and taxes need separate explicit rules. Defining all transfers as basis-preserving would conflict with the purchase rule.

Specify conservation within an owner across carried, stored, listed, reserved and work-in-progress goods. Inputs removed at action start must carry their basis into pending production rather than disappear until completion; cancellation, crop failure and fermentation must release or destroy it exactly once. Existing wealth smooths meal/drink consumption and credits expected crop/batch output, so these paths need a deliberate treatment rather than a reference-value fallback. Require failed/partial action rollback and branch isolation as well as successful completion tests.

### Medium — Pooling is simple only if residuals, migration and state growth are bounded

A quantity/total-cost pool avoids per-lot history, which is the right starting point. Proportional removal still requires a deterministic fractional-basis rule: integer coin rounding on repeated tiny withdrawals can leave valuable empty pools or erase cost. Require zero quantity to imply zero remaining basis and full liquidation to remove exactly the original aggregate basis, with bounded numerical error if fractional arithmetic is chosen.

Old saves contain quantities but cannot recover actual acquisition history. Versioning alone does not solve that information loss. Choose an explicitly synthetic opening basis or restrict the first experiment to newly initialised worlds; distinguish either from observed historical cost. Do not claim that migrating the named saves reconstructs their real costs.

Keep ledger size bounded by owners × goods plus active work, without retaining transaction history merely to compute weighted averages. Prediction cloning and rollback occur on the planning path, so measure cold/warm decision time and allocations with the parallel ledger even before it affects selection. Reuse the shared protocol's provisional 2× trigger; do not infer negligible overhead from the small number of goods alone.

## Expected effect on Finn

At the day-14 approximate prices, buying a block for 18.75 and valuing it at 12.50 loses about 6.25 reference wealth immediately. Cost basis could remove that isolated loss, assuming the full cash charge is capitalised and no impairment occurs. It neither establishes a buyer for the block nor raises the eventual garment's sale proceeds. It can preserve the apparent value of an overpriced input while suppressing the apparent value added by Finn's own block-making.

The reverted acquisition-cost experiment already changed the candidate without producing any blocks over its 24-hour replay. That was a narrower experiment, not evidence that persistent cost basis succeeds or fails. Day-16's rejected purchase and absent completed block trades likewise cannot establish an impairment value or sales demand. No behavioural improvement should be claimed for this unimplemented option.

## Smallest useful alternative and required revisions

Keep persistent world wealth unchanged while investigating the narrower acquisition-cost forecast correction in plan 01, using the existing before/after shopping and actual activity measurements. A temporary diagnostic bridge between cash spent, reference asset value and estimated sale revenue can expose the penalty without a save migration or a persistent accounting ledger. It does not itself cure inactivity; it isolates the causal question more cheaply.

Before reconsidering plan 10, add the no-input/unsold-production counterexample, the valuation-consumer boundary, ownership versus location transfer distinction, work-in-progress invariants, opening-basis policy and a bounded pool arithmetic/performance requirement. Only open implementation after evidence shows that changing persistent wealth is desirable independently of Finn. The existing deferred verdict should remain.

## Fresh review of the revised research gate

**Verdict: accept as a deferred research gate; integrated wellbeing remains not implementation-ready.** The added review-resolution section addresses the findings at the appropriate level: value-added neutrality is explicit, consumer policies remain separate, economic-account authority and prediction isolation are named, ownership changes are distinguished from storage movement, and work-in-progress basis has conservation requirements. Bounded aggregate pools and explicit opening-basis provenance address the arithmetic and migration risks without inventing unavailable purchase history.

The fresh-world shadow-ledger path is coherent; a future synthetic migration remains an explicitly labelled alternative, not historical evidence. Labour capitalisation, impairment, score integration and numerical tolerance still require decisions before implementation, as the revised document acknowledges. Those deferred choices do not block acceptance of this research gate. No new high or medium issue was found in the revised scope. The earlier block example above has also been corrected: actual blocks consume cloth; foraging/water provide the no-input example.
