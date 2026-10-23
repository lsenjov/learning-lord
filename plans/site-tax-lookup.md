# Site tax lookup

Make explicit production tax lookup read only the selected site’s cached rate, with missing entries returning zero. Preserve recipe-based lookup and its cache for existing callers.

1. Remove the unused recipe argument from site lookup and update callers. Cover stacked local and location-type tax, untaxed default and other sites, and save roundtrip. Run the simulation suite, obtain a fresh code review, resolve findings, then commit after review and performance verification.

## Completed

- Site lookup now uses only `(place, good)`, returns zero for missing entries, and no longer accepts a recipe. Recipe lookup and both caches remain intact.
- Regression coverage includes stacked local and location-type rules, untaxed default/other sites, untaxed goods/locations, save roundtrip, and an intentionally populated recipe cache with a missing canonical-site entry.
- `cargo test -p learning-lord-simulation`: 559 passed (372 unit and 187 integration), 6 ignored, no failures; doc tests passed with zero tests. Log: `/tmp/ll-site-tax-tests.log`.
- The final missing-site unit was added after the full-suite compile began, then passed separately with `cargo test -p learning-lord-simulation --lib missing_site_socage_rate_does_not_fall_back_to_recipe_rate`. Log: `/tmp/ll-site-tax-unit.log`.
- `cargo fmt --all --check` and `git diff --check` passed. Independent code review reported no findings.
- Release benchmark, 24 simulated hours with the same 30-citizen save: fresh baseline 34.831732 s; earlier baseline median 34.567589 s. Three changed runs: 17.167226 s, 16.664385 s, 16.729934 s; median 16.729934 s, 51.60% lower elapsed time (2.066× throughput) versus the earlier median.
- A 15-second profile collected 3,167 samples: site tax lookup 0.03% inclusive (inlined), `production_place` 1.01% versus 50.10% before. Remaining inclusive costs included `refresh_supplies` 67.54%, `production_stock` 29.40%, `may_use_object` 25.17% (22.17% self), and ordered planning 7.51%.
