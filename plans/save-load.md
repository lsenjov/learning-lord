# Save and load universes

- Replace the debug export control with Save and Load.
- Save timestamped, versioned Serde/RON files compressed with gzip (`.ron.gz`) in `saves/`; present saved files newest first in an in-app load window.
- Preserve the full universe, including active actions/plans, inventories, random generator state, market and citizen history, crops, storage, taxes, and fractional obligations.
- Loading pauses at the saved simulation time. Reset pacing, background planning jobs and stale UI selections; preserve committed simulation plans.
- Keep the current universe if a file cannot be loaded. Report unsupported versions and malformed files clearly. Existing debug text exports are not save files.
- Keep runtime threads and UI window layout outside the save format. Background planning diagnostics may restart; persisted simulation history remains.
- Retain 30 days of individual trades and tax receipts, the existing 30 completed days of citizen/planning summaries, and one year (365 days) of daily market summaries. Prune during normal simulation at the daily boundary, after transactions have been accounted for. Never prune outstanding obligations or active orders as history.
- Defer shared-state deduplication: serialize citizens' cached market, map and storage snapshots normally for now. Existing exports compressed by about 97% in a gzip trial. Revisit saving shared state once if file size or save/load time becomes a problem.

## Steps

1. Add simulation serialization, history retention and a versioned save format, with round-trip, retention and resumed-action coverage. Independently review, validate, and commit.
2. Replace Export with Save/Load, add compressed timestamped file storage and a saved-universe list, and safely replace the worker universe on load. Test failures, cancellation and UI behavior; independently review, validate, and commit.

## Progress

- Step 1 complete: full-state Serde/RON snapshots preserve active plans and gathering random state; version and state checks reject malformed saves. History retention runs after transaction accounting at 04:00. Simulation unit/integration tests, strict clippy and formatting passed; independent review found no remaining issues.
- Step 2 complete: Save writes unique compressed files; Load lists them in a floating window and restores a paused universe with fresh runtime planning state. Corrupt/truncated gzip, malformed saves, interrupted replacement and stale mutations are covered. Independent desktop review found no issues. All 502 workspace tests passed (four manual probes ignored), strict scoped clippy and formatting passed, and the locked workspace build and a bounded `cargo run --locked` launch succeeded.
