# Action planning and committed execution

## Agreed scope

- Explore all eight-action eat/wait sequences over four simulation hours (256 sequences).
- Rank sequences by average personal wellbeing at action completion. Keep the first sequence found on ties.
- Commit to the first four actions, covering two hours. Replan only when that batch finishes.
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
