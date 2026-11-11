# One-batch production

1. Replace the production goal's two-hour chain sequence with the first feasible batch from existing whole-chain profit and stock ranking. Keep daily targets, restocking, crop/brewing priority and recipe economics. Update affected documentation and regressions, including guild equipment contention and stale plans.
2. Run simulation regressions. Parent agent arranges independent review, resolves valid findings and commits after review. No commit in the implementation task.

Causal finding: production_at_site executes complete projected route blocks. A blocked later recipe discards the entire block, including feasible earlier work on another workstation. Ranking first batches by complete route economics avoids requiring every future workstation now.

Implementation: production_at_site now asks ranked_recipes_at_prices for first batches after existing restocking, tries them in order, and stops after one successful recipe/crop/brewing action. Whole-chain route qualification remains enabled for the selected batch. Execution already replans at the committed goal boundary and abandons stale goals on MissingInputs; no execution changes were needed.

Save evidence: latest local save saves/universe-1793670923.358698401.ron.gz (version 16, time 2955600000) has Eira/Jori in Weavers’ Guild and Xena/Beni in Second Weavers’ Guild. Each guild site has one spinning wheel and one loom. All four members are sleeping and all four workstations have empty workers lists at this snapshot, so it does not establish active contention. Controlled regressions exercise both occupied-equipment directions and stale competing plans instead.

Validation complete: cargo test -p learning-lord-simulation --no-fail-fast reports 731 passed, 1 failed and 6 ignored. All production, guild, stale-plan, whole-chain and brewing regressions pass. The remaining failure is empty_universe_advances_and_rejects_clock_overflow: advancing an empty universe at u64::MAX by zero milliseconds overflows unchanged construction.rs:490 (day + DAY_MS), through process_construction and compact_construction, before production planning is involved. Left outside scope. cargo fmt --all --check and git diff --check pass.

Implementation, validation and independent review are complete. Review found no high or medium issues and independently passed 190 targeted tests with 1 ignored. Its low documentation finding about the overflow description is corrected above. The unrelated overflow was reproduced and confirmed to bypass production planning.
