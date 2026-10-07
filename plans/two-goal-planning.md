# Two-goal planning

Compare complete single goals and ordered pairs of distinct goals using the existing duration-weighted wellbeing score. Keep each goal's four-hour prerequisite bound; production has one two-hour target measured from its own start.

## Completed implementation

One coordinated change updates search, execution and background planning together:

- [x] Replace recursive horizon continuations with bounded single/pair evaluation and fixed production duration.
- [x] Commit execution and shopping demand to the first complete goal; begin background planning when its final action starts if it lasts at most thirty minutes, otherwise when thirty minutes remain.
- [x] Update readouts and documentation, verify goal boundaries and lifecycle behavior, run workspace checks and the fixed spatial benchmark, and obtain an independent review.

The second goal is a forecast only. Failed prerequisites continue to trigger replanning from actual state. Scheduler ordering, standing listing policy, random execution yields, and scoring remain unchanged.

## Validation

- `cargo test --workspace --locked`: 279 passed, two manual benchmarks ignored.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Native desktop inspection confirmed the first-goal completion boundary and separate forecast heading, with the 24-citizen roster intact.
- Independent Sol-6.1 review found no high, medium or low issues.

## Performance

Ran the existing release `larger_spatial_town_benchmark` with 24 citizens for 14 simulated days, on 16 logical CPU threads with 12 planning workers. The fixture uses a fixed spatial layout, citizen IDs and forage seeds, and exercises the real background planning runtime. No task-owned desktop, build or other test was running during measurement; the user's existing desktop was left open.

| Measurement | Previous workplace-sales run | Two-goal run |
| --- | ---: | ---: |
| Full benchmark | 124.100 s | 32.18 s |
| Startup | 41.147 ms | 35.693 ms |
| Day 14 | 8.619 s | 3.078 s |
| Final trade count | 1,415 | 1,711 |
| Final private-location trades | 1,181 | 1,433 |
| Final live sell orders | 536 | 531 |

This run was approximately 3.9 times faster. All 14 days completed without errors or hitting the per-day time cap. Needs remained finite, balances remained nonnegative, and total coins stayed at 7,200 at every daily sample. Maximum hunger was at most zero at each daily sample; this does not establish the maximum between samples.

This is an observed comparison, not a guaranteed speedup: goal selection and replanning behavior have changed, and place UUIDs can still affect tied choices. Memory usage was not measured. Different candidate durations still affect average-wellbeing comparisons, and prerequisite search remains variable in cost.
