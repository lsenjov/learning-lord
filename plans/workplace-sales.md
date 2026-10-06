# Workplace sales and shopping trips

Sell orders store their physical location. Citizens with an owned field or workshop list and withdraw every good there; other citizens use the public market. Customers can visit workshops while homes remain private and production requires ownership.

The API, planner and desktop changes form one implementation milestone, committed together after validation and review.

- Add order locations and local settlement, keeping town-wide price and activity calculations. Add concrete purchase actions and selling-place selection.
- Route each shopping basket through the nearest useful locations, including travel and five minutes per stop in prediction. Treat consecutive stops as one bounded purchase sequence and start the buying cooldown after its last purchase. Update requests, stale-plan validation, and display locations.
- Add regressions for locality, access, settlement, shopping trips, cooldowns and changed stock. Run formatting, workspace tests and strict Clippy; benchmark the larger town. Review and fix all high and medium findings before the milestone is committed.

Purchases reserve no stock. Fixed planned stops buy only local stock still available when they complete, using existing partial settlement and replanning behavior. Supplier choice uses one nearest-location route rather than exploring combinations.

## Validation

- Workspace tests: 279 passed, two manual benchmarks ignored. Formatting and strict Clippy passed.
- Regression coverage checks local settlement, own stock exclusion, private workplace sales, incidental goods, customer access, production ownership, multistop routing, cooldowns, travel horizons, partial fills, requests and history.
- Independent review found a medium issue in demand reporting beyond the execution commitment. The request helper now includes only stops that can begin before replanning; a regression covers that boundary. Re-review reports no high or medium issues.
- Native desktop smoke advanced a 24-citizen town to day 1, producing 104 trades. The market directory displayed flour at Bram's, Hugo's and Niko's mills and wood at the public Market.
- A preliminary functional release smoke reached five days, with 732 trades (625 from private workplaces), no errors and 7,200 total coins conserved. Timing from that run is excluded because builds and tests ran concurrently.
- Final release benchmark used the existing deterministic spatial-town fixture with 24 citizens, 12 planning workers and a 120-second per-day cap. All 14 days completed in 124.10 seconds of simulation processing (41 ms startup), versus the earlier 106.9-second run: approximately 16% longer. Day 14 took 8.62 seconds. The run ended with 1,415 trades, 1,181 from private workplaces, 536 open orders and 7,200 total coins; maximum hunger remained zero at each daily sample. No builds, tests or this task's desktop instance ran during timed simulation; the user's pre-existing desktop instance remained open. This is one comparison with different economic trajectories, not a measurement isolating routing cost. Peak memory was not measured.
