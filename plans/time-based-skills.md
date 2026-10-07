# Time-based production skills

Implement qualified skills with integer milliseconds of actual matching production practice. Starting professions are qualified at zero practice; absent skills remain unavailable. Accumulate partial work, including forecasts, capped at 4,368 hours (52 weeks of twelve-hour days). Travel and other actions earn no practice. Existing actions retain their chosen durations.

For progress p = clamp(practice hours / 4,368, 0, 1), duration reduction is 0.5 * (2p - p²). New jobs take ceil(base duration * (1 - reduction)); core prices retain base durations.

## Implementation step

Replace levels and completion gains with elapsed practice, adapt fixtures and focused tests, and show practice hours and duration reduction in expanded citizen details. Update current documentation. Run formatting, simulation/workspace tests and clippy, then obtain independent review and fix findings before committing this cohesive step.

## Validation

Completed the implementation step. `cargo test --workspace` passed (two manual benchmarks remain ignored). Final focused production tests and the forecast/actual replay test passed after the last coverage additions. `cargo clippy --workspace --all-targets -- -D warnings`, formatting, and diff whitespace checks passed. Independent review found no high, medium, or low issues.
