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
property's repairs without protecting a cash reserve, including no reserve for property taxes. The town treasury funds public
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

### Step 2 implementation notes

Condition uses fixed-point lifetime units. Natural wear is continuous, and hourly
commissioning is anchored to absolute simulation hours. Repair work credits are
aggregated before advancing the map, so a room's repair trajectory affects its
contents consistently without depending on agent iteration order. Condition does
not change object effectiveness. Natural/crop objects have no construction repair
cost and stay outside this system; all river/forest objects are exempt.

Repair targets reference existing rooms or objects; public outdoor storage needs
no invented building. A real Account/Town payer distinguishes guild/household cash
from the treasury. Guild construction cash flows reconcile separately from fees.
An initial unawarded repair refreshes its damage snapshot when reposting. Once any
portion is awarded, the project keeps that scope across later failed auctions or
contracts; additional wear can trigger another project after completion. Partial
work immediately improves condition, while installed wood and remaining work
remain on the same project after failure.

Review tightened saved-stage validation to require canonical proportional wood
allocations, preventing forged zero-material portions, and forbids duplicate
unfinished repair projects for one target. Deterministically seeded IDs preserve
auto-posting and auction behaviour across split advancement. Save version is 15.

Focused step 2 repair tests: nine pass. These cover hourly eligibility and reserve
protection, no-bid snapshot refresh with exact split-advance/save equivalence,
lowest-condition funding priority, partial repair credit/material retention and
fixed-scope continuation, active room repair with exact work/cash across split
advancement, ongoing wear after completed repair, real guild/treasury payments and
conservation, forged material/duplicate target rejection, and household formation
followed by canonical payer/history rebinding. The partial-failure unit injects an
accelerated deadline; the existing construction integration test exercises the
canonical 30-day failure path. Active room repair persistence is checked separately.
Contents' fixed-point condition is compared with a tiny rounding tolerance during
active room repairs; contract progress and cash remain exact.


### Step 2 condition and display notes

Condition uses one bounded fixed-point integer per room/object. Normal wear is
exact across split advances; room/object repair credits are aggregated before
applying elapsed wear, avoiding carpenter iteration order. Room-condition
integration accounts for reaching zero or full condition during an interval.
Fractional work rounding during room repairs can introduce negligible condition
differences between caller partitions; payment and completed-work accounting stay
exact. Automatic repair inspection runs on absolute hourly boundaries.

A separate native screenshot harness was built into ignored `target/` output,
without replacing the production binary or touching user saves. Inspected a
1600x1000 Vulkan capture at 1.25 scale with zero-condition rooms/objects and active
repair auctions. Condition labels and repair rows fit the existing compact layout;
zero-condition storage still displays usable bread/water and unlimited quantity.
The existing missing window-title glyph remains unchanged. Capture:
`target/maintenance-native-capture/maintenance.png`.

Condition validation: all seven new tests pass in the combined simulation run,
covering elapsed integrals, zero/full caps, overflowing intermediate products,
maximum elapsed time, exact ordinary split advancement, resource exemptions,
combined repair credit order, and invalid saved condition bounds.

Desktop validation: the full 170-test run finished in 329.31 seconds with 165
passed, four failed and one ignored; the ten-day initial-town production/market
simulation passed with automatic maintenance active. Two additional failures were
resolved: room labels gained condition percentages, and the carpenter plan display
had unwrapped an absent selection despite the inspector resolving a fallback
citizen. The latter now passes the resolved citizen's ID. Fresh independent review
is clear. Final focused reruns pass all 28 Locations tests and all five inspector
tests, leaving only the two documented baseline desktop failures. Construction
controls/citizen display checks pass 12/12 and guild history checks pass 3/3.

The existing guild property-tax fixture exposed that guilds already have a weekly
expense. The user explicitly kept the no-reserve rule: repairs may leave guild tax
unpaid. The tax-history fixture now has enough cash to isolate tax accounting and
includes held repair escrow in its closing balance assertion.

Existing integration fixtures were adjusted for intended continuous wear and
hourly events: travel checks preserve geometry and room/object identity rather
than unchanged condition; extreme numeric rollback tests permit whichever
numerical overflow is detected first; treasury overflow uses two independently
funded large payers so reserved repair funds cannot prevent the intended overflow.
The canonical construction failure test restores supply and waits for queued
maintenance capacity before requesting continuation.
Town and private tax fixtures now include held construction escrow when checking
post-tax funds; all assessed/paid/arrears and rollback assertions remain intact.
No service payments occur in those fixtures. The three corrected integration
targets pass: simulation 15, taxes 14, travel 5.

Step 2 final validation: the complete simulation package passes 627 tests (439
library and 188 integration), zero failures and six ignored. The final nine repair
tests pass again after equivalent Clippy cleanup. Workspace check, formatting,
and diff whitespace checks pass. Workspace Clippy passes with only the three
unchanged baseline warnings in planning/goals and production/routes; no new
warnings remain. Independent review and fresh reviews of valid findings and
fixture adjustments have no remaining high or medium findings. Desktop results
and the native inspection are recorded above; the two known baseline desktop
failures remain outside this feature.
