# Action planning and committed execution

## Agreed scope

- Explore eat/wait sequences until a completed action reaches or crosses four simulation hours. The current 30-minute actions yield eight actions per sequence and 256 candidates.
- Rank sequences by average personal wellbeing at action completion. Any optimal tied sequence is acceptable.
- Commit to two simulation hours. Finish any action crossing that time boundary before replanning.
- Predict using independent citizen snapshots; current actions have no interactions with the rest of the universe.
- Keep search and batch execution in a separate planning module, reusing citizen action and need calculations.
- Enable automatic planning explicitly with `start_planning()` on an idle citizen, agent, or universe agent. Ordinary unplanned citizens retain manual action control.
- Once enabled, continue queued actions and future batches through `advance()`, including across action and batch boundaries within one tick.
- Preserve original snapshots and return numeric errors without partially applying changes.
- Leave threads, pruning, variable horizons, world interactions, frontend controls, and time-integrated scoring for later.

## Implementation step

1. Implement pure plan search, inspectable predictions, and committed batch execution with citizen/agent/universe delegation. Test optimality, scoring, tie handling, two-hour commitment, replanning, tick boundaries, numeric errors, and snapshot isolation. Update documentation, run workspace tests/build, formatting and Clippy, obtain an independent review, resolve high/medium findings and low documentation findings, and commit the completed step.

## Progress

- Plan recorded before implementation.
- Complete: added pure four-hour search, inspectable plans, and execution of committed two-hour batches in `planning.rs`.
- Added opt-in planning on citizens, agents, and universe snapshots. Advances continue across action and batch boundaries while preserving their source, including on errors.
- Updated README and design notes. Eight new tests cover exhaustive optimality, completion-state averages, deterministic ties, commitment/replanning, tick equivalence, errors, and independent branches.
- Validation passed: all 31 tests, workspace build, formatting, Clippy with warnings denied, and diff checks.
- Independent review found no high, medium, or low issues.
- The walkthrough identified duration-based horizons, tie-test changes, and the `ActivePlan` naming correction. Follow-up implementation is tracked in [planning-followup.md](planning-followup.md).
