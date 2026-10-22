# Citizen grouping and sorting

## Approved scope

Add compact Citizens roster grouping by None, starting Profession (default), Household, designated Workplace, Wealth fifth and current Activity. Groups start collapsed, expand to existing citizen inspectors, and support Open all / Collapse all. Independent household rows open their citizen directly. Sort Name ascending, Wellbeing ascending and Wealth descending by clickable headers, reversible; aggregate groups use mean wealth and median needs wellbeing, with names breaking ties.

Summaries show citizen count, mean coins, mean wealth and median wellbeing. Allocate shared household coins and economic assets evenly while retaining each citizen's personal assets. Wealth fifth boundaries approximate population fifths without splitting households. Activity maps current actions into producing, travelling, trading, moving supplies, personal needs and waiting/idle. Explanations use tooltips. Guild and production-chain grouping are deferred.

## Single implementation step

Implement the roster view model, compact native UI and state/selection integration; update relevant documentation and meaningful tests. Preserve controls and expansion during snapshots, reset stale identity state on load/restart, and preserve existing inspector navigation and keyboard selection. Obtain an independent review, resolve valid medium/high findings and obtain a fresh review after fixes, then commit the completed step.

## Validation

Test partition membership, household asset attribution and aggregation, medians, household-preserving fifth boundaries, exhaustive activity mapping, sorting defaults/directions and ties, group controls/expansion/citizen selection, snapshot persistence and restart reset. Run formatting, targeted roster tests and desktop suite (excluding the optional ten-day economic probe), then Clippy as appropriate. Use headless Bevy interaction tests where desktop rendering is unavailable.

## Validation record

- Citizens tests: 19 passed, including all grouping partitions, household accounting, fifth boundaries, aggregate and child sorting, activity mapping, menu visibility, expansion, identity-safe selection, snapshot refresh and restart.
- Serial desktop suite: 156 passed, one manual profiling test ignored, ten-day economic probe excluded. The final menu guard was subsequently covered by the Citizens tests and the affected inspector/window test (passed).
- `cargo clippy -p learning-lord --all-targets -- -D warnings`, formatting and diff whitespace checks passed.
