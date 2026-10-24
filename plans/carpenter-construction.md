# Carpenter construction

Status: step 1 implemented; verification and independent review recorded below. Step 2 implemented and independently reviewed. Step 3 is implemented with the agreed milestone payments and persistent unfinished construction; final verification is recorded below. Step 4 is implemented; verification and independent review are recorded below.

## Scope

Add carpenter-built rooms and installed objects, commissioned manually from Locations through household-funded market requests. Carpenters procure materials and perform persistent work. Autonomous commissioning, repair/deterioration, new standalone buildings, land purchases, storage quantity limits, and employee/bulk contracts are deferred. Payments are released at 12-base-labour-hour milestones.

Use the existing building/room/object hierarchy, shared economic accounts, goods market, travel, and needs systems. Keep UI controls compact; explanations belong in tooltips or inspection details.

## Space and ownership

- Rooms have small (6 m²), medium (12 m²), and large (24 m²) presets. No geometric placement or composition restrictions.
- Object footprints include operating/access clearance. Their summed footprints cannot exceed room area.
- Assign existing rooms the smallest preset that fits their contents. Report any room exceeding the largest preset before choosing an exception.
- A plot has 120 m² of total room area shared by all buildings. Completed rooms and pending room requests count. Outdoor resources/crop plots do not.
- Building owners commission installed objects and repairs; room expansion requires ownership of both building and plot. Resolve ownership through household membership where applicable. Residents do not independently gain alteration rights.
- Completed improvements belong to the building owner. Ordinary facility access remains unchanged.
- Reserve room area for requested objects and plot allowance for requested rooms when posting; release reservations on cancellation/failure only when no work/materials have been installed. Unfinished construction retains its space until completed; demolition is outside this feature.
- Preserve rooms and their other contents when residents move. Constructed beds may be unassigned; resident assignment is separate.

## Construction requirements

Wood remains stored in grams. Required wood equals the output of unskilled woodcutting over the base construction labour time: currently 200 g per labour hour. Skill improvements reduce actual work duration, not material requirements. Rooms finish empty.

| Room | Area | Base labour | Wood |
|---|---:|---:|---:|
| Small | 6 m² | 30 hours | 6,000 g |
| Medium | 12 m² | 60 hours | 12,000 g |
| Large | 24 m² | 120 hours | 24,000 g |

| Object | Footprint | Base labour | Wood |
|---|---:|---:|---:|
| Bed | 3 m² | 20 hours | 4,000 g |
| Pantry shelving | 2 m² | 10 hours | 2,000 g |
| Dry goods bin | 2 m² | 10 hours | 2,000 g |
| Fish crate | 1 m² | 5 hours | 1,000 g |
| Wood rack | 3 m² | 10 hours | 2,000 g |
| Textile chest | 1 m² | 15 hours | 3,000 g |
| Liquid barrel | 1 m² | 20 hours | 4,000 g |
| Millstone | 6 m² | 40 hours | 8,000 g |
| Oven | 4 m² | 40 hours | 8,000 g |
| Spinning wheel | 2 m² | 30 hours | 6,000 g |
| Loom | 6 m² | 40 hours | 8,000 g |
| Sewing table | 3 m² | 20 hours | 4,000 g |
| Brewing equipment | 6 m² | 50 hours | 10,000 g |
| Fermentation vessel | 2 m² | 20 hours | 4,000 g |

All materials are wood initially, including placeholders for equipment that would eventually use other materials. Storage objects retain existing good compatibility and mixing restrictions without quantity capacity limits.

## Household budgets and quotes

- Maximum client offer is cash minus existing commitments minus seven days of household essentials, floored at zero. Any positive offer is allowed, even one coin.
- Estimate essential quantities at current market prices, including food and water. Do not equate recent purchases with needs: households may self-produce, buy production inputs, or go without. Sleeping currently has no rental charge.
- Use seven-day history where applicable to estimate current lifestyle consumption and earning time. Separate recurring household costs from materials and capital purchases. Include clothing replacement and account for taxes/fees without double counting.
- Carpenter hourly earnings target covers household essentials, current discretionary consumption, and a reserve-replenishment allowance, divided by realistic combined household earning hours. It does not fund an aspirational lifestyle.
- Quote materials at current market values, including owned materials, and add expected procurement/construction time at the earnings target. No separate profit-margin setting initially.
- A carpenter bids only when the job fits the maximum offer, can be completed by the deadline, and can be financed. Financing includes the materials and household living costs needed to reach the next milestone payment, accounting for other commitments and shared funds. Availability of the full material requirement is not a bidding prerequisite; carpenters can procure over time and record unmet demand for caravans.
- Recheck the client's protected reserve at award. Cancel if the winning payment would compromise it.

