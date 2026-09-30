# Berry inventory and foraging

## Agreed behaviour

- Citizens start with zero berries. Inventory quantities are grams stored as `f64`; berries are the only good for now.
- Berries provide 0.5 nutrition per gram and take one second per gram to eat.
- Eat selects up to 50 nutrition from current inventory when it starts. Smaller portions are allowed; an empty Eat is skipped. Eating consumes berries and restores hunger continuously while both needs continue growing.
- Forage takes 30 minutes and adds a uniform random yield of 5–15 grams on completion. Prediction uses exactly 10 grams without consuming randomness.
- Before a non-first Eat, less than 20 available nutrition triggers replanning from actual state. The first action is exempt. Prediction excludes actions that would already trigger this check.
- Retain completion-based average wellbeing, the four-hour prediction horizon, and the two-hour execution commitment with action completion before normal replanning.
- Random generator state belongs to each citizen snapshot, preserving independent branches and reproducible advancement from the same snapshot.
- Within each prediction, Eat becomes eligible again four simulation hours after the previous predicted meal completes. Every new plan resets eligibility, regardless of actual recent eating. This permits one meal in the current four-hour horizon and repeated meals in longer predictions.
- Keep the exhaustive search simple; the agreed meal spacing replaces the temporary pruning and state-caching optimizations.
- Round positive eating durations up to the nearest millisecond; meals retain their chosen quantity once started.
- Display berry grams, Forage, upcoming actions, and the commitment countdown. Future meal durations depend on forage outcomes, so the countdown describes the commitment rather than claiming an exact future replanning time.

## Implementation step

1. Implement berry inventory, continuous portioned eating, snapshot-owned forage randomness, average-yield prediction, the replan check, and prediction-local meal spacing. Update the desktop, tests, and current documentation. Verify conservation, partial and empty meals, completion-only yield, prediction isolation, random bounds, first-action exemption, threshold boundaries, repeated replanning, meal-spacing expiry and reset, longer predictions, stocked-citizen performance, immutable failures, and equivalent tick partitioning. Run tests, formatting, Clippy and build, obtain independent review, resolve findings, and commit.

## Progress

- Plan recorded before implementation.
- Implementation complete, including desktop readouts and current documentation.
- All 64 workspace tests pass; formatting, Clippy with warnings denied, workspace build, and diff whitespace checks pass.
- Previously slow stocked-citizen cases complete in approximately 1–2 ms in the local debug benchmark with the meal-spacing rule and simple exhaustive search.
- Independent final review found no high or medium issues. Fixed its low documentation finding by qualifying the half-meal example as a full 100-gram meal.
