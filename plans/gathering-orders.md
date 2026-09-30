# Gathering planning orders

## Step 1

Represent gathering candidates as orders containing one through the planning horizon divided by the primitive duration. Predict and execute each existing primitive separately, with one automatic trip per order. Preserve the last gathering family across goal and resource searches; a different gathering family or non-gathering activity permits another order. Keep persisted plans as primitive action sequences.

Validate duration-derived bounds, wealth order selection, resource supply and surplus, adjacency across goal boundaries and nested preparation, travel and score equivalence, cooldowns, fresh planning state, and commitment replanning. Run simulation tests and workspace checks, then obtain a fresh Sol-6.1 review and resolve high or medium findings before committing this step.

## Validation

- Workspace tests: 107 passed (17 desktop, 31 simulation unit, 59 simulation integration); no failures.
- Formatting check, workspace Clippy with warnings denied, workspace build, and whitespace diff check passed.
- Fresh Sol-6.1 review approved the implementation with no high, medium or low findings.
- Four fixed-map debug timing cases (100 plans each) averaged approximately 0.9–3.2 ms per plan.
- Implementation step completed after verification and review.

## Later review

Examine whether duration-averaged wellbeing favors shorter orders or goals that omit later low wellbeing. This change retains the existing scoring rule.