## Market contract lifecycle

1. Owner selects a specific room size or installed object and destination. Validate authority, area, and positive budget; reserve the maximum payment and required area.
2. Collect one sealed quote per carpenter for 24 simulation hours. A carpenter may have one outstanding quote or active job. Quotes are not revised to undercut others.
3. Award to the lowest valid quote; randomly break equal-price ties. Revalidate affordability and commitments at award. Release the difference between maximum budget and winning quote.
4. No valid quotes closes the request and releases cash/area. Reposting an unsuccessful request requires an increased available budget; do not add autonomous reposting in this version.
5. Winner sources materials through existing economic actions and travels to the job site. Grant contract-specific work access, not general access to the client's stock.
6. Procure and consume materials for each milestone when work on that milestone starts. Work proceeds in one-hour actions (or a shorter final action), with persistent base-work progress. Meals, sleep, and replanning preserve progress. Awarded work remains a work commitment.
7. Release proportional payment after each 12 base labour hours, with a shorter final milestone when needed. Skills reduce actual time, not material quantities or payment proportions. Use cumulative integer settlement so the final total exactly matches the quote. On completion, create the usable room/object and convert the space reservation to occupied area.
8. The deadline is 30 simulation days after award. Failure releases unpaid client escrow. All completed work and installed materials, including partial progress within an unpaid milestone, remain as household-owned unfinished construction occupying its space. Paid milestones are not refunded; the departing carpenter receives no payment for an incomplete milestone. Unused, uninstalled materials remain the carpenter's.
9. A manual continuation request seeks quotes only for remaining work and missing materials. Preserve project identity, progress, installed materials and space across contracts; prevent concurrent contracts on the same project. Completed work is not usable until the full room/object is finished.

Owner cancellation is allowed during bidding. Post-award discretionary cancellation is not part of the initial lifecycle. There are no advance deposits; milestone payments come from the client's full upfront escrow. Escrow remains owned wealth but cannot fund ordinary purchases or another commitment.

## Implementation steps

Each implementation step requires appropriate behavioural checks, an independent gpt-6.1-sol code review where available, resolution of valid high/medium findings followed by fresh review, and a commit. Fix low documentation findings and report other low findings. Stop to discuss significant simplifications or unexpected changes of scope/behaviour.

### 1. Space model

- Replace object-count limits with room areas and footprints; add plot allowance and validation.
- Adapt initial world construction, bedroom relocation, and bed representation without introducing automatic extra housing.
- Update existing area displays and persistence validation; follow the repository's strict save-version policy.
- Verify initial rooms fit, mixed contents work, oversized contents fail, and resident changes preserve other furnishings.

### 2. Household affordability and carpenter pricing

- Add inspectable essential reserve, recurring lifestyle cost, earning-hours, replenishment, and quote calculations.
- Reuse history and market estimates while distinguishing consumption from production procurement and capital spending.
- Define explicit fallback/tuning values for new households, sparse markets, earning hours, and reserve replenishment before implementing dependent calculations.
- Verify shared-account accounting, missing history, self-provisioning, reserve protection, and financing over delayed payment.

### 3. Contracts and carpenter execution

- Add carpenter skill/starting role and initial-world provisioning; settle initial carpenter count and skill before wiring generation.
- Implement distinct construction contract records within the market feature, escrow, bidding, deterministic simulation event boundaries, area reservations, settlement, and persistence.
- Integrate procurement over time, contract-specific access, persistent project progress, short work actions, milestone payments, needs breaks, and skill practice.
- Preserve unfinished projects across failure, including work/materials within unpaid milestones; manually re-tender only their remaining requirements. Financing forecasts the next payment rather than the entire project.
- Reuse existing goods shopping and unlimited carrying; do not add a hauling-capacity subsystem.
- Verify auction timing, random tie handling, competing commitments within one household, no-bid closure, cancellation, award revalidation, exact-once settlement, failure cleanup, and save/load during work.
- Verify identical outcomes for equivalent split/large time advances, including completion/deadline ordering.

### 4. Locations controls and end-to-end evaluation

- Add an Add room control on building rows and an Add object control on room rows. Disable unavailable choices with tooltip reasons. Add a compact continuation control for unfinished projects without an active request/contract.
- Submit requests through the existing simulation mutation channel; validate again in simulation state.
- Expose concise request/bid/work status and inspection details for budget, quotes, progress, blockers, and deadline.
- Exercise low-budget/no-bid requests and successful object/room construction through the full workflow. Inspect compact layouts and run relevant workspace checks.
- Observe carpenter liquidity, wood supply, quote affordability, work/needs balance, and deadline success before tuning or extending scope.

## Decisions to settle during preparation

