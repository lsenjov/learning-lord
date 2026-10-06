# Larger town

One cohesive implementation step:

- Expand the starting town from six to 24 citizens, preserving the original six and creating four of each profession with the existing supplies, needs, skills, homes, and workplaces.
- Keep the existing map area and shared public places; make the roster usable for all citizens.
- Verify role counts, ownership, starting states, selection, workspace tests, formatting, strict Clippy, and native UI.
- Compare six and 24 citizens using the same release build and deterministic spatial fixture, recording startup, daily advance times, worker count, hardware concurrency, and memory where practical. Bound slow runs and report limitations.
- Obtain independent code review, resolve high/medium findings, and commit the completed step.

## Results

Implemented 24 citizens (four of each profession), with distinct names, homes and required workplaces. The originals retain their starting needs, clothing, food, coins and skills. The town retains the 2 km square map, 200 m minimum place spacing, and three shared public places. The roster has four pages of six citizens with page navigation and page-local number shortcuts.

Verification: 273 tests passed, two manual benchmarks ignored; formatting and strict workspace Clippy passed. Native UI smoke selected Yara on page four and opened her daily history. Screenshot: `/tmp/larger-town-page4.png`. Independent review identified a benchmark setup mismatch in home names; this was fixed and map counts and all home positions are now checked before timing. Preliminary measurements from the faulty fixture were discarded.

### Performance method

Release simulation test binary, sequential runs with no desktop simulation running. AMD Ryzen AI 7 350, 16 logical CPUs, 12 planning workers. A fixed 250 m grid gives each agent its own home and workplace, with the first six using identical coordinates in both scenarios; forage seeds and agent IDs are fixed. Place UUIDs remain randomly generated, so exact tie outcomes can vary. Each run advances 14 consecutive simulated days from 06:00 using `PlanningRuntime`, bounded at 120 seconds per day. Python subprocess wall time and Linux `RUSAGE_CHILDREN.ru_maxrss` measure process time and peak resident memory. These figures exclude the Bevy renderer and do not measure UI frame rates.

Six-citizen run: startup 25.011 ms; 14 days completed in 12.183 seconds; peak RSS 65,076 KiB. Day 1: 433.134 ms; day 3: 351.619 ms; day 8: 774.252 ms; day 10: 665.915 ms; day 14: 2,197.730 ms. All needs remained finite and coins stayed at 1,800. Final cumulative trades: 341; live orders: 140.

24-citizen run: startup 91.104 ms; 14 days completed in 106.870 seconds; peak RSS 89,232 KiB. Day 1: 1,627.306 ms; day 3: 3,070.894 ms; day 8: 6,954.349 ms; day 10: 6,216.781 ms; day 14: 21,191.852 ms. All needs remained finite, boundary hunger never exceeded zero beyond floating-point rounding, and coins stayed at 7,200. Final cumulative trades: 1,440; live orders: 506. Neither scenario hit the cancellation cap.

The fourfold population increase took 8.8 times as long across 14 days, while peak simulation-process memory grew from 63.6 MiB to 87.1 MiB. Performance varies by day and slows as the larger town develops. The single comparison is a useful workload measurement, not a statistical benchmark or a guarantee for arbitrary random towns. The growing number of live orders accompanies the slowdown; identifying its precise cost requires profiling. No planner optimization was added.

Raw logs: `/tmp/larger-town-benchmark6.log` and `/tmp/larger-town-benchmark24.log`.

To rerun either scenario, set `TOWN_BENCH_CITIZENS` to `6` or `24`:

```sh
TOWN_BENCH_CITIZENS=24 TOWN_BENCH_DAYS=14 cargo test -p learning-lord-simulation --release --locked larger_spatial_town_benchmark -- --ignored --nocapture --test-threads=1
```

Final UI review also found a rotated travel line escaping the map viewport into the footer. The route now uses bounded, unrotated dots. Focused map and citizen tests, strict desktop Clippy and a refreshed native screenshot verified the fix.

| Simulated day | Six citizens (s) | 24 citizens (s) |
| --- | ---: | ---: |
| 1 | 0.433 | 1.627 |
| 2 | 0.465 | 2.745 |
| 3 | 0.352 | 3.071 |
| 4 | 0.470 | 6.134 |
| 5 | 0.774 | 5.087 |
| 6 | 0.812 | 7.823 |
| 7 | 1.410 | 8.478 |
| 8 | 0.774 | 6.954 |
| 9 | 0.545 | 13.679 |
| 10 | 0.666 | 6.217 |
| 11 | 1.539 | 7.160 |
| 12 | 0.929 | 6.930 |
| 13 | 0.786 | 9.678 |
| 14 | 2.198 | 21.192 |
