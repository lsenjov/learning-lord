# Daily prices and universe restart

- Randomize starting prices and update at 04:00 each simulation day: independent uniform berries 1–2 coins/kg, pebbles 1–4 coins/kg.
- Universe owns market randomness and current prices. Citizen snapshots carry the current price context for standalone advancement and predictions; insertion and universe updates synchronize it.
- Predictions hold prices fixed. Existing plans continue until normal replanning; active trades retain their starting quote. Actions starting exactly at an update use the new prices.
- Find rocks yields uniform 2.5–7.5 grams, with 5 grams used in predictions; forage remains 5–15 grams, averaging 10.
- Add Restart universe, creating a new paused universe at 1x with Ada and fresh starting prices. Clear errors and elapsed pacing. Display current prices.

## Implementation step

1. Implement price state, deterministic daily updates across time boundaries, reduced pebble yields, restart controls, documentation and tests. Verify price bounds, branching, quote timing, prediction isolation, reset behaviour and changed planning choices. Run checks, obtain independent review, fix findings, and commit.

## Progress

- Plan recorded before implementation.
- Implemented randomized starting/daily prices, boundary-aware advancement and repricing, half pebble yields, price display, and restart with acknowledgment of the new universe.
- All 83 workspace tests pass. Formatting, Clippy with warnings denied, workspace build, and diff whitespace checks pass.
- Independent review using code-review-skill found no high, medium, or low issues.
- Checked the native app visually; prices, restart controls, and the plan display fit. Replaced an unsupported font separator with ASCII.
