# Drinking water reserves

1. Completed: add a carried-water wellbeing bonus (+10 at 400 g, diminishing to +14 at 1,200 g), include unconsumed active drinks, extend reserve replenishment with a 400 g water target through existing withdrawal/buy/fetch paths, and retain 400 g in addition to production ingredients. Verify history integration, planning acquisition and stock retention.
2. Completed: display drinking reserves and their wellbeing contribution, document the policy, run impacted checks and a town performance probe, and obtain independent review.

Food reserve candidates remain unchanged. Water replenishment is an alternative within the existing reserve goal; no extra goal or starting water is introduced.

Backend validation: full simulation unit, integration and documentation tests pass. History tests cover standalone and concurrent drinking across bonus clamps; acquisition tests cover buying, fetching and prior withdrawal of own listings. The isolated deterministic 24-citizen release probe advances one day in 2.65 seconds, compared with 2.48 seconds before this change.

Desktop validation: 101 tests passed in the full run; the remaining sleep-readout fixture was updated to supply drinking reserves and its focused rerun passed, including the new reserve display assertion. Workspace format and strict Clippy checks pass. Independent review reports no remaining issues.
