# Consumption availability

One implementation step: calculate food and drink availability once per consumption selection, verify equivalent decisions, measure the result, review and commit.

- Capture one fresh desktop universe and reuse its saved initial state for repeated five-day before/after runs. Keep build settings and measurement conditions consistent; preserve baseline binaries and final states for comparison.
- Discover accessible co-located storage once and count usable food, water and beer together. Reuse these temporary quantities during meal enumeration and candidate scoring, rebuilding them for each later forecast meal.
- Counting does not require sorting locations. Preserve the existing priority when actually consuming goods, and preserve ownership, access checks, reservations, saturation, scoring and tie-breaking.
- Keep the change local to consumption selection. Avoid persistent caches and changes to planning search or simulation rules.
- Verify availability and consumption choices against the original behavior, including stored/carried supplies, households, inaccessible places, reservations, partial meals, drink choices and depletion across forecast meals. Run relevant tests, formatting and strict Clippy.
- Compare repeated timings and CPU profiles on the fixed fixture. Obtain independent review, resolve valid high/medium findings and request fresh review after fixes. Commit the completed step.

## Results

Implemented a temporary food/drink availability array for each meal or drink selection. One unsorted pass discovers accessible co-located stores; candidate enumeration and scoring reuse its quantities. Forecast meals rebuild the array after consumption. Actual stock consumption retains its sorted source priority.

Three sequential runs per version, loading the same saved fresh desktop universe and advancing five days:

| Run | Before | After |
| --- | ---: | ---: |
| 1 | 79.241 s | 53.090 s |
| 2 | 76.708 s | 53.013 s |
| 3 | 75.876 s | 53.196 s |
| Median | 76.708 s | 53.090 s |

Median advancement time fell **30.8%**. Median process user CPU time fell from 186.107 s to 123.762 s (**33.5%**); CPU time includes worker threads. These measurements cover one fixed fresh universe in the normal development build, not a release build or every possible town. Setup, serialization and final-state comparison are outside advancement timing. Every later run asserted exact final `Universe` equality with the first baseline run.

The baseline is commit `1509bce` plus only the fixture/comparison harness changes. The fixture SHA-256 is `9ec393f090993b37a1d55a35aa57b3d7e727e798eeac265da850231815e52072`. Local logs, snapshots, timing JSON, raw profiles and flamegraphs are in the ignored directory `debug/consumption-availability-2026-10-13/`. A separate frame-pointer build was used for each CPU profile; the clean timings above used normal development builds.

At 99 Hz, the before/after profiles captured 18,585/12,285 user CPU samples, with no lost samples. Both profiling runs also passed final-universe equality. The union of `available_units` and the new `consumption_available_units` accounted for 42.15% before and 15.68% after (7,833 versus 1,926 samples, approximately 79.12 versus 19.45 CPU seconds). `food_forecast` fell from 55.35% to 34.68% (10,286 versus 4,261 samples); it remains a substantial cost. These are inclusive sampled call-stack costs, overlap with one another and must not be added. Inlining can also affect individual symbol attribution; the repeated clean timings establish the overall improvement.

The ignored workload accepts `LL_PROFILE_FIXTURE` (load an existing initial universe, or create a fresh one if absent), `LL_PROFILE_EXPECTED` (assert final universe equality) and `LL_PROFILE_OUTPUT` (write a final snapshot without overwriting an existing file). To repeat against the retained fixture:

```sh
LL_PROFILE_FIXTURE="$PWD/debug/consumption-availability-2026-10-13/fresh.ron" \
LL_PROFILE_EXPECTED="$PWD/debug/consumption-availability-2026-10-13/before-1.ron" \
cargo test -p learning-lord simulation::tests::fresh_universe_five_day_profile -- --exact --ignored --nocapture
```

Validation: 642 workspace tests passed, 6 ignored, no failures, including the ten-day town regression. New tests compare batched availability with the original per-good queries under access, ownership, reservation and saturation constraints, and verify depletion between forecast meals. Strict workspace Clippy, formatting and whitespace checks passed. Independent code review approved the implementation without findings.
