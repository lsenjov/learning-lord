# Sleep and continuous action effects

## Agreed behaviour

- Tiredness increases by 100 per 24 simulation hours, including during sleep.
- Negative tiredness represents being rested, with a floor of -100 and no upper cap.
- Sleep takes eight hours and continuously removes 100 tiredness over its duration.
- Eat takes 30 minutes and continuously removes 50 hunger over its duration.
- Hunger and tiredness continue growing during every action. Completion gives no additional recovery.
- Personal wellbeing adds `-max(tiredness, 0)` to the existing hunger score.
- Retain average wellbeing at action completions, the four-hour prediction horizon, and the two-hour commitment. Finish actions crossing either boundary; sleep is not interrupted.
- Preserve immutable snapshots and numeric error handling. Interruption APIs are out of scope.
- Existing constructors start tiredness at zero; add `Citizen::with_needs(hunger, tiredness)` for explicit starting needs. Clamp finite starting tiredness at -100.
- Show tiredness and Sleep in the desktop, with an accurate countdown until replanning after a long action.

## Implementation step

1. Add tiredness, continuous need advancement, action-specific durations, Sleep planning, and desktop readouts. Update affected tests and documentation; verify partial recovery, the floor, daily balance, immutable/error behaviour, exhaustive planning, long-action boundaries, and equivalent small/large advances. Run workspace tests, formatting, Clippy and build; obtain independent review, resolve high/medium issues and low documentation issues, and commit the completed step.

## Progress

- Completed tiredness, its floor and wellbeing contribution, eight-hour Sleep, continuous meal/sleep recovery, Sleep planning, and desktop readouts/countdowns.
- All 52 workspace tests pass, including exhaustive comparison of all 511 plan candidates, daily balance, partial recovery, numeric failures, immutable snapshots, and long-action boundaries.
- Formatting, Clippy with warnings denied, workspace build, and diff checks pass. Visually verified the native desktop's tiredness and action/plan readouts.
- Independent review found no high, medium, or low issues and independently reran all 39 simulation tests.
