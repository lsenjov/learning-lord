# Production decision visibility

Show recipe diagnostics alongside the selected agent's saved planning decision. Capture the decision-time profit per hour, remaining target and carried/missing batch inputs. Explain whether a sequence was observed, whether a complete competing plan scored lower, and whether the recipe appears later in the selected forecast. Distinguish untested recipes from infeasible ones; report only exclusion reasons supported by the search evidence.

## Implementation milestone

1. Capture bounded diagnostic evidence during the existing search, present it using the existing agent detail layout, and add coverage for selected, feasible alternative, blocked and untested recipes. Preserve search choices, scoring and performance; do not rerun planning to populate the display. Run relevant tests, workspace checks and a bounded town scenario, then obtain independent Sol-6.1 review and resolve findings before committing.

Market history charts are being discussed separately and are not authorized for implementation in this milestone.

Implementation complete. Diagnostics capture bounded recipe snapshots and evidence from starting-goal sequences and their completed continuations without extra search branches. Inputs are labelled per batch and as decision-time values. Recipes in later goals count as observed; a lower-score explanation requires a completed competing forecast.

Validation: all 168 simulation tests and 30 desktop tests pass. Strict workspace Clippy, formatting and diff checks pass. Independent Sol-6.1 review found one medium issue in later-goal evidence; it was fixed with regression coverage and re-reviewed with no remaining findings.

Known validation limitation: the randomized three-day town test failed with `WealthOverflow` (including steps 62, 113 and 114). An isolated unchanged HEAD `22999de` reproduced the same failure at step 52; another baseline run passed in 7.08 seconds. This existing simulation failure remains unresolved. No deadlines, search policies or tests were relaxed to hide it.
