# Daily production stock floor

Replace the one-batch exploratory fallback with a minimum stock target for each profitable output: whole batches producible in twelve hours at current skill. Demand can increase that target. Keep saleable stock and in-progress deductions, personal reserves, profit ordering, and independent twelve-hour alternatives evaluated against current inputs and funds. Use the maximum ingredient requirement per good across alternatives, plus separately committed underway inputs; reserve personal food afterwards and deduct carried/listed stock from purchase demand.

## Step

1. Update target calculation, document current policy, and verify no-demand/low-demand floors, skill adjustment, carried/listed/in-progress stock, independent goods without competing for work, inputs or funds, demand above the floor, and unprofitable recipes. Run simulation checks and a bounded town/performance smoke test; obtain independent Sol-6.1 review and resolve high/medium findings before committing.

## Validation

- The full simulation suite passes. All 15 production-target tests, simulation Clippy, formatting, and diff checks pass.
- Initial independent Sol-6.1 review found no remaining logic or documentation issues and verified both added coverage regressions, but identified a medium performance regression.
- Before the planning changes, the three-day town acceptance test exceeded a 60-second bound. A separate 30-second diagnostic found initial planning at 30–107 ms, then later half-hour advances taking 13.89 s (step 64), 3.96 s (step 68), and 2.33 s (step 69), before timing out inside step 72. All temporary instrumentation was removed.
- The user then authorized unique top-level goals and ordered production prefixes, recorded in [unique-planning-goals.md](unique-planning-goals.md). With those changes, all 180 workspace tests pass and the standalone town test completes in 9.95 seconds. A fresh independent Sol-6.1 review found no issues. The stock floor and planning changes are complete together; random town states can still vary in planning cost.
