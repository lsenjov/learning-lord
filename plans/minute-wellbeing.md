# Minute-sampled planning wellbeing

## Agreed behaviour

- Sample prediction wellbeing every simulation minute across action and goal boundaries.
- Weight samples by represented duration; score a final partial minute at its endpoint without retaining it as an extra sample when extending the plan.
- Use endpoint state including completion effects. Compare goal variants and full plans using duration-weighted sampled wellbeing.
- Preserve execution, horizons and cooldowns. Update the decision display.
- Revisit sampling accuracy and planning performance in future; endpoint sampling can attribute a completion benefit up to one minute early.

## Implementation step

1. Implement sampling and regression coverage, update documentation, run checks and independent review, resolve findings and commit.

## Completed

- Added duration-weighted endpoint sampling with a shared minute schedule across actions and goals. Goal comparisons accumulate only their own interval; full-plan comparisons carry the existing accumulator forward.
- Final partial samples are provisional so extending a plan does not reward extra boundaries. Updated decision text and current design documentation.
- Added regressions for split-action equivalence, partial-minute weights, goal continuity and completion effects; updated independent plan-score and fixed-price checks.
- Validation: all 97 workspace tests, formatting, Clippy with warnings denied, build and diff checks passed. Independent review reported no findings.
- Four fixed-map debug timing cases (100 plans each) averaged 2.8–14.9 ms per plan. The previously oscillating reproduction now selects repeated gathering at one site. Revisit accuracy and broader performance as the simulation grows.
