# Supplier recipe lookup

1. Replace the global available-recipe search in `production/routes.rs::supplier` with output-first filtering of `Recipe::ALL`, followed by the existing crop/brewing exclusions, skill/duration eligibility, and direct validation of the specified site. Preserve the exactly-one-supplier rule and no-site result. Compare against the previous lookup across goods, skills, and site access, run simulation tests, obtain a fresh review, and verify the saved-game replay before committing.

Validation:

- The old/new equivalence test covers every good, absent skills and each skill at zero/max practice, duplicate owned workshops, foreign and guild access, public and town sites, a missing site, and no site. Current recipes have unique outputs; the existing exactly-one check remains intact.
- `cargo test -p learning-lord-simulation`: 558 passed, 0 failed, 6 ignored. Full output: `/tmp/ll-supplier-simulation-tests.log`.
- Independent implementation review approved with no findings.
- The unchanged 24-hour, 30-citizen saved-game replay took 48.451954 seconds in a fresh run; the previous three-run median was 48.699756 seconds. Optimized runs took 34.370987, 34.711202, and 34.567589 seconds, with a median of 34.567589 seconds: 29.02% lower than the previous median and 28.66% lower than the fresh baseline.
- A 15-second profile (3,078 samples) still shows supply refresh and site access dominating: `refresh_supplies` 81.58% inclusive, `production_place` 50.10% inclusive, `may_use_object` 46.75% inclusive / 37.72% self, and goal planning 4.06% inclusive. Further optimization is outside this step.