- Exact essential basket and price fallback when local goods are absent.
- Expected earning hours and reserve-replenishment horizon, including new households.
- How much discretionary consumption history to preserve versus minimum essential requirements.
- Procurement/deadline forecasting under limited current supply; expected future supply is not a guaranteed purchase.
- Contract payments are untaxed initially (agreed); record them separately from ordinary goods trades and capital/lifestyle consumption.
- Initial carpenter population/skills and handling of privately owned versus guild-owned building controls.
- Unassigned-bed assignment behaviour and visibility; no new rental system is implied.

These are outstanding details, not authorization to broaden the feature. The approved scope remains manually requested rooms and installed objects.

## Step 1 implementation evidence

- Replaced object counts with typed 6/12/24 m² room presets and catalogue footprints; initial rooms choose the smallest fitting preset.
- Added shared 120 m² plot allowance checks for additional buildings and save validation. Construction reservations follow in step 3.
- Relocation preserves rooms and furnishings, leaves old beds unassigned, and reuses vacant beds only in household-owned buildings. Existing household setup can still create bedrooms; no autonomous housing was added.
- Locations displays used/total room area and unassigned beds. Save format is version 11, with no migration.
- Behaviour checks cover initial preset sizing, mixed contents, overcrowding, plot limits, room preservation, vacant beds, and mixed building ownership.
- Independent review identified an ownership issue in vacant-bed reuse; it was fixed and a fresh review found no high or medium issues.
- Verification: final simulation package suite passed (395 library tests and 188 integration tests, six ignored); all five final space tests passed. Formatting passes. Workspace Clippy passes with three existing warnings in unchanged production/planning code; strict warnings-as-errors fails those warnings.
- Workspace desktop tests need a larger test stack (`RUST_MIN_STACK=16777216`); the default-stack market-test overflow also reproduces at HEAD. Two unrelated desktop assertions reproduce at HEAD: `details_distinguish_current_goal_from_forecast` expects “Forecast after replanning”, and `decision_panel_shows_reachable_hunger_and_uses_saved_prices` expects “Replenish reserves”. Final desktop suite: 155 passed, two baseline assertion failures described above, one ignored; the long-running initial-town test passed.

## Step 2 implementation evidence

- Added inspectable household/singleton budgets, hourly carpenter earnings, quotes and delayed-payment financing; ordinary goods pricing is unchanged.
- Reserve protects seven days of hunger-rate-specific nutrition and 400 g water per member per day. Food uses the cheapest obtainable offer per nutrition; when no edible offers exist, world reference prices provide the fallback. Estimates do not guarantee future supply volume.
- Lifestyle uses up to seven complete days of actual personal consumption at current prices, with food/water essentials as a lower bound. Equipped garments add replacement amortization over their existing ten-day lifetime. Material/capital purchases and production inputs do not enter lifestyle.
- Existing consumed history mixes production inputs with personal consumption. Added separate citizen personal-consumption attribution and standalone workshop-fee payments; strict save version is now 12, without migration.
- Earning time initially assumes eight hours per member per day. Complete history uses production/travel/trade hours, conservatively subtracts food-goal time, and clamps to one–24 hours per member. Existing food-goal history includes meals, so subtraction can underestimate earning time; the lower bound prevents zero denominators.
- Earnings include observed cash taxes/workshop fees once, plus reserve deficit replenishment over seven days. Budget deducts actual tax arrears, workshop liabilities and caller-supplied new commitments exactly once. Contract payments remain untaxed initially; ordinary goods taxes remain intact.
- Quotes value owned and missing materials equally, adding expected procurement and skill-adjusted construction hours. Financing charges procurement only for missing materials and recurring living costs over forecast calendar time until the next payment, without assuming future receipts.
- Added seven finance behavior tests covering shared accounts, missing/complete history, deprivation/self-provisioning, production and capital exclusion, standalone tax/fee history, earning time, liabilities, owned-material valuation and delayed financing. Extreme-balance tests preserve one-coin commitments and exact fractional liability rounding.
- Independent review found loss of cash precision above 2^53. Protected cash/commitments/reserve now use integer arithmetic; debt components and fractional workshop fees aggregate exactly before rounding. README save-version documentation was corrected. Fresh review found no remaining high/medium issues.
- Verification: full simulation library passed (402 passed, six ignored), including all seven finance tests and existing history, guild fee and persistence checks. Formatting passes.

## Step 3 implementation evidence

