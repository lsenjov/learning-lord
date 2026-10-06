# Background planning

One cohesive implementation step spans the scheduler, simulation runtime, and desktop worker because these APIs and cancellation semantics must be validated together. Commit the complete feature after independent review and all required checks.

- Replace per-request Rayon dispatch with a bounded, long-lived global FIFO of starting-goal jobs. Preserve original-order reduction, cancellation, deterministic errors, and sequential fallback if pool initialization fails.
- Keep exported universes immutable and free of shared task handles. A separate runtime owns at most one pending request per citizen and is used by the desktop worker; existing synchronous APIs remain available.
- Submit only during a current action known to finish the commitment or exhaust the plan. Submit immediately when a final action of at most thirty minutes starts; for longer final actions add a simulation event when thirty minutes remain. Complete only that action on a clone using expected forage and frozen market context. Unexpected shortages retain fresh planning at the actual boundary.
- Adopt only the plan at its matching citizen, original-plan, action-index and clock boundary. Check the first action against actual state, fall back to actual-state planning when the forecast is unusable, and never copy projected resources, needs, RNG or market into live state.
- Integrate the runtime with the desktop worker, keeping restart and shutdown responsive during waits, preserving command order and the last successful snapshot on errors or interruption.
- Verify FIFO ordering, worker bounds, cancellation, panic recovery, timing, game-day jumps, adoption authority and snapshot isolation. Run formatting, strict Clippy and the workspace suite; measure a fixed-fixture day advance in both modes. Repeat independent review until no high or medium issues remain; fix low documentation issues directly.

## Fixed-fixture day measurements

Run `cargo test -p learning-lord-simulation fixed_town_day_benchmark -- --ignored --nocapture --test-threads=1` for the manual benchmark. It advances the same cloned six-role town by twenty-four simulated hours in synchronous and background modes, with forage seeds 42–47, fixed citizen IDs, and a fixed map whose sites are co-located. Both modes use the same optimized simulation development build and initialized global FIFO pool. Round zero warms up both modes; rounds one through five alternate which mode runs first.

| Round | Synchronous (ms) | Background (ms) |
| --- | ---: | ---: |
| Warm-up | 1193.982 | 794.032 |
| 1 | 1045.085 | 758.646 |
| 2 | 1077.057 | 799.752 |
| 3 | 1013.248 | 790.425 |
| 4 | 1016.793 | 744.064 |
| 5 | 1053.309 | 762.608 |
| Median, measured rounds | 1045.085 | 762.608 |

Raw output is in `/tmp/ll-background-day-benchmark.log`. These measurements compare the retained synchronous API with the background API in the final implementation, rather than an older build. They are specific to this co-located fixture. Frozen forecasts can choose different plans from fresh actual-state planning; the resulting universes are not expected to match exactly. A large jump provides no wall-clock lead time between events, and background planning does not guarantee faster day jumps or bound expensive individual goal searches.
