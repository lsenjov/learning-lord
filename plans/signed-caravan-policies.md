# Signed caravan policies

## Scope

Global and per-good import/export rates add without clamping. Effective rates at or above 100% block the direction. Imports have a per-good switch that also blocks existing stock. Negative rates fund subsidies from the shared town treasury, with partial fills and signed rounding carry. Preserve separate global and good drafts and migrate existing saves through defaults.

## Implementation

- [x] Implement the cohesive policy change across simulation pricing, planning, settlement, treasury limits, per-good and global controls, save compatibility, documentation and meaningful regression tests. Run relevant checks, obtain independent review through the parent, resolve findings and commit only after review.

## Validation

Check stacking and offsets, blocked rates, signed settlements, import switches on existing stock, shared subsidy exhaustion and conservation, signed carry, legacy saves, independent drafts, and existing caravan behavior. Run formatting, simulation tests and desktop tests/checks.

## Completion

Implemented policy, treasury-aware settlement/planning, controls, compatibility, documentation and regression tests. Independent review completed; findings resolved.

- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo test -p learning-lord-simulation -- --skip empty_universe_advances_and_rejects_clock_overflow`: passed. The skipped construction timestamp overflow reproduces on clean HEAD.
- `RUST_MIN_STACK=8388608 cargo test -p learning-lord caravan`: 14 passed.
- Full desktop run with the larger test stack: 172 passed, one ignored, two failures reproduced on clean HEAD: `citizens::tests::details_distinguish_current_goal_from_forecast` and `tests::decision_panel_shows_reachable_hunger_and_uses_saved_prices`. The default test stack overflowed in a separate desktop test, so verification used an 8 MiB test stack.

## Review fixes

- Export subsidy payments now solve for cumulative asking coverage using actual seller receipts and current signed carry. Treasury affordability and settlement use the same payment calculation. Existing nonnegative-rate gross-up rounding is preserved.
- Purchase estimates use the fill plan's actual units and coins, then price missing units at reference, including exhausted and partially funded import subsidies.
- Added regressions for reachable positive carry 9998 followed by a -2500 rate, whole versus fragmented orders, partial treasury affordability, conservation, and minimum subsidy payments checked against brute force across signed carries and subsidy rates.
- `cargo test -p learning-lord-simulation --lib marketplace::tests`: 52 passed.
- `cargo test -p learning-lord-simulation --test caravans`: 9 passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed after fixes. Broad baseline suites were not rerun. Fresh independent review found no remaining high or medium findings; its low documentation finding was corrected.

The fresh reviewer independently reran all 52 marketplace tests and 9 caravan integration tests successfully. The completed implementation and review fixes are committed together with this plan.
