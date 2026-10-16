# 07 — Value useful reserves at replacement cost

## Hypothesis and scope

Useful production inputs can be worth more to their owner than their likely immediate sale proceeds. A bounded reserve value could recognise avoided future purchases without repricing every good town-wide. This is an explicit preference for supply security, not neutral accounting.

## Proposed algorithm

- Keep ordinary reference wealth. Add a separate reserve benefit for eligible input units up to a fixed production block's requirements, calculated from already-defined workplace production intentions.
- For those bounded units, use a nonnegative premium `max(0, replacement_cost - reference_value)`. Cap the premium to prevent an extreme ask from dominating hunger, thirst or sleep. Do not add the full inventory value twice.
- Calculate quantities per economic account/workplace and share the benefit consistently with existing household wellbeing. Deduct reserved/incoming ownership correctly; a transfer home → workplace must not create or destroy the same account's reserve benefit.
- Snapshot requirements and the external quote curve at the beginning of the evaluated decision. Reuse it for both before/after values to prevent hypothetical purchases from exhausting cheap listings and inflating the value of remaining stock within the same comparison.
- Exclude own sell listings from replacement evidence. No external supply means no new scarcity premium in the first version. Trial-only and unprofitable output recipes must not create unlimited useful reserves.

## Critique

This deliberately makes holding supplies rewarding. It can encourage hoarding, relieve sellers of real scarcity constraints, and reward stock whose final product never sells. A frozen curve prevents within-plan mark-to-market gains but price changes can still inflate reserves at the next decision. The cap and horizon become important tuning choices.

Determining 'useful' is circular if the production mix itself depends on this premium. Shared ingredients must be counted once, and a good may be both food and a production input. Existing food reserve wellbeing already gives additional value to useful stock, so overlapping bonuses can double-reward berries or water. Production input reserve utility should not silently replace dietary protection.

## Implementation plan if chosen

1. Choose and document the reserve horizon, premium ceiling and shared-account allocation. Use a fixed requirement snapshot, not recursive optimisation of bonus and production.
2. Prototype the premium as an isolated score component, report its contribution in diagnostics and test ownership/location invariance, own-quote exclusion, mixed recipes and food overlap.
3. Compare reserve fulfilment, purchases, consumption, wellbeing and stock accumulation over multiple days; review before adoption.

## Evaluation and recommendation

Potentially addresses useful inventory directly, but adds more policy than two procurement sizes. Quote-depth evaluation must be cached once per candidate/account rather than run on every wellbeing sample. Defer until we decide that retaining reserves is an explicit wellbeing objective. It can mask bad production economics if used simply to make Finn work.

## Review resolution: bounded preference, not free investment credit

- Explicitly compare shopping → production against shopping → sleep. Consuming ingredients removes their reserve premium; report that loss separately from output wealth. Reject this as an inactivity intervention if it merely creates well-stocked sleepers. This is a genuine disadvantage of a reserve preference, not a bookkeeping bug to hide with another bonus.
- Use one frozen external depth curve per economic account/root decision. Exclude listings of the entire household/account, not just the citizen. Only externally covered quantity earns a replacement premium; uncovered tail earns zero extra premium. Compute actual replacement charge with existing per-seller rounding, then subtract reference for the same covered quantity. Convert once using WEALTH_WELLBEING_PER_COIN and clamp the total account premium to finite C wellbeing units. C and the reserve horizon must be approved before a prototype; there is no unconstrained per-unit bonus.
- Allocate one physical usable account inventory budget deterministically across sorted workplace IDs and their frozen requirements; remove allocated units before the next site. Incoming units are counted once only when available under existing delivery semantics; state-reserved/inaccessible stock is excluded. Existing dietary-protected quantities are allocated first and excluded from production-premium eligibility, including food/drink inputs.
- Freeze intentions from the unchanged production economics, exclude trial-only inputs, and reuse the same context in sibling before/after evaluations. No recursive bonus-influenced production target calculation. Tests cover two members/two sites, carried inputs, transit and dietary overlap.
- The only next authorized step this plan could support without further design is an offline component-value diagnostic. Integrated reserve scoring remains gated on C/horizon/household-score placement and evidence of actual productive use.
