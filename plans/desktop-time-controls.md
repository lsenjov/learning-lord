# Desktop time controls

## Agreed scope

- Start paused with one citizen at hunger zero and planning enabled.
- Add Run/Pause, speed choices 1x/2x/3x/5x/10x/20x, and an exact 30-minute step while paused.
- At 1x, each real second advances simulation time by 60,000 milliseconds.
- Measure real elapsed time with a monotonic clock, including simulation processing time. Slow updates produce larger subsequent advances rather than a queue of fixed catch-up ticks.
- Cap automatic update starts at a configurable rate, default 60 per real second. Expose `LEARNING_LORD_MAX_UPDATES_PER_SECOND` so 30 or another positive integer can be selected at launch.
- Preserve fractional milliseconds. Account for speed and pause commands at their input timestamps; paused time does not accrue.
- Use one worker with one advance in progress. Pause allows an in-progress update to finish and prevents further automatic steps. Publish only the latest completed snapshot to the UI.
- Display `Day 0 | 00:00:00`, hunger, wellbeing, current action, remaining action time, the rest of the planned actions in order, and time until replanning.

## Implementation step

1. Implement and test the real-time pacing and single-worker lifecycle; add Bevy controls and readouts; update run documentation. Verify fractional timing, update caps, delayed processing, pause/resume, speed changes, exact manual stepping, and clock formatting. Run workspace tests/build, formatting and Clippy, exercise the desktop controls, obtain an independent review, resolve high/medium findings and low documentation findings, and commit the completed step.

## Progress

- Completed the worker, pacing, controls, clock, citizen readouts, remaining planned actions, and replanning countdown.
- All 45 workspace tests pass, including headless Bevy button-to-worker/UI checks; formatting, Clippy with warnings denied, build, and diff checks pass.
- Visually verified the native desktop at a configured 30 updates per second, including the complete planned-action readout. OS permission prevented synthetic mouse checks; in-process UI checks exercised speed selection, manual stepping, and run/pause instead.
- Independent review found no remaining high, medium, or low issues.
