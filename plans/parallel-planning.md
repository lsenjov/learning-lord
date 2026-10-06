# Parallel citizen planning

## Plan

1. Evaluate each starting goal in a bounded, process-wide worker pool, preserving ordered reduction and the sequential search as a test reference. Validate identical plans and diagnostics, document thread architecture, and benchmark the reconstructed Finn fixture. Commit after independent review and required checks.

## Architecture

Only starting goals for one citizen run concurrently. Each task evaluates every variant and continuation with its own snapshot and continuation cache. Citizens and execution remain sequential. The lazy Rayon pool uses max(logical CPUs minus four, two) workers; CPU detection failure uses two. Pool initialization failure falls back to sequential evaluation. Rayon propagates task panics to the caller. Ordered collection waits for every task before selecting the first error or reducing scores and diagnostics in the original goal order.

At most seven goals can occupy workers. Separate caches can duplicate continuation work, so speedup is limited by goal imbalance and duplicated search. Cold pool initialization and warm planning timings must be reported separately.

## Validation

The new tests compare exact `Plan` equality against the original shared-cache sequential planner: actions, durations, goal ranges, score, candidate order, production diagnostics, and prices. Fixtures cover hungry citizens, purchased food and clothing, production, garment listing versus wearing, and equal-score ordering. Repeated calls preserve citizen snapshots, market snapshots, and RNG state. Busy-citizen and prediction-overflow failures agree with the reference. Thread-count tests cover detection failure and one through sixteen logical CPUs. Independent Sol-6.1 review reported no high, medium, or low issues. All 238 workspace tests pass (235 existing plus three added); the strengthened equivalence fixture was also rerun after adding production materials and exact score ties. Strict workspace Clippy (`--all-targets -- -D warnings`), formatting, and diff whitespace checks pass.

## Benchmark

Reconstructed Finn from `debug/universe-1791287397.611676701.txt`, using only the Finn fixture section of `/tmp/ll-planning-diagnostic.rs`: actual tailoring skill 24.6, 17 carried garments, exported map, prices, inventories and 118 market orders. The benchmark temporarily appended that reconstruction and a test harness to the current planner, restoring source afterward. `/tmp/ll-parallel-benchmark.py` and `/tmp/ll-parallel-benchmark.log` retain the harness and raw results. Both modes used the same optimized simulation development build and hardware. The original sequential planner retained its shared continuation cache. Each result matched the complete sequential Plan exactly.

Rust detected 16 logical CPUs; the pool contains 12 workers. Cold pool initialization took 0.425 ms. After one warm-up of each mode, five alternating measurements (one test thread to avoid competing benchmark tests) gave:

| Mode | Median | Range | Search calls |
| --- | --- | --- | --- |
| Original sequential, shared cache | 1.273 s | 1.268–1.277 s | 13,335 |
| Parallel roots, separate caches | 0.613 s | 0.605–0.650 s | 13,335 |

This fixture improved by approximately 2.08×. The measured fixture showed no additional continuation search calls from separate caches; other states can duplicate work. At most seven tasks can run, goal runtimes vary, and this is one reconstructed citizen rather than a universal speedup claim. Cold initialization was measured separately from warm planning.
