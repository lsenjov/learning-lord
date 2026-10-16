# Review 07 — Useful reserve value

**Verdict: retain as a deferred design alternative; revise before authorizing a prototype.** The document correctly identifies this as a supply-security preference, freezes valuation within a decision, excludes self-quotes and avoids adding inventory's full value twice. Its explicit design gate is appropriate. The following are design requirements to settle at that gate, not requests to implement the feature now.

## Prioritized findings

### Medium — Reserve consumption can make productive work less attractive

**Location:** `07-useful-reserve-value.md:9–12, 25`.

The proposal discusses hoarding, but does not spell out the immediate consequence for the stated Finn problem: sleep retains the premium, while assembling garments consumes eligible blocks and cloth and loses it. With a fixed requirement snapshot, finished garments do not replace that input benefit. `WeightedWellbeing::add` (`planning/goals.rs:382`) averages endpoint scores over action duration, so the loss matters during production, not merely at the final inventory comparison. Purchases may look better while subsequent work looks worse.

Required revision: add an explicit hypothesis and acceptance observation for **shopping followed by production versus retaining the same supplies and sleeping**. Report the premium lost on consumption separately from shopping gains and output wealth. The day-14 acquisition experiment already demonstrated that improving a candidate's economics did not make work beat sleep; it cannot establish this proposal's benefit. Day-16 Finn may remain asleep or become a better stocked sleeper. No counterfactual currently supports a stronger prediction.

### Medium — Finite depth and aggregate caps need a valuation contract

**Location:** `07-useful-reserve-value.md:10, 12–13, 23`.

A frozen quote curve is necessary but insufficient to define `replacement_cost`. Suppose an account has ten eligible blocks and external sellers offer only one. The text says what happens with no supply, but leaves open whether that single quote prices all ten, only one, or a failed ten-unit replacement. Likewise, a per-unit ceiling can still produce a large account bonus across grams, ingredients and workplaces. “Prevent ... dominating hunger, thirst or sleep” is not implied by a per-unit cap.

Required revision: require a finite quantity-depth valuation, an explicit partially supplied tail rule, monetary-to-score units and an aggregate account-level ceiling. Require zero/partial depth, small cheap plus expensive depth, per-seller rounding and finite-result checks. Exact numerical knobs can remain deferred; these semantics cannot. Exclude the whole economic account's listings through `market_party()`, rather than only the deciding citizen's ID. The latter misses household self-quotes (`households.rs:34–44`).

### Medium — Shared stock and dietary overlap are acknowledged without a decision rule

**Location:** `07-useful-reserve-value.md:11, 19, 23–24`.

Counting by account/workplace does not by itself prevent the same owned unit backing requirements at two workplaces. Existing `refresh_workplace_supplies` sums worker requirements by `(economic_id, place)` (`households.rs:395`), while `decision_score` adds shared wealth using household allegiance (`households.rs:58`). Neither provides a cross-workplace allocation of physical reserve stock. A transfer-invariance test alone also passes implementations that duplicate the bonus everywhere. Similarly, merely testing food overlap does not decide whether berries/water can earn both dietary and production bonuses.

Required revision: specify one account-wide physical-unit budget allocated deterministically across workplace requirements, with incoming/reserved units counted once and unavailable units excluded. State whether dietary-protected units are excluded from production eligibility, or explicitly justify and bound additive value. Check simultaneous workers, two workplaces, goods in transit and consumption, as well as relocation. Keep dietary protection intact. These can be stated as prototype preconditions without settling every allocation parameter now.

## Circularity and implementation cost

Freezing requirements is a sound boundary if their source is independent of the bonus. Existing `projected_recipes_at` evaluates owned inputs at reference and missing inputs against market depth (`production.rs:543, 629`), and permits a limited unprofitable trial. Require the intention snapshot to use that unmodified economics and explicitly define whether trial inputs are excluded; “must not create unlimited” still permits a bounded subsidy for a recipe already known to lose money.

The performance warning is sensible. Prefer one immutable requirements/quote context per evaluated root decision/account, shared across its candidate branches and before/after samples. Caching independently per candidate can multiply identical quote work and may accidentally freeze different curves after branch mutations. Inventory-dependent eligibility can be evaluated against that shared context. Use the evidence document's same-state cold/warm median/p95 protocol; no new benchmark or build was run for this review.

## Simpler alternative and required decision

If the objective is to reduce Finn's expensive speculative procurement, first compare the already proposed smaller procurement alternative on identical snapshots. It changes fewer policies and directly exposes shopping cost versus actual work. If the objective is explicitly to retain production safety stock even at a wellbeing cost, this proposal is a defensible separate experiment after the three revisions above. Do not adopt it merely because reserve-inclusive scores increase: require actual production, consumption and sales alongside stock accumulation over the shared replay protocol.

Reviewed the proposal, `evidence.md`, and relevant production, scoring, household and restock code. No implementation, builds or simulations performed.

## Fresh review of the resolution

**Verdict: suitable for an offline component-value diagnostic; integrated scoring remains a design gate.** The new resolution addresses all three medium findings: it explicitly measures the consumption penalty against sleeping, defines finite externally covered replacement depth and an account-wide ceiling in wellbeing units, and allocates a unique physical inventory budget after dietary protection. It also excludes household self-quotes and trial-only inputs and shares one immutable root-decision context. No unresolved high or medium finding blocks that bounded research step.

This is not approval to add the component to planner scores or adopt reserve preference. Before an integrated prototype, choose C and the horizon and specify household-score placement so shared value is applied once with the intended allegiance weighting. The diagnostic should report raw component values and separately label any hypothetical capped-score calculations with their assumed parameters. It must not treat a higher hypothetical score as evidence that Finn works more; the shopping → production versus shopping → sleep comparison remains the decisive causal check, followed by the common replay protocol if integration is later chosen.

The earlier findings above record the original review and are resolved by the appended plan section at this research scope. No implementation, builds or simulations were performed in this fresh review.
