# Action endpoint wellbeing scoring

## Agreed behaviour

- Replace minute sampling with each action's average starting and ending wellbeing, weighted by its duration.
- Goal variants use their own actions; full plans accumulate all actions. Completion effects are included in endpoint wellbeing.
- Keep predicted and actual inventory transfers, needs, travel, horizons, cooldowns and execution unchanged.
- Update decision text and design documentation. Retain a note to revisit the approximation: wellbeing may change nonlinearly within actions and goods arrive at completion.

## Implementation step

1. Replace the scoring accumulator, cover duration weighting and the exported detour, update docs, run checks and an independent review, fix findings and commit.

## Validation

- All 102 workspace tests passed, including duration weighting, linear travel split equivalence, goal/full-plan accumulation, finite extreme scores and the exported market-state regression. Formatting, Clippy with warnings denied, build and diff checks passed.
- Reconstructing the exported planning state now produces market → sell → buy → eat → river → repeated rock gathering, eliminating the forest detour.
- Four fixed-map debug timing cases (100 plans each) averaged approximately 1.1–3.5 ms per plan.
- Independent Sol-6.1 review approved the change with no high, medium or low findings. Implementation step completed.
