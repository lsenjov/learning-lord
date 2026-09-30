# Fixed-price trading

- Unlimited marketplace, using existing prices.
- Sell pebbles sells all held pebbles. Buy berries spends available positive coins to top up to 100 grams, buying less when funds are insufficient. No borrowing.
- Each trade takes five minutes. Quantities and payment are fixed from actual state at action start; transfer occurs at completion. Needs continue growing.
- Predictions use the same trade rules. Empty trades are skipped. Preserve food replan checks and meal spacing.

## Implementation step

1. Add trading actions, execution, predictions, UI labels and documentation. Verify transfers, conservation, partial purchases, snapshot isolation, planning and performance. Run checks, obtain independent review, resolve findings, and commit.

## Progress

- Plan recorded before implementation.

- Superseded during implementation by `plans/goal-planning.md`; trading and goal planning are completed and reviewed together.
