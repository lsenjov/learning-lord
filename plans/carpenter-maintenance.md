# Carpenter capacity, construction portions, and maintenance

## Agreed behaviour

Carpenters commit to at most 24 base labour hours of remaining work, including
outstanding bids. Projects are tendered in portions of at most 12 base hours.
Starting work on one portion opens the next portion's 24-hour auction; different
carpenters may work concurrently. Each portion has its own fixed price, materials,
completion payment, and 30-day deadline. Partial work and installed materials
survive failure, and replacement contracts cover only the remainder.

Tender estimates use current material prices and the median qualified carpenter's
skill-adjusted labour and procurement costs, ignoring current workload, plus 20%.
Procurement is estimated for each portion. Reserve the full affordable project
budget upfront, allocate it proportionally to remaining work, and refund unused
winning allowances. Household commitments protect seven days of essentials.
Smaller affordable offers remain permitted; construction is manually requested.

Rooms and constructed objects wear from full condition to zero in 100 days.
Objects' wear increases linearly from 1x to 5x as their room deteriorates. Condition
does not yet reduce effectiveness or storage capacity; nothing is destroyed.
All objects at river and forest locations, including their storage, do not degrade. At 95% condition or below,
owners automatically tender a repair for the damage at posting. Repairs require
wood and labour proportional to original construction costs and improve condition
as work is completed. Ongoing wear is additional damage. One repair project per
target, lowest condition first when funding is limited. No-bid repair auctions
repost for another 24 hours with refreshed damage and estimate. Guilds fund their
property's repairs without protecting a cash reserve. The town treasury funds public
market and warehouse storage repairs without a reserve. Construction remains manual.

## Commit steps

1. Refactor construction to independent portions, 24-hour carpenter commitments,
   project-wide escrow and revised tender estimates. Update save validation and UI
   callers. Verify simultaneous builders, queue capacity, material ownership,
   continuation, accounting, save/load and split advancement. Independent review,
   fix valid high/medium issues, fresh review, then commit.
2. Add persistent condition, deterministic wear, proportional repair projects,
   automatic owner/guild commissioning and no-bid retries, and compact condition
   and repair displays. Verify timing, room multipliers, repair snapshots,
   concurrent wear/repair, ownership, funding and persistence. Independent review,
   resolve findings, fresh review, then commit.

## Validation notes

Keep user-owned untracked AGENTS.md untouched. Existing desktop tests
`details_distinguish_current_goal_from_forecast` and
`decision_panel_shows_reachable_hunger_and_uses_saved_prices` fail at baseline.
Desktop tests need RUST_MIN_STACK=16777216. Existing Clippy warnings outside this
work remain outside scope. Save format changes use a new strict version; no
migration is implemented.

### Step 1 implementation notes

Projects retain aggregate progress for displays and own fixed portions with local
work/material totals. Contracts quote one portion's remainder. Future contract IDs
are assigned when commissioning, preserving deterministic split advancement.
Unallocated project funding is held separately from each portion's escrow; failed
or cancelled portions stop future scheduling and release that unallocated funding,
while already posted portions continue. Manual continuation requires all current
portions to close and re-estimates only unfinished work.

Review corrected separate reservations for each carpenter's carried wood and the
household's shared stored/listed wood, and rejected saved future IDs colliding with
another portion's historical contract. Multi-portion regression testing caught an
hourly quote timer drifting with caller advance size; new auctions now process
immediately and subsequent checks stay on their posting-time grid.

Step 1 validation complete: simulation package 611 passed (423 library and 188
integration), six ignored; all 18 construction tests pass. Focused finance tests
10/10, desktop construction controls 9/9, and citizen construction display tests
3/3 pass. Workspace check, formatting check, and diff whitespace check pass.
Independent review has no remaining high or medium findings. Clippy will be run
again with the final maintenance step; no unrelated baseline warnings are changed.
