# Farming milestone

1. Add desktop action compatibility and location plots, timestamp based crop lifecycle, explicit crop work, ownership, standing crop wealth, harvest taxes, and deterministic planning using town consumption demand and future labor. Validate lifecycle, prediction, background execution, and multi-day town behavior.
2. Show field plots in Locations; update starting bread, document farming, and validate the interface.

Each completed step requires independent review and a commit by the coordinating agent.

## Validation

- Simulation unit and integration tests cover lifecycle deadlines, elapsed jumps, prediction, demand ratios, workload capacity, individual and multiple-field ownership, harvest history, socage reservations, and standing crops excluded from weekly asset taxes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Manual 24-citizen spatial town probe over ten days preserves every citizen's map snapshot. Planting began on day five, the first harvest arrived on day nine, and total crop output reached 16,800g on day ten. Debug simulation took 9.25 seconds for the ten days in that run. Crop choices and planting dates depend on the town's needs and market conditions.
- Independent reviews identified and resolved timestamp overflow, crop history accounting, and overlapping future labor reservations.

Step one is complete: lifecycle and planning implementation, regression tests, and independent review are finished, committed as `339bcf9`.

Step two is complete: Locations shows all twelve plots with crop status, expected yield, tending state and windows, and harvest readiness. Citizen current actions and plans identify the crop work, field and plot. Desktop citizens start with fourteen bread loaves (700 nutrition); README documents crop work, standing value, demand and taxes.

- All 100 desktop tests pass. The existing town production and market transaction test now runs ten days, covering the initial seven-day food reserve and later food trades while retaining every production and trade assertion.
- Desktop clippy and workspace formatting checks pass.
- Native Bevy screenshots were inspected at a 1,100 by 620 logical window size: `/tmp/ll-crops-native-0.png` shows growing, ready, dead and empty plots; `/tmp/ll-crops-native-1.png` shows the remaining plots through twelve plus stock, transfers and taxes; `/tmp/ll-crops-native-2.png` shows crop action labels in the citizen sidebar and details, plus starting food. The isolated visual harness was stopped. Unsupported separators in the new plot text were replaced with ASCII characters.
