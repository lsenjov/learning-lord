# Need eligibility and waiting

One milestone: restrict eating, drinking, and sleeping goals to positive needs at their predicted start; add a scored two-hour Wait goal using existing half-hour actions, available first or second. Observe actual execution of Wait separately from forecasts using a simulation probe.

Validate boundary eligibility, need progression during waiting, second-goal eligibility after waiting, and planner fallback. Run simulation tests and workspace checks; obtain an independent review, resolve valid high/medium findings and re-review, then commit the milestone.

Diagnostic outcome: one simulated day from the current fresh ten-citizen desktop town produced one contiguous two-hour Wait execution each for Ada, Dara, Gita and Hale. Starts occurred 10.662–11.221 hours into the run while their personal needs were negative. Production was feasible but scored below Wait. These observations count actual execution, excluding second-goal forecasts. The latest available saved town is version 8 and cannot load under current version 10; no save migration was performed. Temporary diagnostic code was removed.

Final validation: `cargo test --workspace --quiet` passed (699 passed, 0 failed, 7 ignored across unit, integration and documentation suites); `cargo clippy --workspace --all-targets --quiet -- -D warnings`, `cargo fmt --all --check`, and `git diff --check` passed. Independent review and the final fresh review reported no findings.
