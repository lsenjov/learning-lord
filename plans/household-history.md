# Daily household summaries

## Agreed behaviour

- Add Current and Daily history views to household details in Collectives.
- Record household cash opening/closing balances, sales receipts, purchases and taxes paid without double counting withheld taxes.
- Record produced, bought, sold, consumed and tax-collected goods, with member work/production/consumption contributions.
- Snapshot closing household stocks by location or carrier, distinguishing listed goods and tax reservations from available supplies.
- Transfers inside the household are not production, consumption or trade. Market revaluation is not cash income. Sales belong to the household, not the member who lists goods.
- Days close at 04:00 using existing boundary conventions; show the current partial day and retain 365 completed days.
- Persist compact totals and nonzero closing stock entries, not transactions, plans or universe snapshots. Existing strict save compatibility applies.
- Preserve clickable entity references and existing simulation decisions.

## Step 1: simulation records and persistence

- Add bounded household daily records and hooks for activity, production, consumption, trade and tax collection.
- Capture closing stocks and reconcile actual cash flows with account balances.
- Validate persistence, day boundaries, large versus split advances, shared-account accounting and retention.
- Measure representative compressed history size; independently review, resolve findings and commit.

## Step 2: household history display

- Add Current/Daily history selection, current partial totals and completed-day selection in Collectives.
- Render cash, goods, member activity and closing supplies using existing entity links.
- Keep history selection stable during updates and avoid rebuilding unchanged content.
- Update documentation, run relevant tests and formatting/Clippy/build checks, independently review and commit.

## Progress

- Step 1 complete and independently reviewed. Daily totals, member contributions, closing stocks and save-format version 4 implemented. Six targeted accounting/retention/persistence tests and strict simulation Clippy pass.
- Size sample: 365 varied days for one household, two members, eight active goods and sixteen stock entries per day serialize to 2,546,854 bytes of pretty RON and 126,864 bytes of gzip (level 6), approximately 124 KiB per household-year. This is a reproducible synthetic sample, not a simulated year.
- Step 2 complete and independently reviewed. Current/History views, stable day selection, linked summary rows and idle refresh guards implemented; targeted UI tests pass.
- Final verification: full workspace tests, formatting, strict all-target workspace Clippy and the desktop build pass. No remaining high or medium review findings; cash-category documentation clarified.
