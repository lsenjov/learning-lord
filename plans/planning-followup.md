# Planning walkthrough follow-up

## Agreed changes

- Stop prediction after the completed action that reaches or crosses four simulation hours.
- Commit to two simulation hours, finishing any action in progress at that boundary before replanning.
- Store a variable-length action sequence and average the actual number of completion scores.
- Test that a chosen plan is optimal without requiring the first equally good sequence.
- Rename `PlanExecution` to `ActivePlan` and `plan_execution` fields/getters to `active_plan` throughout code, tests, and current documentation.
- Preserve the existing 30-minute eating/waiting durations, endpoint scoring, immutable snapshots, and error behavior.

## Implementation step

1. Update search and batch execution to track durations, apply the naming changes, and adjust optimality tests. Add boundary-crossing coverage without changing gameplay action durations. Update documentation, run workspace tests/build, formatting, and Clippy, obtain an independent review, resolve high/medium findings and low documentation findings, and commit the completed step.

## Progress

- Plan recorded before implementation.
- Complete: prediction uses a four-hour remaining-duration budget and a variable-length action list; scores average the actual completion count without overflowing a sum.
- `ActivePlan` tracks elapsed execution time, including partial actions, and replans at the first action completion at or after two hours. Renamed the citizen field/getter to `active_plan` and updated documentation.
- Tie tests now verify the selected sequence's optimal score instead of prescribing a winner. Added private boundary fixtures for prediction overrun and a delayed action crossing the commitment boundary; gameplay durations remain unchanged.
- Validation passed: all 33 tests, workspace build, formatting, Clippy with warnings denied, and diff checks.
- Independent review found no high, medium, or low issues.
