# Construction context persistence

- [x] Implement, independently review, validate, and commit the construction context persistence fix as one cohesive work unit.

Stop persisting citizen construction escrow, protected funds, access, and wood reserves. Accept and ignore legacy cache fields. Validate authoritative map, market, stock, accounts, and construction state before rebuilding context, then validate access-dependent actions and reservations. Rebuild every worker's access before calculating procurement protection.

Refresh before completed action settlement, before starting or resuming citizen planning, before workplace supply preparation, and before background planner snapshots. Preserve returned universe freshness for save round trips. Keep refreshes out of incomplete action settlement iterations.

Readers: escrow contributes to wealth; protected cash limits purchasing; access controls places and storage (including load validation); wood reserve controls reserve planning. Construction processing refreshed before caravan price and stock changes, leaving protection stale when citizens spent or planned. Citizen actions can also change stock available to later citizens.

Validation: legacy cache omission and wrong-field compatibility, active construction action load round trips, invalid underlying escrow and household balances, access rebuild fixed points, stock-change action boundaries, full simulation tests, and the latest actual day 31 save through the normal loader followed by 24 hours of planner advancement and another load round trip. Independent review is required before commit.

Implemented and independently reviewed. The loader review found that malformed market inputs could reach procurement aggregation before validation; source validation now runs first. Fresh review found no remaining issues. End-of-event refresh remains because background planning can begin before an action completes, and returned context must survive save round trips.

Validation completed: 455 unit tests and 187 integration tests pass with the existing clock-overflow test explicitly skipped (6 unit tests ignored). The unskipped suite reproduces that existing overflow at construction.rs:515. The latest actual day 31 save loads through the normal loader, round trips unchanged, advances 24 hours with the planner, and round trips unchanged again. The overlapped smoke timing is not a controlled performance measurement. An additional malformed-market regression passes separately, bringing the total to 643 passing tests.
