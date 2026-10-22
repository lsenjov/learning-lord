# Priority goal planning

Implement separate goals in order: thirst, hunger, clothing, tiredness, reserves, listing, production, wait. Evaluate one eligible goal at a time and stop at the first feasible plan, retaining existing within-goal planning, production quotas, and economic constraints. Remove two-goal continuation searches.

Needs trigger above zero; clothing replacement at 10% condition. Household reserves trigger below 100 nutrition or 400 g drink per member and refill toward 300/1,200; acquire one spare per member at 20% garment condition. Listing uses a 12-hour interval after successful work. Blocked goals fall through.

Single implementation step: update planning and eligibility, maintenance timing, household clothing snapshot, and behavioral tests; run simulation checks; obtain independent review and address findings; commit.

Completed implementation and independent review; no high or medium findings remain. Updated integration fixtures to isolate production and updated assertions for one-goal plans and unattempted lower priorities.

Validation: all simulation tests passed (556 passed, 6 ignored), workspace `cargo check` passed, formatting and diff whitespace checks passed.

Observed policy limitation: a household with no coins and depleted pantry can spend its time gathering food, drinking, eating and sleeping, postponing production indefinitely. Tracing confirmed replenishment worked on actual shortages; no unrelated topups or quota failure caused the delay. This follows the agreed fixed priority and remains a tuning/design consideration.
