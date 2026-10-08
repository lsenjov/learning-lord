# Farming milestone

1. Add desktop action compatibility and location plots, timestamp based crop lifecycle, explicit crop work, ownership, standing crop wealth, harvest taxes, and deterministic planning using town consumption demand and future labor. Validate lifecycle, prediction, background execution, and multi-day town behavior.
2. Show field plots in Locations; update starting bread, document farming, and validate the interface.

Each completed step requires independent review and a commit by the coordinating agent.

## Validation

- Simulation unit and integration tests cover lifecycle deadlines, elapsed jumps, prediction, demand ratios, workload capacity, individual and multiple-field ownership, harvest history, socage reservations, and standing crops excluded from weekly asset taxes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Manual 24-citizen spatial town probe over ten days preserves every citizen's map snapshot. Planting began on day five, the first harvest arrived on day nine, and total crop output reached 16,800g on day ten. Debug simulation took 9.25 seconds for the ten days in that run. Crop choices and planting dates depend on the town's needs and market conditions.
- Independent reviews identified and resolved timestamp overflow, crop history accounting, and overlapping future labor reservations.

Step one is complete: lifecycle and planning implementation, regression tests, and independent review are finished. The coordinating agent will commit it before the desktop step.
