# Household living standards

Implement and review as one coherent step, then commit after independent review.

- Persist household tier and assessment state, defaulting existing households to tier 1. Carry the household tier into citizen planning context.
- Gate clothing and dietary satisfaction, forecasts, consumption preferences and reserve provisioning by tier. Preserve Beer carbohydrate intake and add alcohol satisfaction using the agreed dietary model.
- Assess completed seven-day cash flow daily, using classified cash receipts and payments. Reset eligibility after tier changes, and extend promotion eligibility after demotion.
- Add bounded household developer controls and a compact current-tier display with assessment details and all tier descriptions in its tooltip.
- Verify promotion, demotion, cooldowns, inactive needs, provisioning and save compatibility with focused tests, then run relevant existing checks. Have an independent agent review the complete change and resolve valid findings before committing.

## Accounting findings

Existing household history records goods purchases and receipts, inn payments and receipts, taxes, workshop fees, and construction payments separately. Client construction escrow movements are excluded. Carpenter receipts count as earnings and carpenter material purchases count as production expenses. The promotion buffer uses all operating expenditure, as clarified during implementation. No loan principal or interest system currently exists. Unsold goods and starting savings must not contribute to this assessment.

## Implementation and review

The coherent implementation step is complete. Independent reviews found and resolved malformed-save lookup, stale active plans, inactive alcohol intake, legacy plan migration, and internal construction receipt issues. Fresh implementation and test reviews reported no findings.

Construction payments between members of the same household are excluded from operating income with a small daily receipt counter. Carpenter material purchases retain the agreed production-expense treatment. Existing saves start a fresh tier 1 assessment period and cancel their old queued purchase plans.

## Validation

- Full simulation library and integration suite: 741 passed, 6 ignored, 1 filtered.
- The filtered test, `empty_universe_advances_and_rejects_clock_overflow`, also fails on an isolated unmodified HEAD checkout at `construction.rs:490` due to an existing integer overflow. It was left outside this change.
- Focused coverage includes actual daily advancement and save validation, rolling cash flow, exact promotion threshold, fresh-day resets, demotion cooldowns, inactive needs, internal construction receipts, migration, and malformed saves.
- Formatting and whitespace checks pass.
- Final desktop tier controls and tooltip tests: 2 passed.

A cancelled plan's market demand can remain until the next planning refresh. This is a minor existing request-refresh limitation; tier changes cancel pending purchases immediately.