- Added a construction catalogue, persistent projects and sealed market contracts, full client escrow, hourly bid opportunities over a 24-hour auction, random equal-price selection, award revalidation and a 30-day deadline. Carpenter qualification and practice use the existing skill model. Three new unpractised carpenters bring the initial population to 33 and site count to 59.
- Work advances in actions of at most one actual hour. Each contract pays cumulative rounded price fractions at 12 base-hour milestones and its shorter final milestone. Skill changes actual time only. Milestone materials are installed before work; completed and partial work and installed materials survive failure. Manually commissioned continuation prices only remaining work and missing materials.
- Carpenters collect owned supplies and buy available partial fills through ordinary goods actions, retaining unmet demand for later supply. Quoted and active material requirements protect supplies from ordinary excess listing. Needs and reserve tasks use existing planner eligibility; ordinary planning resumes when work ends.
- Escrow is removed from spendable cash and remains owned wealth. Dedicated citizen/household history fields record deposits, refunds and payments. New commitments account for shared household financing; essential goods may use living cash, other workers' outstanding procurement capital is protected, and shared owned wood is allocated once across quotes. Contract payments are untaxed; goods trades retain existing taxes.
- Added strict save format 13 with validation of project/output identity, retained area, contract chronology, escrow/payment totals, continuation progress ranges, skill-adjusted actions, material coverage and derived access/finance context. Public mutation rejects forged construction actions, and committed ordinary plans cannot contain them. Household formation rejects outstanding construction commitments and retained unfinished client work.
- Fourteen behavior tests cover escrow/refunds, auction/no-bid/retry behavior, random ties, area reuse, award reserves and sealed prices, short-action save/load and corruption, persistent partial milestones, a real deadline and continuation through completion, partial purchases from three suppliers over time, two active household carpenters completing separately financed jobs, shared wood allocation and ordinary essential spending.
- Independent review identified and resolved ownership/area revalidation, immutable sealed pricing, shared materials/capital accounting, losing-bidder availability and strict action/save validation issues. Final verification passed 416 simulation library tests (six ignored) and 188 integration tests. Workspace check passes; Clippy reports only the three existing production/planning warnings. Formatting and diff checks pass. The final failure-travel cleanup rerun also passed.


## Step 4 implementation evidence

- Locations now has compact building Add room and room Add object menus. All three room sizes and fourteen objects use the owner household's maximum offer. Invalid area, ownership, reserve and no-bid retry choices are disabled with tooltip reasons; guild-funded controls are disabled explicitly. No autonomous requests were added.
- Requests, cancellation and continuation use the existing generation-aware worker mutation channel, with shared pending guards for tax/caravan edits and simulation-side revalidation. Continuation excludes its own retained reservation when checking the destination. Same-time requests use contract sequence for latest-status display.
- Project rows show status, bid count and progress; budget, wood, quotes, paid milestones and deadlines are available on hover. Bidding has Cancel; closed unfinished work has Continue. Citizen and household history conditionally show construction escrow deposits/refunds and receipts, without subtracting milestone payments from cash a second time.
- Eight construction UI tests cover cashflow accounting, cash/area restrictions, pending area reservations, disabled tooltip-bearing menu items, no-bid higher-budget rules, request/cancel and continuation through worker mutations, same-time contract ordering, and stale-generation clicks. All pass. Final focused reruns also passed all 28 Locations tests, seven caravan-control tests, and four corrected population/table tests (47 checks total).
- Ran the full desktop suite once with `RUST_MIN_STACK=16777216`: 159 passed, six failed, one ignored, including successful completion of the long initial-town production test (240 seconds total). Four failures were stale expectations of 30 citizens/eight professions; tests now use actual population or the approved 33 citizens/nine professions, preserving five table cells per citizen, and all four pass individually. The other two reproduce the previously recorded baseline assertions: `details_distinguish_current_goal_from_forecast` expects “Forecast after replanning”; `decision_panel_shows_reachable_hunger_and_uses_saved_prices` expects “Replenish reserves”. The full suite was not repeated after the focused corrections. Default-stack overflow is the previously recorded baseline limitation.
- Workspace check, formatting and diff checks pass. Workspace Clippy including all targets reports only the three existing simulation production/planning warnings; no new desktop warnings remain. No economic tuning was added. Step 3's behavioral scenarios supply completion/deadline/liquidity/procurement coverage; the UI tests exercise manual dispatch and retry controls.
- Inspected a native Bevy/Vulkan 1600×1000 capture at 1.25 scale using a temporary screenshot harness and empty temporary launch directory. The fourteen-choice menu fits the scrollable detail panel, with disabled entries muted and no layout blocker. New controls and room area labels use ASCII symbols because the current font lacks several Unicode glyphs. The existing window-title glyph remains unchanged. No screenshot harness code or test saves are included in the repository.
- Independent review found one low duplicate-error display issue, which was fixed. Fresh review found no high/medium issues. A final fresh pass also cleared the glyph, resource grouping and population-test changes with no remaining findings.
