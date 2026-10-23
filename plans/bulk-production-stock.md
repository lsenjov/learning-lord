# Bulk production stock forecast

## Single implementation step

Replace per-good stock initialization in recipe projections with one bulk forecast. Preserve economic ownership, active recipe input clamping and outputs, pending planting, crop care forecasts, and brewing totals. Traverse accessible properties once for delayed output, retaining the existing single-good path for callers that need one good. Share crop forecasting only; keep usable owned inputs separate.

Validate exact bulk/single-good equality and expected values using active production, planting, crop tending and missed care windows, brewing preparation/fermentation, and shared/private vessel fixtures. Run formatting and the simulation suite. Review and measure the fixed 30-citizen, 24-hour replay before committing.

## Validation

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `TMPDIR="$PWD/target/profile-tmp" cargo test -p learning-lord-simulation` passed: 376 unit tests and 187 integration tests; 6 ignored. Full output: `target/bulk-stock-tests.log`.
- Bulk stock matches the existing per-good calculation bit for bit in the added fixtures. Existing projected recipe regression tests pass with the new initialization.
- Independent review approved the implementation and final fixtures without findings.

## Performance

Fixed replay: `/tmp/ll-priority-current.ron`, 30 citizens for 24 simulated hours, using `/tmp/ll-priority-profile.rs`.

- Fresh baseline runs: 19.516598 s, 19.293700 s, 19.122272 s; median 19.293700 s.
- Bulk forecast runs: 14.374602 s, 14.131479 s, 14.405654 s; median 14.374602 s.
- Median elapsed time fell 25.5% (about 1.34× faster). The earlier historical 16.729934 s baseline was not used for this comparison because the fresh baseline differed.
- A 15-second profile collected 3,306 samples. Inclusive costs: `production_stocks` 4.93%, projected recipes 58.32%, workplace supply refresh 55.60% across two split symbols (31.16% + 24.44%), `may_use_object` 11.19% across two split symbols (6.59% + 4.60%), and planning 11.65%. Inclusive parent/child costs overlap and should not be added together.
