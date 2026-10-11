# Daily guild summaries

## Agreed behaviour

- Show Current and Daily history views for guilds in Collectives, matching household history navigation.
- Record actual production worker-hours and completed output by member and workshop. Goods remain owned by their existing economic account.
- Record opening/closing guild coins, fees accrued, payments received, expenses paid and outstanding fee obligations, including fractional carry.
- Close days at 04:00; keep the current partial day and 365 completed days.
- Store compact totals, not event logs or simulation snapshots. Histories must not participate in prediction.
- Preserve clickable entities, stable day selection, idle UI behaviour and strict save-version compatibility.

## Step 1: simulation accounting

- Add compact daily records, execution/accounting hooks and persistence validation.
- Test fractional fees and debt repayment, shared accounts, production attribution, day boundaries, split advancement and retention.
- Independently review, fix findings and commit.

## Step 2: guild history UI

- Add history navigation and linked financial/activity summaries.
- Update documentation and UI tests; independently review and commit.
- Run workspace tests, formatting, strict Clippy and build.

## Progress

- Both steps implemented and independently reviewed; no high or medium findings remain.
- Tests cover exact fees and debt repayment, shared accounts, interrupted work, output across 04:00, weekly tax timing, caravan repayments, retention and save validation. A regression preserves historical payer identities after later household formation.
- UI tests cover linked rows, day navigation, rollover/pruning, idle caching and reset behaviour. Household and guild views share day controls.
- Locked workspace tests and build, formatting, strict all-target workspace Clippy and diff checks pass. Save format is version 6 under the existing strict compatibility policy.
- Step 1 committed as `070aa90`; step 2 complete with reviewed and verified guild history UI.
